//! Executor thread: runs actions (D-Bus calls and process spawns take 5–50 ms and must never block
//! input processing), NumberPad LED writes and the KWin pointer switch.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use zbus::blocking::Connection;
use zbus::zvariant::Value;

use crate::actions::{Action, StepKind};
use crate::config::Steps;
use crate::dbus::SignalMsg;

/// At most ~30 steps per second per action; queued steps are merged.
const MIN_INTERVAL: Duration = Duration::from_millis(33);
/// post_write_delay_ms of the NumberPad LED.
const LED_DELAY: Duration = Duration::from_millis(50);
/// Time a KWin helper script stays loaded.
const SCRIPT_LIFETIME: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub enum Request {
    Run { action: String, when_running: Option<String> },
    Led { hidraw: String, value: u8 },
    /// Enable/disable the touchpad pointer in KWin (`eventN`).
    Pointer { device: String, enabled: bool },
    Steps(Steps),
    Quit,
}

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

pub fn spawn(conn: Option<Connection>, steps: Steps, signals: Sender<SignalMsg>) -> Handle {
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("executor".into())
        .spawn(move || Executor { conn, steps, signals, last: HashMap::new(), scripts: Vec::new(), next_script: 0 }.run(rx))
        .expect("spawn executor thread");
    Handle { tx, thread: Some(thread) }
}

struct Executor {
    conn: Option<Connection>,
    steps: Steps,
    signals: Sender<SignalMsg>,
    last: HashMap<String, Instant>,
    /// Loaded KWin helper scripts and when to unload them.
    scripts: Vec<(String, Instant)>,
    next_script: u32,
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
            let first = match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(r) => r,
                Err(RecvTimeoutError::Timeout) => {
                    self.unload_scripts(false);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            };
            let mut batch = vec![first];
            batch.extend(rx.try_iter());
            let mut quit = false;
            for (req, n) in coalesce(batch) {
                match req {
                    Request::Quit => quit = true,
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
                    Request::Run { action, when_running } => {
                        self.rate_limit(&action);
                        if let Err(e) = self.execute(&action, when_running.as_deref(), n) {
                            eprintln!("action {action}: {e}");
                            let _ = self.signals.send(SignalMsg::ActionFailed(action, e));
                        }
                    }
                }
            }
            self.unload_scripts(false);
            if quit {
                break;
            }
        }
        self.unload_scripts(true);
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
            Action::None | Action::System | Action::Pause | Action::NumberPad(_) => Ok(()),
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
                const DEST: &str = "org.kde.Solid.PowerManagement";
                const PATH: &str = "/org/kde/Solid/PowerManagement/Actions/KeyboardBrightnessControl";
                const IFACE: &str = "org.kde.Solid.PowerManagement.Actions.KeyboardBrightnessControl";
                let get = |m: &str| -> Result<i32, String> {
                    self.call(DEST, PATH, IFACE, m, &())?.body().deserialize::<i32>().map_err(|e| e.to_string())
                };
                let max = get("keyboardBrightnessMax")?;
                let cur = get("keyboardBrightness")?;
                let target = (cur + count * self.steps.keyboard_backlight_levels as i32).clamp(0, max);
                if target != cur {
                    self.call(DEST, PATH, IFACE, "setKeyboardBrightness", &(target,))?;
                }
                Ok(())
            }
        }
    }

    /// `app:` with when_running close/focus/minimize/toggle: a short-lived KWin script looks up the
    /// app's windows and calls back `LaunchApp` when there are none.
    fn app_toggle(&mut self, id: &str, mode: &str) -> Result<(), String> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("kaget");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        self.next_script += 1;
        let name = format!("kaget-app-{}-{}", std::process::id(), self.next_script);
        let path = dir.join(format!("{name}.js"));
        std::fs::write(&path, kwin_script(id, mode)).map_err(|e| e.to_string())?;
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
        let exec = spawn(Some(conn), Steps::default(), sig);
        launch_app(&app).unwrap();
        std::thread::sleep(Duration::from_secs(4));
        assert!(running(), "{app} did not start");
        exec.tx.send(Request::Run { action: format!("app:{app}"), when_running: Some("close".into()) }).unwrap();
        std::thread::sleep(Duration::from_secs(3));
        assert!(!running(), "the KWin script did not close {app}");
        exec.join();
    }

    #[test]
    fn script_quotes_ids() {
        let s = kwin_script("org.kde.kcalc.desktop", "close");
        assert!(s.contains(r#"const id = "org.kde.kcalc";"#));
        assert!(s.contains(r#""LaunchApp", "org.kde.kcalc.desktop""#));
    }
}
