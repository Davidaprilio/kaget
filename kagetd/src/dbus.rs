//! D-Bus API `io.github.kaget.Daemon1`. Methods run on zbus' own thread; they read a
//! snapshot published by the main thread and send commands back to it.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use serde_json::json;
use zbus::blocking::Connection;
use zbus::object_server::SignalEmitter;

use crate::actions::{self, Action};
use crate::config::{self, Config};
use crate::executor::Request;
use crate::triggers::Trigger;

pub const NAME: &str = "io.github.kaget";
pub const PATH: &str = "/io/github/kaget/Daemon";
pub const IFACE: &str = "io.github.kaget.Daemon1";

/// Commands from D-Bus to the main thread.
#[derive(Debug)]
pub enum Command {
    Reload,
    SetEnabled(bool),
    SetLive(bool),
    /// Actions the main thread runs itself (keys, NumberPad, pause).
    RunLocal(String),
    /// Save `panel.scale` (the panel's own size slider).
    SetPanelScale(f64),
    /// The panel window opened or closed itself (shortcut, Esc, click elsewhere).
    SetPanelOpen(bool),
    /// The panel window is being used with the mouse.
    PanelActivity,
}

/// Signals from the daemon threads to the bus.
#[derive(Debug)]
pub enum SignalMsg {
    Touches(String),
    Gesture(String, String),
    StateChanged,
    ActionFailed(String, String),
    ControlsChanged(String),
    /// Panel mode: live panel size while a finger drags the `scale` column.
    PanelScale(f64),
    /// Panel mode: column under the finger (-1: none).
    PanelFocus(i32),
}

/// What D-Bus methods read; written by the main thread.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub state_json: String,
    pub config: Config,
    pub max_fingers: u32,
    pub semi_mt: bool,
    pub passive: bool,
    pub numberpad: bool,
    /// Global shortcut names reported by kglobalaccel (filled in the background at start-up).
    pub kwin_shortcuts: Vec<String>,
    pub plasma_shortcuts: Vec<String>,
}

pub type Shared = Arc<Mutex<Snapshot>>;

pub struct Daemon {
    pub shared: Shared,
    pub commands: Sender<Command>,
    pub executor: Sender<Request>,
    pub config_path: PathBuf,
    pub controls: crate::executor::Controls,
}

const CONTROL_KINDS: &[&str] = &["brightness", "volume", "keyboard_backlight"];

fn triggers_json(s: &Snapshot, only_system: bool) -> String {
    let v: Vec<_> = Trigger::all(s.max_fingers)
        .into_iter()
        .filter(|t| !only_system || t.system_owned())
        .map(|t| {
            if only_system {
                return json!({ "trigger": t.to_string(), "description": t.description() });
            }
            let (available, reason) = match t.available(s.max_fingers, s.semi_mt) {
                Err(r) => (false, r),
                Ok(()) if s.passive && t.system_owned() => (false, "Used by KDE (needs filter mode)".to_string()),
                Ok(()) if t.system_owned() && !s.config.general.override_system_gestures => {
                    (false, "Used by KDE (turn on “Override system gestures”)".to_string())
                }
                Ok(()) => (true, String::new()),
            };
            json!({
                "trigger": t.to_string(),
                "group": t.group(),
                "fingers": t.fingers(),
                "system_owned": t.system_owned(),
                "available": available,
                "reason": reason,
                "description": t.description(),
            })
        })
        .collect();
    serde_json::to_string(&v).unwrap_or_default()
}

#[zbus::interface(name = "io.github.kaget.Daemon1")]
impl Daemon {
    fn get_state(&self) -> String {
        self.shared.lock().unwrap().state_json.clone()
    }

    fn get_config(&self) -> String {
        serde_json::to_string(&self.shared.lock().unwrap().config).unwrap_or_default()
    }

    fn get_defaults(&self) -> String {
        serde_json::to_string(&Config::default()).unwrap_or_default()
    }

