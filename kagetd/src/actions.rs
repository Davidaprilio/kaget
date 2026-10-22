//! Actions: parsing, slider inverses and the catalog for the UI.

use serde::Serialize;

use crate::keys;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    System,
    /// Key codes sent through the virtual keyboard (`key:` / `shortcut:`).
    Keys(Vec<u16>),
    /// KDE global shortcut (kglobalaccel).
    Global { component: String, name: String },
    /// Volume / brightness / keyboard backlight; `dir` is +1 or -1, sizes come from `[steps]`.
    Step { kind: StepKind, dir: i32 },
    App(String),
    Command(String),
    NumberPad(NumberPadOp),
    Pause,
    /// Show / hide the touchpad control panel (`kaget-panel`).
    Panel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    Volume,
    Brightness,
    KeyboardBacklight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberPadOp {
    Toggle,
    BrightnessCycle,
    BrightnessUp,
    BrightnessDown,
}

/// Built-in actions: name, label, category.
const BUILTINS: &[(&str, &str, &str)] = &[
    ("volume.up", "Volume up", "Media"),
    ("volume.down", "Volume down", "Media"),
    ("volume.mute", "Mute", "Media"),
    ("media.play_pause", "Play / pause", "Media"),
    ("media.next", "Next track", "Media"),
    ("media.previous", "Previous track", "Media"),
    ("brightness.up", "Screen brightness up", "Hardware"),
    ("brightness.down", "Screen brightness down", "Hardware"),
    ("keyboard_backlight.up", "Keyboard backlight up", "Hardware"),
    ("keyboard_backlight.down", "Keyboard backlight down", "Hardware"),
];

/// KDE global shortcuts behind the non-stepped built-ins (Plasma 6).
const BUILTIN_GLOBALS: &[(&str, &str, &str)] = &[
    ("volume.mute", "kmix", "mute"),
    ("media.play_pause", "mediacontrol", "playpausemedia"),
    ("media.next", "mediacontrol", "nextmedia"),
    ("media.previous", "mediacontrol", "previousmedia"),
];

/// Slider pairs: moving back along a slider runs the inverse.
const INVERSES: &[(&str, &str)] = &[
    ("volume.up", "volume.down"),
    ("brightness.up", "brightness.down"),
    ("keyboard_backlight.up", "keyboard_backlight.down"),
    ("media.next", "media.previous"),
    ("numberpad.brightness_up", "numberpad.brightness_down"),
    ("kwin:Switch One Desktop to the Left", "kwin:Switch One Desktop to the Right"),
    ("kwin:Switch One Desktop Up", "kwin:Switch One Desktop Down"),
    ("kwin:Window One Desktop to the Left", "kwin:Window One Desktop to the Right"),
];

pub fn inverse(action: &str) -> Option<&'static str> {
    INVERSES.iter().find_map(|&(a, b)| {
        if a == action {
            Some(b)
        } else if b == action {
            Some(a)
        } else {
            None
        }
    })
}

