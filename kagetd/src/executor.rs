//! Executor thread: runs actions (D-Bus calls and process spawns take 5–50 ms and must never block
//! input processing), NumberPad LED writes and the KWin pointer switch.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use zbus::blocking::Connection;
use zbus::zvariant::Value;

use crate::actions::{Action, StepKind};
use crate::config::Steps;
use crate::dbus::SignalMsg;
use serde_json::json;

/// At most ~30 steps per second per action; queued steps are merged.
const MIN_INTERVAL: Duration = Duration::from_millis(33);
/// post_write_delay_ms of the NumberPad LED.
const LED_DELAY: Duration = Duration::from_millis(50);
/// Time a KWin helper script stays loaded.
const SCRIPT_LIFETIME: Duration = Duration::from_secs(3);
/// `system:<n>:window`: after the fingers lift, KWin finishes the desktop slide; the carried window
/// is put on the desktop only then (earlier it would jump and slide in with it).
const CARRY_SETTLE: Duration = Duration::from_millis(600);

#[derive(Debug)]
pub enum Request {
    Run { action: String, when_running: Option<String> },
    Led { hidraw: String, value: u8 },
    /// Enable/disable the touchpad pointer in KWin (`eventN`).
    Pointer { device: String, enabled: bool },
    Steps(Steps),
    /// Read brightness / volume / keyboard backlight and publish them (`ControlsChanged`).
    RefreshControls,
    /// Set a control to an absolute value in percent (no KDE OSD), then publish.
    SetControl { kind: String, value: f64 },
    /// Panel mode: move a control by `delta` percent (queued moves are summed).
    AdjustControl { kind: String, delta: f64 },
    /// Show or hide the `kaget-panel` window (started when needed).
    PanelWindow(bool),
    /// `system:<n>:window`: KWin's desktop switch starts (keep the active window on all desktops)
    /// or its fingers lifted (put that window on the desktop KWin ends up on).
    CarryWindow(bool),
    /// The helper script pinned this window (KWin `internalId`).
    WindowCarried(String),
    Quit,
}

/// Latest control values as JSON, read by `GetControls`.
pub type Controls = Arc<Mutex<String>>;

pub struct Handle {
    pub tx: Sender<Request>,
    thread: Option<JoinHandle<()>>,
}