    /// Validate, save and apply. Returns "" or an error message.
    fn set_config(&self, json: String) -> String {
        let cfg: Config = match serde_json::from_str(&json) {
            Ok(c) => c,
            Err(e) => return format!("invalid config: {e}"),
        };
        if let Err(e) = cfg.validate() {
            return e;
        }
        if let Err(e) = config::save(&self.config_path, &cfg) {
            return format!("cannot write {}: {e}", self.config_path.display());
        }
        self.shared.lock().unwrap().config = cfg;
        let _ = self.commands.send(Command::Reload);
        String::new()
    }

    fn list_triggers(&self) -> String {
        triggers_json(&self.shared.lock().unwrap(), false)
    }

    fn list_actions(&self) -> String {
        let s = self.shared.lock().unwrap();
        serde_json::to_string(&actions::catalog(&s.kwin_shortcuts, &s.plasma_shortcuts, s.numberpad)).unwrap_or_default()
    }

    fn system_gestures(&self) -> String {
        triggers_json(&self.shared.lock().unwrap(), true)
    }

    /// Test an action. False if it does not parse.
    fn run_action(&self, action: String) -> bool {
        match Action::parse(&action) {
            Err(e) => {
                eprintln!("RunAction {action}: {e}");
                false
            }
            Ok(Action::Keys(_) | Action::NumberPad(_) | Action::Pause | Action::Panel) => {
                self.commands.send(Command::RunLocal(action)).is_ok()
            }
            Ok(_) => self.executor.send(Request::Run { action, when_running: None }).is_ok(),
        }
    }

    fn set_enabled(&self, enabled: bool) {
        let _ = self.commands.send(Command::SetEnabled(enabled));
    }

    fn set_live(&self, live: bool) {
        let _ = self.commands.send(Command::SetLive(live));
    }

    /// Last read control values: `{brightness, volume, keyboard_backlight}` each with `available`,
    /// `value` (percent) and extras (`muted`, `level`/`max`). Call `RefreshControls` for fresh values.
    fn get_controls(&self) -> String {
        let c = self.controls.lock().unwrap().clone();
        if c.is_empty() { "{}".into() } else { c }
    }

    /// Re-read the controls; the result arrives as `ControlsChanged`.
    fn refresh_controls(&self) {
        let _ = self.executor.send(Request::RefreshControls);
    }

    /// Set `brightness`, `volume` or `keyboard_backlight` to `value` percent (no OSD).
    fn set_control(&self, kind: String, value: f64) -> bool {
        if !CONTROL_KINDS.contains(&kind.as_str()) || !value.is_finite() {
            return false;
        }
        self.executor.send(Request::SetControl { kind, value }).is_ok()
    }

    /// Save the panel size; returns "" or an error.
    fn set_panel_scale(&self, scale: f64) -> String {
        let (lo, hi) = config::PANEL_SCALE;
        if !scale.is_finite() || !(lo..=hi).contains(&scale) {
            return format!("scale must be between {lo} and {hi}");
        }
        let _ = self.commands.send(Command::SetPanelScale(scale));
        String::new()
    }

    /// The panel window opened / closed on its own: panel mode follows it.
    fn set_panel_open(&self, open: bool) {
        let _ = self.commands.send(Command::SetPanelOpen(open));
    }

    /// The panel window is in use: keeps panel mode from timing out.
    fn panel_activity(&self) {
        let _ = self.commands.send(Command::PanelActivity);
    }

    /// Called by the KWin helper script when an app has no window yet.
    fn launch_app(&self, desktop_id: String) {
        let _ = self.executor.send(Request::Run { action: format!("app:{desktop_id}"), when_running: None });
    }