impl Action {
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if let Some(&(_, c, n)) = BUILTIN_GLOBALS.iter().find(|(a, _, _)| *a == s) {
            return Ok(Action::Global { component: c.into(), name: n.into() });
        }
        let step = |kind, dir| Ok(Action::Step { kind, dir });
        Ok(match s {
            "volume.up" => return step(StepKind::Volume, 1),
            "volume.down" => return step(StepKind::Volume, -1),
            "brightness.up" => return step(StepKind::Brightness, 1),
            "brightness.down" => return step(StepKind::Brightness, -1),
            "keyboard_backlight.up" => return step(StepKind::KeyboardBacklight, 1),
            "keyboard_backlight.down" => return step(StepKind::KeyboardBacklight, -1),
            "none" | "" => Action::None,
            "system" => Action::System,
            "kaget.pause" => Action::Pause,
            "kaget.panel" => Action::Panel,
            "numberpad.toggle" => Action::NumberPad(NumberPadOp::Toggle),
            "numberpad.brightness_cycle" => Action::NumberPad(NumberPadOp::BrightnessCycle),
            "numberpad.brightness_up" => Action::NumberPad(NumberPadOp::BrightnessUp),
            "numberpad.brightness_down" => Action::NumberPad(NumberPadOp::BrightnessDown),
            _ => {
                let (kind, rest) = s.split_once(':').ok_or_else(|| format!("unknown action '{s}'"))?;
                let rest = rest.trim();
                if rest.is_empty() {
                    return Err(format!("action '{s}' needs a value after ':'"));
                }
                match kind {
                    "key" => Action::Keys(keys::parse_key_combo(rest)?),
                    "shortcut" => Action::Keys(keys::parse_shortcut(rest)?),
                    "kwin" => Action::Global { component: "kwin".into(), name: rest.into() },
                    "global" => {
                        let (c, n) = rest
                            .split_once(':')
                            .ok_or_else(|| "global action must be global:<component>:<shortcut name>".to_string())?;
                        Action::Global { component: c.into(), name: n.into() }
                    }
                    "app" => Action::App(rest.into()),
                    "cmd" => Action::Command(rest.into()),
                    _ => return Err(format!("unknown action type '{kind}'")),
                }
            }
        })
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct CatalogEntry {
    pub action: String,
    pub label: String,
    pub category: String,
    pub slider: bool,
}

fn entry(action: &str, label: &str, category: &str) -> CatalogEntry {
    CatalogEntry {
        action: action.into(),
        label: label.into(),
        category: category.into(),
        slider: inverse(action).is_some(),
    }
}

/// Built-in actions plus the KWin and Plasma shortcuts reported by the desktop.
pub fn catalog(kwin_shortcuts: &[String], plasma_shortcuts: &[String], numberpad: bool) -> Vec<CatalogEntry> {
    let mut v = vec![
        entry("none", "Do nothing", "General"),
        entry("system", "System default", "General"),
        entry("kaget.pause", "Pause / resume KAGeT gestures", "General"),
        entry("kaget.panel", "Show / hide the touchpad control panel", "General"),
    ];
    for &(name, label, cat) in BUILTINS {
        v.push(entry(name, label, cat));
    }
    if numberpad {
        v.push(entry("numberpad.toggle", "NumberPad on / off", "NumberPad"));
        v.push(entry("numberpad.brightness_cycle", "NumberPad brightness (cycle)", "NumberPad"));
        v.push(entry("numberpad.brightness_up", "NumberPad brightness up", "NumberPad"));
        v.push(entry("numberpad.brightness_down", "NumberPad brightness down", "NumberPad"));
    }
    const PLASMA_LABELS: &[(&str, &str)] = &[
        ("activate application launcher", "Application launcher"),
        ("toggle do not disturb", "Do Not Disturb on / off"),
        ("next activity", "Next activity"),
    ];
    let label = |s: &str| PLASMA_LABELS.iter().find(|(n, _)| *n == s).map(|(_, l)| *l);
    if plasma_shortcuts.is_empty() {
        for &(n, l) in PLASMA_LABELS {
            v.push(entry(&format!("global:plasmashell:{n}"), l, "Plasma"));
        }
    }
    for s in plasma_shortcuts {
        v.push(entry(&format!("global:plasmashell:{s}"), label(s).unwrap_or(s), "Plasma"));
    }
    for s in kwin_shortcuts {
        let a = format!("kwin:{s}");
        let cat = if s.starts_with("Window") { "Window" } else { "System" };
        v.push(entry(&a, s, cat));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_forms() {
        assert_eq!(Action::parse("volume.up").unwrap(), Action::Step { kind: StepKind::Volume, dir: 1 });
        assert_eq!(
            Action::parse("media.next").unwrap(),
            Action::Global { component: "mediacontrol".into(), name: "nextmedia".into() }
        );
        assert_eq!(Action::parse("shortcut:Ctrl+Alt+T").unwrap(), Action::Keys(vec![29, 56, 20]));
        assert_eq!(
            Action::parse("kwin:Overview").unwrap(),
            Action::Global { component: "kwin".into(), name: "Overview".into() }
        );
        assert_eq!(
            Action::parse("global:plasmashell:toggle do not disturb").unwrap(),
            Action::Global { component: "plasmashell".into(), name: "toggle do not disturb".into() }
        );
        assert_eq!(Action::parse("app:org.kde.kcalc.desktop").unwrap(), Action::App("org.kde.kcalc.desktop".into()));
        assert_eq!(Action::parse("system").unwrap(), Action::System);
        assert!(Action::parse("cmd:").is_err());
        assert!(Action::parse("explode").is_err());
    }

    #[test]
    fn inverses() {
        assert_eq!(inverse("volume.down"), Some("volume.up"));
        assert_eq!(inverse("kwin:Switch One Desktop to the Left"), Some("kwin:Switch One Desktop to the Right"));
        assert_eq!(inverse("kwin:Overview"), None);
    }
}