impl Handle {
    /// Finish queued requests (pointer restore, LED off) and stop.
    pub fn join(mut self) {
        let _ = self.tx.send(Request::Quit);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn spawn(conn: Option<Connection>, steps: Steps, signals: Sender<SignalMsg>, controls: Controls) -> Handle {
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("executor".into())
        .spawn(move || {
            Executor {
                conn,
                steps,
                signals,
                controls,
                values: HashMap::new(),
                targets: HashMap::new(),
                last: HashMap::new(),
                scripts: Vec::new(),
                next_script: 0,
                carried: None,
                drop_at: None,
            }
            .run(rx)
        })
        .expect("spawn executor thread");
    Handle { tx, thread: Some(thread) }
}

struct Executor {
    conn: Option<Connection>,
    steps: Steps,
    signals: Sender<SignalMsg>,
    controls: Controls,
    /// Last read values in percent, and where panel-mode moves are heading (kept unrounded so
    /// small moves add up, e.g. for the few keyboard backlight levels).
    values: HashMap<String, f64>,
    targets: HashMap<String, f64>,
    last: HashMap<String, Instant>,
    /// Loaded KWin helper scripts and when to unload them.
    scripts: Vec<(String, Instant)>,
    next_script: u32,
    /// `system:<n>:window`: the window pinned to all desktops, and when to let it go.
    carried: Option<String>,
    drop_at: Option<Instant>,
}

/// KDE's kglobalaccel object path for a component (`org.kde.kcalc.desktop` -> `org_kde_kcalc_desktop`).
pub fn component_path(component: &str) -> String {
    let c: String = component.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    format!("/component/{c}")
}

/// Start a process detached from the daemon (own session, no stdio) and reap it in the background.
fn spawn_detached(mut cmd: Command) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Merge runs of the same action that queued up while a previous one was executing.
fn coalesce(reqs: Vec<Request>) -> Vec<(Request, u32)> {
    let mut out: Vec<(Request, u32)> = Vec::new();
    for r in reqs {
        // a slider drag queues many values: only the latest one matters
        if let (Some((Request::SetControl { kind: k, value: v }, _)), Request::SetControl { kind, value }) =
            (out.last_mut(), &r)
            && k == kind
        {
            *v = *value;
            continue;
        }
        if let (Some((Request::AdjustControl { kind: k, delta: d }, _)), Request::AdjustControl { kind, delta }) =
            (out.last_mut(), &r)
            && k == kind
        {
            *d += *delta;
            continue;
        }
        if let (Some((Request::Run { action: a, when_running: w }, n)), Request::Run { action, when_running }) =
            (out.last_mut(), &r)
            && a == action
            && w == when_running
            && Action::parse(action).is_ok_and(|p| matches!(p, Action::Step { .. }))
        {
            *n += 1;
            continue;
        }
        out.push((r, 1));
    }
    out
}

impl Executor {
    fn run(mut self, rx: Receiver<Request>) {
        loop {
            let wait = self.drop_at.map_or(Duration::from_millis(500), |t| {
                t.saturating_duration_since(Instant::now()).min(Duration::from_millis(500))
            });
            let first = match rx.recv_timeout(wait) {
                Ok(r) => r,
                Err(RecvTimeoutError::Timeout) => {
                    self.drop_window(false);
                    self.unload_scripts(false);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            };
            let mut batch = vec![first];
            batch.extend(rx.try_iter());
            let mut quit = false;
            let mut publish = false;
            for (req, n) in coalesce(batch) {
                match req {
                    Request::Quit => quit = true,
                    Request::RefreshControls => {
                        self.targets.clear();
                        publish = true;
                    }
                    Request::AdjustControl { kind, delta } => {
                        if self.values.is_empty() {
                            self.publish_controls();
                        }
                        let from = self.targets.get(&kind).or_else(|| self.values.get(&kind)).copied();
                        if let Some(from) = from {
                            let target = (from + delta).clamp(0.0, 100.0);
                            self.targets.insert(kind.clone(), target);
                            if let Err(e) = self.set_control(&kind, target) {
                                eprintln!("panel {kind}: {e}");
                            }
                            publish = true;
                        }
                    }
                    Request::PanelWindow(open) => {
                        self.targets.clear();
                        if let Err(e) = self.panel_window(open) {
                            eprintln!("control panel: {e}");
                        }
                        if open {
                            publish = true;
                        }
                    }
                    Request::SetControl { kind, value } => {
                        if let Err(e) = self.set_control(&kind, value) {
                            eprintln!("panel {kind}: {e}");
                        }
                        publish = true;
                    }
                    Request::Steps(s) => self.steps = s,
                    Request::Led { hidraw, value } => {
                        if let Err(e) = crate::sys::hid_set_feature(&hidraw, &crate::numberpad::led_report(value)) {
                            eprintln!("NumberPad LED write failed: {e}");
                        }
                        std::thread::sleep(LED_DELAY);
                    }
                    Request::Pointer { device, enabled } => {
                        if let Err(e) = self.set_pointer(&device, enabled) {
                            eprintln!("KWin pointer switch failed: {e}");
                        }
                    }
                    Request::CarryWindow(true) => {
                        // swiping on before the last window was let go: it keeps riding along
                        if self.drop_at.take().is_none() || self.carried.is_none() {
                            self.carried = None;
                            if let Err(e) = self.load_script("carry", CARRY_SCRIPT) {
                                eprintln!("carry window: {e}");
                            }
                        }
                    }
                    Request::CarryWindow(false) => self.drop_at = Some(Instant::now() + CARRY_SETTLE),
                    Request::WindowCarried(id) => self.carried = Some(id),
                    Request::Run { action, when_running } => {
                        self.rate_limit(&action);
                        if let Err(e) = self.execute(&action, when_running.as_deref(), n) {
                            eprintln!("action {action}: {e}");
                            let _ = self.signals.send(SignalMsg::ActionFailed(action, e));
                        }
                    }
                }
            }
            if publish {
                self.publish_controls();
            }
            self.drop_window(quit);
            self.unload_scripts(false);
            if quit {
                break;
            }
        }
        self.unload_scripts(true);
    }

    /// Put the carried window on the current desktop once the slide settled (`now`: at once).
    fn drop_window(&mut self, now: bool) {
        if !now && self.drop_at.is_none_or(|t| t > Instant::now()) {
            return;
        }
        self.drop_at = None;
        if let Some(id) = self.carried.take()
            && let Err(e) = self.load_script("drop", &drop_script(&id))
        {
            eprintln!("carry window: {e}");
        }
    }

    fn rate_limit(&mut self, action: &str) {
        if let Some(t) = self.last.get(action) {
            let since = t.elapsed();
            if since < MIN_INTERVAL {
                std::thread::sleep(MIN_INTERVAL - since);
            }
        }
        self.last.insert(action.to_string(), Instant::now());
    }

    fn conn(&self) -> Result<&Connection, String> {
        self.conn.as_ref().ok_or_else(|| "no D-Bus session bus".to_string())
    }

    fn call<B>(&self, dest: &str, path: &str, iface: &str, method: &str, body: &B) -> Result<zbus::Message, String>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        self.conn()?.call_method(Some(dest), path, Some(iface), method, body).map_err(|e| e.to_string())
    }

    pub fn invoke_shortcut(&self, component: &str, name: &str) -> Result<(), String> {
        self.call("org.kde.kglobalaccel", &component_path(component), "org.kde.kglobalaccel.Component", "invokeShortcut", &(name,))
            .map(|_| ())
    }

    fn set_pointer(&self, device: &str, enabled: bool) -> Result<(), String> {
        set_pointer(self.conn()?, device, enabled)
    }

    fn execute(&mut self, action: &str, when_running: Option<&str>, n: u32) -> Result<(), String> {
        match Action::parse(action)? {
            Action::None | Action::System | Action::SystemAs { .. } | Action::Pause | Action::NumberPad(_) => Ok(()),
            Action::Panel => Ok(()), // panel mode lives in the engine
            Action::Keys(_) => Err("key actions are sent by the main thread".into()),
            Action::Global { component, name } => {
                for _ in 0..n {
                    self.invoke_shortcut(&component, &name)?;
                }
                Ok(())
            }
            Action::Step { kind, dir } => self.step(kind, dir * n as i32),
            Action::App(id) => match when_running.unwrap_or("new") {
                "new" => launch_app(&id),
                mode => self.app_toggle(&id, mode),
            },
            Action::Command(text) => {
                let mut c = Command::new("sh");
                c.arg("-c").arg(text);
                spawn_detached(c)
            }
        }
    }

    /// Volume / brightness / keyboard backlight by `count` steps (negative = down).
    fn step(&self, kind: StepKind, count: i32) -> Result<(), String> {
        let up = count > 0;
        let times = count.unsigned_abs();
        match kind {
            StepKind::Volume => {
                let p = self.steps.volume_percent;
                if (p - 1.0).abs() < 1e-9 {
                    // Plasma's small step is 1 % and shows the OSD
                    let name = if up { "increase_volume_small" } else { "decrease_volume_small" };
                    for _ in 0..times {
                        self.invoke_shortcut("kmix", name)?;
                    }
                    Ok(())
                } else {
                    let arg = format!("{}%{}", p * times as f64, if up { "+" } else { "-" });
                    let out = Command::new("wpctl")
                        .args(["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", &arg])
                        .output()
                        .map_err(|e| format!("wpctl: {e}"))?;
                    if out.status.success() {
                        Ok(())
                    } else {
                        Err(format!("wpctl: {}", String::from_utf8_lossy(&out.stderr).trim()))
                    }
                }
            }
            StepKind::Brightness => {
                let delta = self.steps.brightness_percent * count as f64 / 100.0;
                // flags 0: show the OSD (0x1 would suppress it)
                let r = self.call(
                    "org.kde.ScreenBrightness",
                    "/org/kde/ScreenBrightness",
                    "org.kde.ScreenBrightness",
                    "AdjustBrightnessRatio",
                    &(delta, 0u32),
                );
                if r.is_ok() {
                    return Ok(());
                }
                let name = if up { "Increase Screen Brightness Small" } else { "Decrease Screen Brightness Small" };
                for _ in 0..times {
                    self.invoke_shortcut("org_kde_powerdevil", name)?;
                }
                Ok(())
            }
            StepKind::KeyboardBacklight => {
                let max = self.kbd_call("keyboardBrightnessMax")?;
                let cur = self.kbd_call("keyboardBrightness")?;
                let target = (cur + count * self.steps.keyboard_backlight_levels as i32).clamp(0, max);
                if target != cur {
                    self.call(KBD_DEST, KBD_PATH, KBD_IFACE, "setKeyboardBrightness", &(target,))?;
                }
                Ok(())
            }
        }
    }

    /// Show / hide `kaget-panel` over D-Bus; start it (it shows itself) when it is not running yet.
    fn panel_window(&self, open: bool) -> Result<(), String> {
        let method = if open { "Show" } else { "Hide" };
        if self.call(PANEL_NAME, PANEL_PATH, PANEL_IFACE, method, &()).is_ok() || !open {
            return Ok(());
        }
        // next to kagetd (development builds) or on PATH
        let local = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("kaget-panel"))).filter(|p| p.exists());
        spawn_detached(Command::new(local.unwrap_or_else(|| "kaget-panel".into())))
    }