    #[zbus(signal)]
    async fn touches(emitter: &SignalEmitter<'_>, json: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn gesture(emitter: &SignalEmitter<'_>, trigger: &str, action: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn state_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn panel_scale(emitter: &SignalEmitter<'_>, scale: f64) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn panel_focus(emitter: &SignalEmitter<'_>, column: i32) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn controls_changed(emitter: &SignalEmitter<'_>, json: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_failed(emitter: &SignalEmitter<'_>, action: &str, error: &str) -> zbus::Result<()>;
}

/// Thread that emits signals, so the main thread never waits for the bus.
pub fn spawn_signals(conn: Connection) -> Sender<SignalMsg> {
    let (tx, rx) = mpsc::channel::<SignalMsg>();
    std::thread::Builder::new()
        .name("signals".into())
        .spawn(move || {
            for msg in rx {
                let none = None::<&str>;
                let r = match &msg {
                    SignalMsg::Touches(j) => conn.emit_signal(none, PATH, IFACE, "Touches", &(j.as_str(),)),
                    SignalMsg::Gesture(t, a) => conn.emit_signal(none, PATH, IFACE, "Gesture", &(t.as_str(), a.as_str())),
                    SignalMsg::StateChanged => conn.emit_signal(none, PATH, IFACE, "StateChanged", &()),
                    SignalMsg::ControlsChanged(j) => conn.emit_signal(none, PATH, IFACE, "ControlsChanged", &(j.as_str(),)),
                    SignalMsg::PanelScale(s) => conn.emit_signal(none, PATH, IFACE, "PanelScale", &(*s,)),
                    SignalMsg::PanelFocus(c) => conn.emit_signal(none, PATH, IFACE, "PanelFocus", &(*c,)),
                    SignalMsg::ActionFailed(a, e) => {
                        conn.emit_signal(none, PATH, IFACE, "ActionFailed", &(a.as_str(), e.as_str()))
                    }
                };
                if let Err(e) = r {
                    eprintln!("D-Bus signal failed: {e}");
                }
            }
        })
        .expect("spawn signal thread");
    tx
}

/// Names of a component's global shortcuts (empty if kglobalaccel does not know it).
pub fn shortcut_names(conn: &Connection, component: &str) -> Vec<String> {
    let path = crate::executor::component_path(component);
    conn.call_method(Some("org.kde.kglobalaccel"), path.as_str(), Some("org.kde.kglobalaccel.Component"), "shortcutNames", &())
        .ok()
        .and_then(|m| m.body().deserialize::<Vec<String>>().ok())
        .map(|mut v| {
            v.retain(|n| !n.trim().is_empty());
            v.sort();
            v
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passive_marks_system_gestures_unavailable() {
        let s = Snapshot { max_fingers: 5, passive: true, ..Default::default() };
        let v: Vec<serde_json::Value> = serde_json::from_str(&triggers_json(&s, false)).unwrap();
        let get = |t: &str| v.iter().find(|e| e["trigger"] == t).unwrap().clone();
        assert_eq!(get("swipe.3.left")["available"], false);
        assert_eq!(get("swipe.3.left")["reason"], "Used by KDE (needs filter mode)");
        assert_eq!(get("swipe.5.left")["available"], true);
        assert_eq!(get("corner.top_left.hold")["group"], "Corners");
        let mut f = Snapshot { max_fingers: 5, passive: false, ..Default::default() };
        let get_f = |f: &Snapshot, t: &str| {
            let v: Vec<serde_json::Value> = serde_json::from_str(&triggers_json(f, false)).unwrap();
            v.into_iter().find(|e| e["trigger"] == t).unwrap()["available"].clone()
        };
        assert_eq!(get_f(&f, "swipe.3.left"), false);
        f.config.general.override_system_gestures = true;
        assert_eq!(get_f(&f, "swipe.3.left"), true);
        let sys: Vec<serde_json::Value> = serde_json::from_str(&triggers_json(&s, true)).unwrap();
        assert!(sys.iter().all(|e| Trigger::parse(e["trigger"].as_str().unwrap()).unwrap().system_owned()));
    }
}
