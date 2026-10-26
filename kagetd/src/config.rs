//! Configuration (`~/.config/kaget/config.toml`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::actions::Action;
use crate::triggers::Trigger;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub general: General,
    pub device: DeviceFallback,
    pub area: Sides,
    pub corners: Corners,
    pub side_drag: SideDrag,
    pub slider: Slider,
    pub gestures: Gestures,
    pub taps: Taps,
    pub steps: Steps,
    pub numberpad: NumberPadCfg,
    pub panel: PanelCfg,
    pub bindings: Vec<Binding>,
}

/// Size of one step of the brightness / volume / keyboard backlight actions.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Steps {
    pub brightness_percent: f64,
    pub volume_percent: f64,
    pub keyboard_backlight_levels: u32,
}

impl Default for Steps {
    fn default() -> Self {
        Self { brightness_percent: 1.0, volume_percent: 1.0, keyboard_backlight_levels: 1 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct General {
    pub enabled: bool,
    /// `passive`: read the touchpad alongside KDE. `filter`: grab it and re-emit it through a clone,
    /// withholding touches that become KAGeT gestures (needed to override KDE's gestures).
    pub mode: String,
    /// Filter mode only: system-owned triggers (KDE's 3/4-finger swipes and pinches, 3-finger tap)
    /// can be bound.
    pub override_system_gestures: bool,
}

pub const MODES: &[&str] = &["passive", "filter"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct DeviceFallback {
    pub width_mm: f64,
    pub height_mm: f64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct Sides {
    pub enabled: bool,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Corners {
    pub enabled: bool,
    pub width: f64,
    pub height: f64,
    pub hold_ms: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SideDrag {
    pub enabled: bool,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
    pub min_travel_mm: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Slider {
    pub step_mm: f64,
    pub continue_s: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Gestures {
    pub swipe_mm: f64,
    pub pinch_ratio: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Taps {
    pub max_tap_ms: u32,
    pub double_tap_ms: u32,
    pub hold_ms: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct NumberPadCfg {
    pub enabled: bool,
    pub calculator_command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Binding {
    pub trigger: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when_running: Option<String>,
}

impl Default for General {
    fn default() -> Self {
        Self { enabled: true, mode: "passive".into(), override_system_gestures: false }
    }
}
impl Default for DeviceFallback {
    fn default() -> Self {
        Self { width_mm: 100.0, height_mm: 60.0 }
    }
}
impl Default for Sides {
    fn default() -> Self {
        Self { enabled: true, top: 27.0, right: 27.0, bottom: 0.0, left: 27.0 }
    }
}
impl Default for Corners {
    fn default() -> Self {
        Self { enabled: true, width: 8.0, height: 10.0, hold_ms: 320 }
    }
}
impl Default for SideDrag {
    fn default() -> Self {
        Self { enabled: false, top: 4.0, right: 4.0, bottom: 4.0, left: 4.0, min_travel_mm: 10.0 }
    }
}
impl Default for Slider {
    fn default() -> Self {
        Self { step_mm: 8.0, continue_s: 2.0 }
    }
}
impl Default for Gestures {
    fn default() -> Self {
        Self { swipe_mm: 12.0, pinch_ratio: 1.3 }
    }
}
impl Default for Taps {
    fn default() -> Self {
        Self { max_tap_ms: 250, double_tap_ms: 300, hold_ms: 500 }
    }
}
/// Touchpad control panel (`kaget-panel`): its slots from left to right. A slot is a built-in slider
/// (`PANEL_CONTROLS`) or `action:<action>`, a button that runs the action (a tap in its column).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PanelCfg {
    pub controls: Vec<String>,
    /// `bottom`, `top`, `center`, `top_left`, `top_right`, `bottom_left`, `bottom_right`.
    pub position: String,
    /// Size factor of the whole panel (the `scale` slider changes it live).
    pub scale: f64,
    /// Panel mode ends after this many seconds without slider use; 0 keeps it open.
    pub auto_hide_s: f64,
}

pub const PANEL_CONTROLS: &[&str] = &["brightness", "volume", "keyboard_backlight", "scale"];
pub const PANEL_MAX_SLOTS: usize = 6;

/// The action of an `action:<action>` panel slot.
pub fn panel_slot_action(slot: &str) -> Option<&str> {
    slot.strip_prefix("action:")
}
pub const PANEL_POSITIONS: &[&str] = &["bottom", "top", "center", "top_left", "top_right", "bottom_left", "bottom_right"];
pub const PANEL_SCALE: (f64, f64) = (0.5, 2.0);

impl Default for PanelCfg {
    fn default() -> Self {
        Self {
            controls: PANEL_CONTROLS.iter().map(|s| s.to_string()).collect(),
            position: "bottom".into(),
            scale: 1.0,
            auto_hide_s: 3.0,
        }
    }
}

impl Default for NumberPadCfg {
    fn default() -> Self {
        Self { enabled: true, calculator_command: "kcalc".into() }
    }
}

fn b(trigger: &str, action: &str) -> Binding {
    Binding { trigger: trigger.into(), action: action.into(), when_running: None }
}

pub fn default_bindings() -> Vec<Binding> {
    vec![
        b("corner.top_right.hold", "numberpad.toggle"),
        b("corner.top_left.hold", "numberpad.brightness_cycle"),
        b("corner.top_left.swipe.right", "brightness.up"),
        b("corner.top_left.swipe.down", "cmd:kcalc"),
        b("corner.top_left.swipe.diagonal", "cmd:kcalc"),
        b("corner.top_right.swipe.left", "brightness.down"),
        b("corner.top_right.swipe.down", "volume.down"),
        // Side drags start without bindings (and disabled): the system keeps its defaults.
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            general: General::default(),
            device: DeviceFallback::default(),
            area: Sides::default(),
            corners: Corners::default(),
            side_drag: SideDrag::default(),
            slider: Slider::default(),
            gestures: Gestures::default(),
            taps: Taps::default(),
            steps: Steps::default(),
            numberpad: NumberPadCfg::default(),
            panel: PanelCfg::default(),
            bindings: default_bindings(),
        }
    }
}

pub const WHEN_RUNNING: &[&str] = &["new", "close", "focus", "minimize", "toggle"];

impl Config {
    /// Check values and bindings; returns a human-readable error.
    pub fn validate(&self) -> Result<(), String> {
        let p = &self.panel;
        if p.controls.is_empty() {
            return Err("panel.controls must list at least one control".into());
        }
        if p.controls.len() > PANEL_MAX_SLOTS {
            return Err(format!("panel.controls: at most {PANEL_MAX_SLOTS} slots"));
        }
        for (i, c) in p.controls.iter().enumerate() {
            if let Some(a) = panel_slot_action(c) {
                match Action::parse(a) {
                    Ok(Action::None | Action::System | Action::SystemAs(_)) => {
                        return Err(format!("panel.controls: '{c}' does nothing"));
                    }
                    Ok(_) => {}
                    Err(e) => return Err(format!("panel.controls: '{c}': {e}")),
                }
            } else if !PANEL_CONTROLS.contains(&c.as_str()) {
                return Err(format!(
                    "panel.controls: unknown control '{c}' (use {} or action:<action>)",
                    PANEL_CONTROLS.join(", ")
                ));
            }
            if p.controls[..i].contains(c) {
                return Err(format!("panel.controls: '{c}' is listed twice"));
            }
        }
        if !PANEL_POSITIONS.contains(&p.position.as_str()) {
            return Err(format!("panel.position must be one of {}", PANEL_POSITIONS.join(", ")));
        }
        if !p.scale.is_finite() || p.scale < PANEL_SCALE.0 || p.scale > PANEL_SCALE.1 {
            return Err(format!("panel.scale must be between {} and {}", PANEL_SCALE.0, PANEL_SCALE.1));
        }
        if !p.auto_hide_s.is_finite() || !(0.0..=60.0).contains(&p.auto_hide_s) {
            return Err("panel.auto_hide_s must be between 0 and 60".into());
        }
        if !MODES.contains(&self.general.mode.as_str()) {
            return Err(format!("general.mode must be one of {}", MODES.join(", ")));
        }
        let mm = |name: &str, v: f64, max: f64| -> Result<(), String> {
            if !v.is_finite() || v < 0.0 || v > max {
                Err(format!("{name} must be between 0 and {max} mm"))
            } else {
                Ok(())
            }
        };
        for (n, v) in [("area.top", self.area.top), ("area.right", self.area.right),
                       ("area.bottom", self.area.bottom), ("area.left", self.area.left)] {
            mm(n, v, 80.0)?;
        }
        for (n, v) in [("side_drag.top", self.side_drag.top), ("side_drag.right", self.side_drag.right),
                       ("side_drag.bottom", self.side_drag.bottom), ("side_drag.left", self.side_drag.left)] {
            mm(n, v, 30.0)?;
        }
        mm("corners.width", self.corners.width, 60.0)?;
        mm("corners.height", self.corners.height, 60.0)?;
        if self.slider.step_mm < 1.0 || self.slider.step_mm > 50.0 {
            return Err("slider.step_mm must be between 1 and 50".into());
        }
        if !(0.0..=10.0).contains(&self.slider.continue_s) {
            return Err("slider.continue_s must be between 0 and 10".into());
        }
        for (name, v) in [("steps.brightness_percent", self.steps.brightness_percent),
                          ("steps.volume_percent", self.steps.volume_percent)] {
            if !v.is_finite() || !(0.1..=25.0).contains(&v) {
                return Err(format!("{name} must be between 0.1 and 25"));
            }
        }
        if !(1..=10).contains(&self.steps.keyboard_backlight_levels) {
            return Err("steps.keyboard_backlight_levels must be between 1 and 10".into());
        }
        if self.gestures.pinch_ratio <= 1.0 {
            return Err("gestures.pinch_ratio must be greater than 1".into());
        }
        if self.taps.max_tap_ms == 0 || self.taps.hold_ms == 0 || self.corners.hold_ms == 0 {
            return Err("tap and hold times must be greater than 0".into());
        }
        for (i, bnd) in self.bindings.iter().enumerate() {
            Trigger::parse(&bnd.trigger).map_err(|e| format!("binding {}: {e}", i + 1))?;
            Action::parse(&bnd.action).map_err(|e| format!("binding {} ({}): {e}", i + 1, bnd.trigger))?;
            if let Some(w) = &bnd.when_running
                && !WHEN_RUNNING.contains(&w.as_str())
            {
                return Err(format!("binding {}: when_running must be one of {}", i + 1, WHEN_RUNNING.join(", ")));
            }
        }
        Ok(())
    }

}

impl Config {
    /// The config the engine runs with: bindings of disabled groups removed and, when the gesture
    /// areas are off, all area bands set to 0. NumberPad bindings stay (its toggle is a printed corner icon).
    pub fn filter_mode(&self) -> bool {
        self.general.mode == "filter"
    }

    pub fn effective(&self) -> Config {
        let mut c = self.clone();
        c.bindings.retain(|b| {
            let numberpad = b.action.starts_with("numberpad.");
            if b.trigger.starts_with("corner.") {
                self.corners.enabled || numberpad
            } else if b.trigger.starts_with("sidedrag.") {
                self.side_drag.enabled
            } else {
                true
            }
        });
        if !self.area.enabled {
            c.area = Sides { enabled: false, top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };
        }
        c
    }
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"))
}

pub fn config_path() -> PathBuf {
    config_dir().join("kaget").join("config.toml")
}

pub fn save(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let body = toml::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    let text = format!("# KAGeT configuration. Edited by the KAGeT app.\n{body}");
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

/// Load without creating or importing anything (for `--replay`).
/// Parse a config file. Bindings of gestures that no longer exist (rotation, corner swipes off the
/// touchpad) are dropped, so they do not make the whole file invalid.
fn parse(text: &str) -> Result<Config, toml::de::Error> {
    let mut cfg: Config = toml::from_str(text)?;
    cfg.bindings.retain(|b| {
        let corner_swipe = b.trigger.starts_with("corner.") && b.trigger.contains(".swipe.");
        !(b.trigger.starts_with("rotate.") || (corner_swipe && Trigger::parse(&b.trigger).is_err()))
    });
    Ok(cfg)
}

pub fn load_readonly(path: &Path) -> Config {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| parse(&t).ok())
        .filter(|c| c.validate().is_ok())
        .unwrap_or_default()
}

/// Load the config; creates it (optionally importing the old asus-numberpad settings) if missing.
/// A broken file is left untouched and defaults are used.
pub fn load(path: &Path) -> Config {
    match std::fs::read_to_string(path) {
        Ok(text) => match parse(&text) {
            Ok(cfg) => match cfg.validate() {
                Ok(()) => cfg,
                Err(e) => {
                    eprintln!("config: {e}; using defaults (file left unchanged)");
                    Config::default()
                }
            },
            Err(e) => {
                eprintln!("config: parse error: {e}; using defaults (file left unchanged)");
                Config::default()
            }
        },
        Err(_) => {
            let cfg = import_legacy().unwrap_or_default();
            if let Err(e) = save(path, &cfg) {
                eprintln!("config: cannot write {}: {e}", path.display());
            }
            cfg
        }
    }
}

/// Import `~/.config/asus-numberpad/config.json` from the earlier Python daemon.
fn import_legacy() -> Option<Config> {
    let text = std::fs::read_to_string(config_dir().join("asus-numberpad").join("config.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut cfg = Config::default();
    if let Some(s) = v.get("slider_step_mm").and_then(|x| x.as_f64()) {
        cfg.slider.step_mm = s;
    }
    if let Some(s) = v.get("slider_continue_s").and_then(|x| x.as_f64()) {
        cfg.slider.continue_s = s;
    }
    let calc = v
        .get("calculator_command")
        .and_then(|c| c.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "kcalc".into());
    let gestures = v.get("gestures")?.as_object()?;
    let corner_bindings: Vec<Binding> = gestures
        .iter()
        .filter_map(|(k, a)| {
            let (corner, rest) = k.split_once('_')?;
            let corner = match corner {
                "topleft" => "top_left",
                "topright" => "top_right",
                _ => return None,
            };
            let trigger = if rest == "hold" {
                format!("corner.{corner}.hold")
            } else {
                format!("corner.{corner}.swipe.{}", rest.strip_prefix("swipe_")?)
            };
            Trigger::parse(&trigger).ok()?;
            let a = a.as_str()?;
            let action = match a {
                "toggle_numpad" => "numberpad.toggle".to_string(),
                "numpad_brightness_cycle" => "numberpad.brightness_cycle".into(),
                "numpad_brightness_up" => "numberpad.brightness_up".into(),
                "numpad_brightness_down" => "numberpad.brightness_down".into(),
                "calculator" => format!("cmd:{calc}"),
                "screen_brightness_up" => "brightness.up".into(),
                "screen_brightness_down" => "brightness.down".into(),
                "volume_up" => "volume.up".into(),
                "volume_down" => "volume.down".into(),
                "mute" => "volume.mute".into(),
                "play_pause" => "media.play_pause".into(),
                "next_track" => "media.next".into(),
                "previous_track" => "media.previous".into(),
                "none" => "none".into(),
                other if other.starts_with("cmd:") => other.into(),
                _ => return None,
            };
            Some(b(&trigger, &action))
        })
        .collect();
    cfg.bindings.retain(|x| !x.trigger.starts_with("corner."));
    let mut all = corner_bindings;
    all.append(&mut cfg.bindings);
    cfg.bindings = all;
    eprintln!("config: imported settings from the asus-numberpad daemon");
    Some(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_roundtrip() {
        let c = Config::default();
        c.validate().unwrap();
        let text = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn removed_gestures_do_not_break_an_old_file() {
        let text = "[gestures]\nrotate_deg = 25.0\n\n[[bindings]]\ntrigger = \"rotate.3.cw\"\naction = \"volume.up\"\n\n\
                    [[bindings]]\ntrigger = \"corner.top_left.swipe.up\"\naction = \"volume.up\"\n\n\
                    [[bindings]]\ntrigger = \"swipe.3.up\"\naction = \"kwin:Overview\"\n";
        let c = parse(text).unwrap();
        c.validate().unwrap();
        assert_eq!(c.bindings.len(), 1);
        assert_eq!(c.bindings[0].trigger, "swipe.3.up");
    }

    #[test]
    fn partial_file_gets_defaults() {
        let c: Config = toml::from_str("[area]\ntop = 10.0\n").unwrap();
        assert_eq!(c.area.top, 10.0);
        assert_eq!(c.area.right, 27.0);
        assert_eq!(c.bindings, default_bindings());
    }

    #[test]
    fn disabled_groups_are_filtered() {
        fn has(c: &Config, t: &str) -> bool {
            c.bindings.iter().any(|b| b.trigger == t)
        }
        let mut c = Config::default();
        c.bindings.push(b("sidedrag.top.3", "kwin:Overview"));
        assert!(!has(&c.effective(), "sidedrag.top.3"), "side drag is off by default");
        c.side_drag.enabled = true;
        assert!(has(&c.effective(), "sidedrag.top.3"));
        c.corners.enabled = false;
        let e = c.effective();
        assert!(!has(&e, "corner.top_left.swipe.right"));
        assert!(has(&e, "corner.top_right.hold"), "NumberPad toggle stays");
        c.area.enabled = false;
        assert_eq!(c.effective().area.right, 0.0);
        assert!(default_bindings().iter().all(|x| !x.trigger.starts_with("sidedrag.")));
    }

    #[test]
    fn panel_validation() {
        let mut c = Config::default();
        c.panel.controls = vec!["volume".into(), "volume".into()];
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.panel.controls = vec!["numpad".into()];
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.panel.scale = 3.0;
        assert!(c.validate().is_err());
        let c: Config = toml::from_str("[panel]\ncontrols = [\"volume\", \"scale\"]\n").unwrap();
        c.validate().unwrap();
        let mut c = Config::default();
        c.panel.controls = vec!["action:kwin:Overview".into(), "brightness".into(), "action:media.play_pause".into()];
        c.validate().unwrap();
        c.panel.controls.push("action:explode".into());
        assert!(c.validate().is_err());
        c.panel.controls = vec!["action:none".into()];
        assert!(c.validate().is_err());
        c.panel.controls = (0..7).map(|_| "volume".to_string()).collect();
        assert!(c.validate().unwrap_err().contains("at most"));
        assert_eq!(c.panel.position, "bottom");
    }

    #[test]
    fn bad_values_rejected() {
        let mut c = Config::default();
        c.area.left = -1.0;
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.bindings.push(b("corner.middle.hold", "none"));
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.bindings.push(b("tap.3", "launch-rockets"));
        assert!(c.validate().is_err());
    }
}