    /// The screen KDE treats as the built-in one (or the first).
    fn display(&self) -> Result<String, String> {
        let v = self.property("org.kde.ScreenBrightness", "/org/kde/ScreenBrightness", "org.kde.ScreenBrightness", "DisplaysDBusNames")?;
        let names: Vec<String> = v.try_into().map_err(|e: zbus::zvariant::Error| e.to_string())?;
        let path = |n: &String| format!("/org/kde/ScreenBrightness/{n}");
        let internal = names.iter().find(|n| {
            self.property("org.kde.ScreenBrightness", &path(n), "org.kde.ScreenBrightness.Display", "IsInternal")
                .ok()
                .and_then(|v| bool::try_from(v).ok())
                .unwrap_or(false)
        });
        internal.or(names.first()).map(path).ok_or_else(|| "no display with adjustable brightness".into())
    }

    fn property(&self, dest: &str, path: &str, iface: &str, name: &str) -> Result<zbus::zvariant::OwnedValue, String> {
        self.call(dest, path, "org.freedesktop.DBus.Properties", "Get", &(iface, name))?
            .body()
            .deserialize::<zbus::zvariant::OwnedValue>()
            .map_err(|e| e.to_string())
    }

    fn int_property(&self, dest: &str, path: &str, iface: &str, name: &str) -> Result<i32, String> {
        i32::try_from(self.property(dest, path, iface, name)?).map_err(|e| e.to_string())
    }

    fn kbd_call(&self, method: &str) -> Result<i32, String> {
        self.call(KBD_DEST, KBD_PATH, KBD_IFACE, method, &())?.body().deserialize::<i32>().map_err(|e| e.to_string())
    }

    /// Current values in percent; a control that cannot be read is `available: false`.
    fn read_controls(&self) -> serde_json::Value {
        let brightness = self.display().and_then(|d| {
            let iface = "org.kde.ScreenBrightness.Display";
            let max = self.int_property("org.kde.ScreenBrightness", &d, iface, "MaxBrightness")?;
            let cur = self.int_property("org.kde.ScreenBrightness", &d, iface, "Brightness")?;
            Ok(json!({"available": max > 0, "value": pct(cur, max)}))
        });
        let volume = read_volume().map(|(v, muted)| json!({"available": true, "value": v, "muted": muted}));
        let kbd = self.kbd_call("keyboardBrightnessMax").and_then(|max| {
            let cur = self.kbd_call("keyboardBrightness")?;
            Ok(json!({"available": max > 0, "value": pct(cur, max), "level": cur, "max": max}))
        });
        let or_missing = |r: Result<serde_json::Value, String>| r.unwrap_or_else(|e| json!({"available": false, "error": e}));
        json!({"brightness": or_missing(brightness), "volume": or_missing(volume), "keyboard_backlight": or_missing(kbd)})
    }

    fn publish_controls(&mut self) {
        let v = self.read_controls();
        for (k, c) in v.as_object().into_iter().flatten() {
            if let Some(x) = c.get("value").and_then(|x| x.as_f64()) {
                self.values.insert(k.clone(), x);
            }
        }
        let j = v.to_string();
        *self.controls.lock().unwrap() = j.clone();
        let _ = self.signals.send(SignalMsg::ControlsChanged(j));
    }

    fn set_control(&self, kind: &str, value: f64) -> Result<(), String> {
        let value = value.clamp(0.0, 100.0);
        match kind {
            "brightness" => {
                let d = self.display()?;
                let max = self.int_property("org.kde.ScreenBrightness", &d, "org.kde.ScreenBrightness.Display", "MaxBrightness")?;
                // never fully dark: 1 % at least
                let v = ((value / 100.0 * max as f64).round() as i32).clamp((max / 100).max(1), max);
                // flags 0x1: suppress the OSD, the panel shows the value itself
                self.call("org.kde.ScreenBrightness", &d, "org.kde.ScreenBrightness.Display", "SetBrightness", &(v, 1u32)).map(|_| ())
            }
            "volume" => {
                let arg = format!("{:.3}", value / 100.0);
                let out = Command::new("wpctl")
                    .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &arg])
                    .output()
                    .map_err(|e| format!("wpctl: {e}"))?;
                if out.status.success() { Ok(()) } else { Err(format!("wpctl: {}", String::from_utf8_lossy(&out.stderr).trim())) }
            }
            "keyboard_backlight" => {
                let max = self.kbd_call("keyboardBrightnessMax")?;
                let level = (value / 100.0 * max as f64).round() as i32;
                self.call(KBD_DEST, KBD_PATH, KBD_IFACE, "setKeyboardBrightnessSilent", &(level.clamp(0, max),)).map(|_| ())
            }
            other => Err(format!("unknown control '{other}'")),
        }
    }

    /// `app:` with when_running close/focus/minimize/toggle: a short-lived KWin script looks up the
    /// app's windows and calls back `LaunchApp` when there are none.
    fn app_toggle(&mut self, id: &str, mode: &str) -> Result<(), String> {
        self.load_script("app", &kwin_script(id, mode))
    }

    /// Load and run a short-lived KWin helper script (unloaded after `SCRIPT_LIFETIME`).
    fn load_script(&mut self, kind: &str, source: &str) -> Result<(), String> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("kaget");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        self.next_script += 1;
        let name = format!("kaget-{kind}-{}-{}", std::process::id(), self.next_script);
        let path = dir.join(format!("{name}.js"));
        std::fs::write(&path, source).map_err(|e| e.to_string())?;
        let path_s = path.to_string_lossy().into_owned();
        let reply = self.call("org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting", "loadScript", &(path_s.as_str(), name.as_str()))?;
        let script_id: i32 = reply.body().deserialize().map_err(|e| e.to_string())?;
        if script_id < 0 {
            return Err("KWin refused the helper script".into());
        }
        self.scripts.push((name, Instant::now() + SCRIPT_LIFETIME));
        // Plasma 6 exposes /Scripting/Script<N>; older versions /<N>
        let run = |p: String| self.call("org.kde.KWin", &p, "org.kde.kwin.Script", "run", &());
        run(format!("/Scripting/Script{script_id}")).or_else(|_| run(format!("/{script_id}"))).map(|_| ())
    }

    fn unload_scripts(&mut self, all: bool) {
        let now = Instant::now();
        let (expired, keep): (Vec<_>, Vec<_>) = self.scripts.drain(..).partition(|(_, t)| all || *t <= now);
        self.scripts = keep;
        for (name, _) in expired {
            let _ = self.call("org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting", "unloadScript", &(name.as_str(),));
            if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
                let _ = std::fs::remove_file(std::path::Path::new(&dir).join("kaget").join(format!("{name}.js")));
            }
        }
    }
}

pub const PANEL_NAME: &str = "io.github.kaget.Panel";
pub const PANEL_PATH: &str = "/Panel";
pub const PANEL_IFACE: &str = "io.github.kaget.Panel1";

const KBD_DEST: &str = "org.kde.Solid.PowerManagement";
const KBD_PATH: &str = "/org/kde/Solid/PowerManagement/Actions/KeyboardBrightnessControl";
const KBD_IFACE: &str = "org.kde.Solid.PowerManagement.Actions.KeyboardBrightnessControl";

fn pct(cur: i32, max: i32) -> f64 {
    if max <= 0 { 0.0 } else { (cur as f64 * 1000.0 / max as f64).round() / 10.0 }
}

/// `wpctl get-volume` prints `Volume: 0.35` or `Volume: 0.35 [MUTED]`.
fn parse_volume(s: &str) -> Option<(f64, bool)> {
    let v: f64 = s.split_whitespace().nth(1)?.parse().ok()?;
    Some(((v * 1000.0).round() / 10.0, s.contains("[MUTED]")))
}

fn read_volume() -> Result<(f64, bool), String> {
    let out = Command::new("wpctl").args(["get-volume", "@DEFAULT_AUDIO_SINK@"]).output().map_err(|e| format!("wpctl: {e}"))?;
    parse_volume(&String::from_utf8_lossy(&out.stdout)).ok_or_else(|| "wpctl: no default audio sink".into())
}

pub fn launch_app(id: &str) -> Result<(), String> {
    // kstart wants the service name without the suffix ("No such service" otherwise)
    let mut c = Command::new("kstart");
    c.arg("--application").arg(id.strip_suffix(".desktop").unwrap_or(id));
    spawn_detached(c)
}

pub fn set_pointer(conn: &Connection, device: &str, enabled: bool) -> Result<(), String> {
    let path = format!("/org/kde/KWin/InputDevice/{device}");
    conn.call_method(
        Some("org.kde.KWin"),
        path.as_str(),
        Some("org.freedesktop.DBus.Properties"),
        "Set",
        &("org.kde.KWin.InputDevice", "enabled", Value::from(enabled)),
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// KWin 6 script for `system:<n>:window`: the active window stays on screen while the desktop
/// slides (windows on all desktops do not move) and is reported back for `drop_script`.
const CARRY_SCRIPT: &str = r#"const w = workspace.activeWindow;
if (w && w.normalWindow && !w.onAllDesktops) {
    w.onAllDesktops = true;
    callDBus("io.github.kaget", "/io/github/kaget/Daemon", "io.github.kaget.Daemon1", "WindowCarried", w.internalId.toString());
}
"#;

/// KWin 6 script: the carried window lives on the current desktop only again.
pub fn drop_script(id: &str) -> String {
    format!(
        r#"const id = {id};
const w = workspace.windowList().find(w => w.internalId.toString() === id);
if (w) {{
    w.desktops = [workspace.currentDesktop];
}}
"#,
        id = js_string(id),
    )
}

/// KWin 6 script for `app:` toggling. `desktopFileName` has no `.desktop` suffix.
pub fn kwin_script(id: &str, mode: &str) -> String {
    let base = id.strip_suffix(".desktop").unwrap_or(id);
    format!(
        r#"const id = {base};
const mode = {mode};
const wins = workspace.windowList().filter(w => w.desktopFileName === id && w.normalWindow);
if (wins.length === 0) {{
    callDBus("io.github.kaget", "/io/github/kaget/Daemon", "io.github.kaget.Daemon1", "LaunchApp", {full});
}} else if (mode === "close") {{
    wins.forEach(w => w.closeWindow());
}} else if (mode === "minimize") {{
    wins.forEach(w => {{ w.minimized = true; }});
}} else {{
    const active = wins.indexOf(workspace.activeWindow) >= 0;
    if (mode === "toggle" && active) {{
        wins.forEach(w => {{ w.minimized = true; }});
    }} else {{
        wins[0].minimized = false;
        workspace.activeWindow = wins[0];
    }}
}}
"#,
        base = js_string(base),
        mode = js_string(mode),
        full = js_string(id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_paths() {
        assert_eq!(component_path("org.kde.kcalc.desktop"), "/component/org_kde_kcalc_desktop");
        assert_eq!(component_path("kwin"), "/component/kwin");
    }

    #[test]
    fn coalesces_adjacent_steps_only() {
        let run = |a: &str| Request::Run { action: a.into(), when_running: None };
        let c = coalesce(vec![run("volume.up"), run("volume.up"), run("volume.down"), run("kwin:Overview"), run("kwin:Overview")]);
        let counts: Vec<u32> = c.iter().map(|(_, n)| *n).collect();
        assert_eq!(counts, [2, 1, 1, 1]);
    }

    /// Desktop test (`cargo test -- --ignored app_toggle`): starts an app, then closes it through
    /// the KWin helper script. App and process name: KAGET_TEST_APP / KAGET_TEST_PROC
    /// (default KInfoCenter; pick one that is not running, its windows get closed).
    #[test]
    #[ignore]
    fn app_toggle_closes_app_on_the_desktop() {
        let app = std::env::var("KAGET_TEST_APP").unwrap_or_else(|_| "org.kde.kinfocenter.desktop".into());
        let proc_name = std::env::var("KAGET_TEST_PROC").unwrap_or_else(|_| "kinfocenter".into());
        let running = || Command::new("pgrep").args(["-x", &proc_name]).output().unwrap().status.success();
        assert!(!running(), "{proc_name} is running; close it or pick another app");
        let conn = Connection::session().unwrap();
        let (sig, _rx) = mpsc::channel();
        let exec = spawn(Some(conn), Steps::default(), sig, Controls::default());
        launch_app(&app).unwrap();
        std::thread::sleep(Duration::from_secs(4));
        assert!(running(), "{app} did not start");
        exec.tx.send(Request::Run { action: format!("app:{app}"), when_running: Some("close".into()) }).unwrap();
        std::thread::sleep(Duration::from_secs(3));
        assert!(!running(), "the KWin script did not close {app}");
        exec.join();
    }

    #[test]
    fn coalesces_slider_drags_to_the_last_value() {
        let set = |k: &str, v| Request::SetControl { kind: k.into(), value: v };
        let c = coalesce(vec![set("volume", 10.0), set("volume", 20.0), set("brightness", 5.0), set("volume", 30.0)]);
        let v: Vec<(String, f64)> = c
            .into_iter()
            .map(|(r, _)| match r {
                Request::SetControl { kind, value } => (kind, value),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(v, [("volume".into(), 20.0), ("brightness".into(), 5.0), ("volume".into(), 30.0)]);
        let adj = |k: &str, d| Request::AdjustControl { kind: k.into(), delta: d };
        let c = coalesce(vec![adj("volume", 1.5), adj("volume", 2.0), adj("volume", -0.5)]);
        assert!(matches!(&c[..], [(Request::AdjustControl { delta, .. }, _)] if (*delta - 3.0).abs() < 1e-9));
    }

    #[test]
    fn volume_and_percent_parsing() {
        assert_eq!(parse_volume("Volume: 0.35\n"), Some((35.0, false)));
        assert_eq!(parse_volume("Volume: 1.00 [MUTED]"), Some((100.0, true)));
        assert_eq!(parse_volume(""), None);
        assert_eq!(pct(1, 3), 33.3);
        assert_eq!(pct(5000, 10000), 50.0);
    }

    #[test]
    fn carry_scripts() {
        assert!(CARRY_SCRIPT.contains(r#""WindowCarried", w.internalId.toString()"#));
        let s = drop_script("{0a1b-2c}");
        assert!(s.contains(r#"const id = "{0a1b-2c}";"#));
        assert!(s.contains("w.desktops = [workspace.currentDesktop];"));
    }

    #[test]
    fn script_quotes_ids() {
        let s = kwin_script("org.kde.kcalc.desktop", "close");
        assert!(s.contains(r#"const id = "org.kde.kcalc";"#));
        assert!(s.contains(r#""LaunchApp", "org.kde.kcalc.desktop""#));
    }
}
