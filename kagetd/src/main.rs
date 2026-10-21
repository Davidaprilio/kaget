//! kagetd – KAGeT touchpad gesture daemon. Passive mode reads the touchpad alongside KDE; filter mode
//! grabs it and re-emits it through a clone so KAGeT gestures can be withheld (and override KDE's).

mod actions;
mod config;
mod dbus;
mod device;
mod engine;
mod executor;
mod filter;
mod geometry;
mod keys;
mod npad;
mod numberpad;
mod reader;
mod record;
mod sys;
mod tracker;
mod triggers;
mod uinput;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use serde_json::json;

use actions::Action;
use config::Config;
use dbus::{Command, Shared, SignalMsg, Snapshot};
use device::TouchpadInfo;
use engine::Engine;
use engine::outputs::{Fired, Outputs};
use executor::Request;
use filter::Filter;
use geometry::Geometry;
use npad::NumberPad;
use reader::{Frame, Reader};
use tracker::Tracker;
use triggers::{CORNERS, EDGES};
use uinput::{VirtualKeyboard, VirtualTouchpad};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const POLL_MS: i32 = 10;
const CONFIG_POLL_S: f64 = 2.0;
const RECONNECT_S: f64 = 1.0;
const TOUCHES_INTERVAL_S: f64 = 1.0 / 60.0;

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

fn install_signal_handlers() {
    let handler = on_signal as extern "C" fn(libc::c_int);
    unsafe {
        libc::signal(libc::SIGTERM, handler as libc::sighandler_t);
        libc::signal(libc::SIGINT, handler as libc::sighandler_t);
    }
}

fn usage() {
    println!(
        "kagetd {VERSION} – KAGeT touchpad gesture daemon

Usage:
  kagetd                  run the daemon
  kagetd --probe          print touches in mm with the zone under them
  kagetd --record FILE    record raw touchpad frames (JSON lines) until Ctrl+C
  kagetd --replay FILE    run a recording through the engine and print the gestures
  kagetd --restore        re-enable the touchpad pointer in KWin and switch the NumberPad LED off
  kagetd --version"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        None | Some("run") => run(),
        Some("--probe") => probe(),
        Some("--record") if args.len() == 2 => record(&args[1]),
        Some("--replay") if args.len() == 2 => replay(&args[1]),
        Some("--restore") => restore(),
        Some("--version") => {
            println!("kagetd {VERSION}");
            0
        }
        Some("-h" | "--help") => {
            usage();
            0
        }
        _ => {
            usage();
            2
        }
    };
    std::process::exit(code);
}

fn node_name(info: &TouchpadInfo) -> String {
    info.path.rsplit('/').next().unwrap_or_default().to_string()
}

fn uinput_writable() -> bool {
    let c = std::ffi::CString::new("/dev/uinput").unwrap();
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

/// Keys the virtual keyboard must declare: the whole table plus the NumberPad keys.
fn keyboard_codes() -> Vec<u16> {
    let mut set: BTreeSet<u16> = keys::all_codes().into_iter().collect();
    set.insert(npad::KEY_NUMLOCK);
    set.into_iter().collect()
}

fn describe(info: &TouchpadInfo, size: (f64, f64)) -> String {
    format!(
        "{} [{}] {} – {:.1} x {:.1} mm, resolution {}x{} units/mm, {} fingers{}",
        info.name,
        info.hid_id(),
        info.path,
        size.0,
        size.1,
        info.x.resolution,
        info.y.resolution,
        info.max_fingers,
        if info.semi_mt { ", semi-multitouch" } else { "" }
    )
}

// ---- daemon ---------------------------------------------------------------------------------------

/// The real outputs of the main thread.
struct MainOut<'a> {
    kb: Option<&'a VirtualKeyboard>,
    exec: &'a Sender<Request>,
    signals: &'a Sender<SignalMsg>,
    hidraw: Option<&'a str>,
    node: &'a str,
    /// Filter mode: NumberPad touches are withheld, the pointer never needs switching off.
    filter: bool,
    dirty: bool,
    /// Tracking ids the engine handed back to the desktop (filter mode).
    replays: Vec<i32>,
}

impl<'a> MainOut<'a> {
    fn new(
        kb: Option<&'a VirtualKeyboard>,
        exec: &'a Sender<Request>,
        signals: &'a Sender<SignalMsg>,
        hidraw: Option<&'a str>,
        node: &'a str,
        filter: bool,
    ) -> Self {
        Self { kb, exec, signals, hidraw, node, filter, dirty: false, replays: Vec::new() }
    }
}

impl Outputs for MainOut<'_> {
    fn fired(&mut self, f: &Fired) {
        eprintln!("{} -> {}", f.trigger, f.action);
        let _ = self.signals.send(SignalMsg::Gesture(f.trigger.clone(), f.action.clone()));
        if f.local {
            return;
        }
        if let Ok(Action::Keys(codes)) = Action::parse(&f.action) {
            match self.kb {
                Some(kb) => {
                    if let Err(e) = kb.tap(&codes) {
                        eprintln!("virtual keyboard: {e}");
                    }
                }
                None => {
                    let _ = self.signals.send(SignalMsg::ActionFailed(f.action.clone(), "/dev/uinput is not writable".into()));
                }
            }
            return;
        }
        let _ = self.exec.send(Request::Run { action: f.action.clone(), when_running: f.when_running.clone() });
    }

    fn key(&mut self, codes: &[u16], down: bool) {
        if let Some(kb) = self.kb {
            let evs: Vec<(u16, i32)> = codes.iter().map(|&c| (c, down as i32)).collect();
            if let Err(e) = kb.send(&evs) {
                eprintln!("virtual keyboard: {e}");
            }
        }
    }

    fn led(&mut self, value: u8) {
        if let Some(h) = self.hidraw {
            let _ = self.exec.send(Request::Led { hidraw: h.to_string(), value });
        }
    }

    fn pointer(&mut self, enabled: bool) {
        if self.filter && !enabled {
            return;
        }
        let _ = self.exec.send(Request::Pointer { device: self.node.to_string(), enabled });
    }

    fn state_changed(&mut self) {
        self.dirty = true;
    }

    fn replay(&mut self, tids: &[i32]) {
        self.replays.extend_from_slice(tids);
    }
}

/// A connected touchpad.
struct Pad {
    reader: Reader,
    tracker: Tracker,
    engine: Engine,
    hidraw: Option<String>,
    node: String,
    size: (f64, f64),
    grabbed_elsewhere: bool,
    /// Filter mode: the grab is held and touches go through this clone.
    filter: Option<Filter<VirtualTouchpad>>,
    /// Why filter mode was requested but is not running.
    mode_error: Option<String>,
}

/// Filter mode: create the clone first, then grab the real device.
fn start_filter(reader: &Reader, info: &TouchpadInfo) -> Result<Filter<VirtualTouchpad>, String> {
    let clone = VirtualTouchpad::new(info).map_err(|e| format!("cannot create the touchpad clone: {e} (needs /dev/uinput)"))?;
    reader.grab(true).map_err(|e| format!("cannot grab the touchpad: {e}"))?;
    Ok(Filter::new(clone, info.slots as usize, &info.keys))
}

struct Daemon {
    cfg: Config,
    cfg_path: PathBuf,
    cfg_mtime: Option<std::time::SystemTime>,
    kb: Option<VirtualKeyboard>,
    numlock: bool,
    exec: Sender<Request>,
    signals: Sender<SignalMsg>,
    shared: Shared,
    commands: Receiver<Command>,
    pad: Option<Pad>,
    live: bool,
    last_touches: f64,
    touches_empty_sent: bool,
    dirty: bool,
}

fn mtime(p: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl Daemon {
    fn geometry(cfg: &Config, info: &TouchpadInfo) -> ((f64, f64), Geometry) {
        let size = info.size_mm((cfg.device.width_mm, cfg.device.height_mm));
        (size, Geometry::new(info.x.minimum, info.x.maximum, info.y.minimum, info.y.maximum, size, cfg))
    }

    fn connect(&mut self, info: TouchpadInfo) {
        let (size, geo) = Self::geometry(&self.cfg.effective(), &info);
        eprintln!("touchpad: {}", describe(&info, size));
        let profile = if self.cfg.numberpad.enabled { numberpad::profile_for(&info) } else { None };
        let hidraw = profile.as_ref().and_then(|_| device::find_hidraw(&info));
        if let Some(p) = &profile {
            eprintln!(
                "NumberPad: {} ({})",
                p.model,
                hidraw.as_deref().unwrap_or("no hidraw node, LED disabled")
            );
        }
        let node = node_name(&info);
        let reader = match Reader::open(info.clone()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("touchpad: cannot open {}: {e}", info.path);
                return;
            }
        };
        let grabbed_elsewhere = reader.grabbed_elsewhere();
        if grabbed_elsewhere {
            eprintln!(
                "touchpad: another process holds an exclusive grab, no touches will arrive \
                 (stop the other touchpad tool)"
            );
        }
        let (filter, mode_error) = if !self.cfg.filter_mode() {
            (None, None)
        } else if grabbed_elsewhere {
            (None, Some("another process holds an exclusive grab".to_string()))
        } else {
            match start_filter(&reader, &info) {
                Ok(f) => {
                    eprintln!("touchpad: filter mode (grabbed, re-emitted through a clone)");
                    (Some(f), None)
                }
                Err(e) => {
                    eprintln!("touchpad: {e}; falling back to passive mode");
                    (None, Some(e))
                }
            }
        };
        let mut engine =
            Engine::new(self.cfg.effective(), geo, info.max_fingers, info.semi_mt, profile.map(NumberPad::new));
        engine.set_filter(filter.is_some());
        let mut out = MainOut::new(self.kb.as_ref(), &self.exec, &self.signals, hidraw.as_deref(), &node, filter.is_some());
        if let Some(np) = &engine.npad {
            np.start(&mut out);
        }
        engine.set_numlock(self.numlock, &mut out);
        self.pad = Some(Pad {
            reader,
            tracker: Tracker::default(),
            engine,
            hidraw,
            node,
            size,
            grabbed_elsewhere,
            filter,
            mode_error,
        });
        self.dirty = true;
    }

    fn disconnect(&mut self, why: &str) {
        if let Some(mut pad) = self.pad.take() {
            eprintln!("touchpad: {why}; waiting for it to come back");
            let mut out = MainOut::new(self.kb.as_ref(), &self.exec, &self.signals, pad.hidraw.as_deref(), &pad.node, pad.filter.is_some());
            pad.engine.reset(&mut out);
            if let Some(f) = pad.filter.as_mut() {
                f.reset();
            }
        }
        self.dirty = true;
    }

    fn reload(&mut self, cfg: Config) {
        self.cfg_mtime = mtime(&self.cfg_path);
        let _ = self.exec.send(Request::Steps(cfg.steps.clone()));
        let reconnect = self.pad.as_ref().is_some_and(|p| {
            p.engine.npad.is_some() != cfg.numberpad.enabled || self.cfg.filter_mode() != cfg.filter_mode()
        });
        self.cfg = cfg;
        if let Some(pad) = self.pad.as_mut() {
            let (size, geo) = Self::geometry(&self.cfg.effective(), &pad.reader.info);
            pad.size = size;
            let mut out = MainOut::new(self.kb.as_ref(), &self.exec, &self.signals, pad.hidraw.as_deref(), &pad.node, pad.filter.is_some());
            pad.engine.reconfigure(self.cfg.effective(), geo, &mut out);
        }
        if reconnect && let Some(mut pad) = self.pad.take() {
            // NumberPad or filter mode switched in the config: rebuild the engine
            let mut out = MainOut::new(self.kb.as_ref(), &self.exec, &self.signals, pad.hidraw.as_deref(), &pad.node, pad.filter.is_some());
            pad.engine.shutdown(&mut out);
            if let Some(f) = pad.filter.as_mut() {
                f.reset();
            }
            let info = pad.reader.info.clone();
            drop(pad); // releases the grab and destroys the clone
            self.connect(info);
        }
        eprintln!("config: applied");
        self.dirty = true;
    }

    fn state_json(&self) -> String {
        let mode = if self.pad.as_ref().is_some_and(|p| p.filter.is_some()) { "filter" } else { "passive" };
        let mode_error = self.pad.as_ref().and_then(|p| p.mode_error.clone());
        let (device, numberpad, enabled, paused) = match &self.pad {
            Some(p) => {
                let i = &p.reader.info;
                let e = &p.engine;
                let device = json!({
                    "name": i.name,
                    "vendor": i.id.vendor,
                    "product": i.id.product,
                    "path": i.path,
                    "sysfs": i.sysfs,
                    "slots": i.slots,
                    "width_mm": (p.size.0 * 10.0).round() / 10.0,
                    "height_mm": (p.size.1 * 10.0).round() / 10.0,
                    "max_fingers": i.max_fingers,
                    "semi_mt": i.semi_mt,
                    "resolution_x": i.x.resolution,
                    "resolution_y": i.y.resolution,
                    "grabbed_elsewhere": p.grabbed_elsewhere,
                });
                let np = match &e.npad {
                    Some(np) => {
                        let r = |v: f64| (v * 10.0).round() / 10.0;
                        let keys: Vec<_> = np
                            .profile
                            .keys
                            .iter()
                            .map(|k| {
                                let (x0, y0) = e.geo.mm(k.x0, k.y0);
                                let (x1, y1) = e.geo.mm(k.x1, k.y1);
                                json!({"x0": r(x0), "y0": r(y0), "x1": r(x1), "y1": r(y1), "label": k.label})
                            })
                            .collect();
                        json!({"supported": true, "active": np.active, "level": np.level, "model": np.profile.model,
                               "led": p.hidraw.is_some(), "keys": keys})
                    }
                    None => json!({"supported": false, "active": false, "level": 0, "keys": []}),
                };
                (device, np, e.enabled, e.paused)
            }
            None => (
                serde_json::Value::Null,
                json!({"supported": false, "active": false, "level": 0, "keys": []}),
                self.cfg.general.enabled,
                false,
            ),
        };
        json!({
            "version": VERSION,
            "enabled": enabled,
            "paused": paused,
            "mode": mode,
            "mode_requested": self.cfg.general.mode,
            "mode_error": mode_error,
            "device": device,
            "numberpad": numberpad,
            "config_path": self.cfg_path.to_string_lossy(),
            "access": {"uinput": self.kb.is_some()},
        })
        .to_string()
    }

    fn publish(&mut self) {
        let state = self.state_json();
        {
            let mut s = self.shared.lock().unwrap();
            s.state_json = state;
            s.config = self.cfg.clone();
            s.max_fingers = self.pad.as_ref().map_or(0, |p| p.reader.info.max_fingers);
            s.semi_mt = self.pad.as_ref().is_some_and(|p| p.reader.info.semi_mt);
            s.numberpad = self.pad.as_ref().is_some_and(|p| p.engine.npad.is_some());
            s.passive = !self.pad.as_ref().is_some_and(|p| p.filter.is_some());
        }
        let _ = self.signals.send(SignalMsg::StateChanged);
        self.dirty = false;
    }

    fn command(&mut self, c: Command) {
        match c {
            Command::Reload => {
                let cfg = self.shared.lock().unwrap().config.clone();
                self.reload(cfg);
            }
            Command::SetLive(on) => {
                self.live = on;
                self.touches_empty_sent = false;
            }
            Command::SetEnabled(on) => {
                if let Some(pad) = self.pad.as_mut() {
                    let mut out = MainOut::new(self.kb.as_ref(), &self.exec, &self.signals, pad.hidraw.as_deref(), &pad.node, pad.filter.is_some());
                    pad.engine.set_enabled(on, &mut out);
                }
                self.dirty = true;
            }
            Command::RunLocal(action) => {
                if let Ok(Action::Keys(codes)) = Action::parse(&action) {
                    match &self.kb {
                        Some(kb) => {
                            let _ = kb.tap(&codes);
                        }
                        None => eprintln!("RunAction {action}: /dev/uinput is not writable"),
                    }
                    return;
                }
                if let Some(pad) = self.pad.as_mut() {
                    let mut out = MainOut::new(self.kb.as_ref(), &self.exec, &self.signals, pad.hidraw.as_deref(), &pad.node, pad.filter.is_some());
                    pad.engine.run_local(&action, &mut out);
                    self.dirty |= out.dirty;
                }
            }
        }
    }

    fn emit_touches(&mut self, now: f64) {
        if !self.live || now - self.last_touches < TOUCHES_INTERVAL_S {
            return;
        }
        let Some(pad) = &self.pad else { return };
        let contacts: Vec<_> = pad
            .tracker
            .contacts()
            .filter(|c| !c.lifted)
            .map(|c| {
                let (x, y) = pad.engine.geo.mm(c.x, c.y);
                json!({"slot": c.slot, "x_mm": (x * 10.0).round() / 10.0, "y_mm": (y * 10.0).round() / 10.0,
                       "role": pad.engine.role(c.slot)})
            })
            .collect();
        if contacts.is_empty() {
            if self.touches_empty_sent {
                return;
            }
            self.touches_empty_sent = true;
        } else {
            self.touches_empty_sent = false;
        }
        self.last_touches = now;
        let _ = self.signals.send(SignalMsg::Touches(json!({"t": now, "contacts": contacts}).to_string()));
    }

    /// One iteration with a connected touchpad. Returns false if the device is gone.
    fn pad_iteration(&mut self) -> bool {
        let Some(pad) = self.pad.as_mut() else { return false };
        let mut fds = vec![pad.reader.raw_fd()];
        if let Some(kb) = &self.kb {
            fds.push(kb.raw_fd());
        }
        let ready = match sys::poll(&fds, POLL_MS) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("poll: {e}");
                return true;
            }
        };
        let mut out = MainOut::new(self.kb.as_ref(), &self.exec, &self.signals, pad.hidraw.as_deref(), &pad.node, pad.filter.is_some());
        if ready.get(1).copied().unwrap_or(false)
            && let Some(on) = self.kb.as_ref().and_then(|k| k.read_numlock())
        {
            self.numlock = on;
            pad.engine.set_numlock(on, &mut out);
        }
        let mut alive = true;
        if ready[0] {
            let mut frames: Vec<Frame> = Vec::new();
            if let Err(e) = pad.reader.read_frames(&mut frames) {
                eprintln!("touchpad read: {e}");
                alive = false;
            }
            for f in &frames {
                if f.resync {
                    pad.tracker.reset();
                    pad.engine.reset(&mut out);
                    if let Some(fl) = pad.filter.as_mut() {
                        fl.reset();
                    }
                }
                pad.tracker.apply(&f.events, f.t);
                pad.engine.frame(&pad.tracker, f.t, &mut out);
                if let Some(fl) = pad.filter.as_mut() {
                    let engine = &pad.engine;
                    fl.process(&f.events, f.t, &|slot| engine.hidden(slot), &std::mem::take(&mut out.replays));
                }
                pad.tracker.remove_lifted();
            }
        }
        let now = sys::now();
        pad.engine.tick(&pad.tracker, now, &mut out);
        if let Some(fl) = pad.filter.as_mut() {
            let engine = &pad.engine;
            fl.process(&[], now, &|slot| engine.hidden(slot), &std::mem::take(&mut out.replays));
        }
        self.dirty |= out.dirty;
        alive
    }

    fn run(&mut self) {
        let mut next_probe = 0.0;
        let mut next_config = sys::now() + CONFIG_POLL_S;
        self.publish();
        while !STOP.load(Ordering::SeqCst) {
            let now = sys::now();
            if self.pad.is_none() {
                if now >= next_probe {
                    next_probe = now + RECONNECT_S;
                    if let Ok(info) = device::find_touchpad() {
                        self.connect(info);
                    }
                }
                if self.pad.is_none() {
                    // no touchpad: keep serving D-Bus and the NumLock LED
                    let fds: Vec<i32> = self.kb.iter().map(|k| k.raw_fd()).collect();
                    let _ = sys::poll(&fds, 100);
                    if let Some(on) = self.kb.as_ref().and_then(|k| k.read_numlock()) {
                        self.numlock = on;
                    }
                }
            } else if !self.pad_iteration() {
                self.disconnect("device lost");
                next_probe = sys::now() + RECONNECT_S;
            }
            while let Ok(c) = self.commands.try_recv() {
                self.command(c);
            }
            let now = sys::now();
            if now >= next_config {
                next_config = now + CONFIG_POLL_S;
                let m = mtime(&self.cfg_path);
                if m != self.cfg_mtime {
                    eprintln!("config: file changed, reloading");
                    let cfg = config::load(&self.cfg_path);
                    self.reload(cfg);
                }
            }
            self.emit_touches(now);
            if self.dirty {
                self.publish();
            }
        }
        if let Some(mut pad) = self.pad.take() {
            let mut out = MainOut::new(self.kb.as_ref(), &self.exec, &self.signals, pad.hidraw.as_deref(), &pad.node, pad.filter.is_some());
            pad.engine.shutdown(&mut out);
            if let Some(f) = pad.filter.as_mut() {
                f.reset();
            }
        }
    }
}

fn run() -> i32 {
    install_signal_handlers();
    let cfg_path = config::config_path();
    let cfg = config::load(&cfg_path);
    eprintln!("kagetd {VERSION} ({} mode), config {}", cfg.general.mode, cfg_path.display());

    let shared: Shared = Arc::new(Mutex::new(Snapshot { config: cfg.clone(), passive: true, ..Default::default() }));
    let (cmd_tx, cmd_rx) = mpsc::channel();

    // D-Bus first so a second instance fails here. The executor needs the connection, so D-Bus
    // requests reach it through a forwarding channel.
    let (proxy_tx, proxy_rx) = mpsc::channel::<Request>();
    let conn = match zbus::blocking::connection::Builder::session().and_then(|b| {
        let conn = b
            .serve_at(
                dbus::PATH,
                dbus::Daemon { shared: shared.clone(), commands: cmd_tx.clone(), executor: proxy_tx.clone(), config_path: cfg_path.clone() },
            )?
            .build()?;
        // the builder would queue for a taken name; fail instead
        conn.request_name_with_flags(dbus::NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())?;
        Ok(conn)
    }) {
        Ok(c) => Some(c),
        Err(zbus::Error::NameTaken) => {
            eprintln!("kagetd is already running ({} is taken)", dbus::NAME);
            return 1;
        }
        Err(e) => {
            eprintln!("D-Bus: {e}; running without the D-Bus API");
            None
        }
    };
    let signals = match &conn {
        Some(c) => dbus::spawn_signals(c.clone()),
        None => mpsc::channel().0,
    };
    let exec = executor::spawn(conn.clone(), cfg.steps.clone(), signals.clone());
    {
        let tx = exec.tx.clone();
        std::thread::spawn(move || {
            for r in proxy_rx {
                if tx.send(r).is_err() {
                    break;
                }
            }
        });
    }
    if let Some(c) = conn.clone() {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let kwin = dbus::shortcut_names(&c, "kwin");
            let plasma = dbus::shortcut_names(&c, "plasmashell");
            let mut s = shared.lock().unwrap();
            s.kwin_shortcuts = kwin;
            s.plasma_shortcuts = plasma;
        });
    }

    let kb = match VirtualKeyboard::new(&keyboard_codes()) {
        Ok(k) => Some(k),
        Err(e) => {
            eprintln!(
                "virtual keyboard: cannot create it ({e}); key actions and the NumberPad are unavailable \
                 (needs write access to /dev/uinput{})",
                if uinput_writable() { "" } else { ", which is not writable" }
            );
            None
        }
    };

    let mut d = Daemon {
        cfg_mtime: mtime(&cfg_path),
        cfg,
        cfg_path,
        kb,
        numlock: false,
        exec: exec.tx.clone(),
        signals,
        shared,
        commands: cmd_rx,
        pad: None,
        live: false,
        last_touches: 0.0,
        touches_empty_sent: true,
        dirty: false,
    };
    d.run();
    eprintln!("kagetd: stopping");
    drop(d);
    exec.join();
    drop(conn);
    0
}

// ---- tools --------------------------------------------------------------------------------------

fn open_touchpad() -> Option<(TouchpadInfo, Reader)> {
    let info = match device::find_touchpad() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{e}");
            return None;
        }
    };
    match Reader::open(info.clone()) {
        Ok(r) => Some((info, r)),
        Err(e) => {
            eprintln!("cannot open {}: {e}", info.path);
            None
        }
    }
}

fn zone(geo: &Geometry, np: Option<&numberpad::Profile>, x: i32, y: i32) -> String {
    let mut parts = Vec::new();
    if let Some(c) = geo.corner_at(x, y) {
        parts.push(format!("corner {}", c.name()));
    }
    for e in EDGES {
        if geo.in_side(e, x, y) {
            parts.push(format!("side-drag {}", e.name()));
        }
    }
    for e in EDGES {
        if geo.in_area(e, x, y) {
            parts.push(format!("area {}", e.name()));
        }
    }
    if let Some(k) = np.filter(|p| p.in_area(x, y)).and_then(|p| p.key_at(x, y)) {
        parts.push(format!("key {}", k.label));
    }
    if parts.is_empty() { "centre".into() } else { parts.join(", ") }
}

fn probe() -> i32 {
    install_signal_handlers();
    let cfg = config::load_readonly(&config::config_path());
    let Some((info, mut reader)) = open_touchpad() else { return 1 };
    let (size, geo) = Daemon::geometry(&cfg, &info);
    println!("touchpad: {}", describe(&info, size));
    println!("x: {}..{}  y: {}..{}", info.x.minimum, info.x.maximum, info.y.minimum, info.y.maximum);
    let profile = numberpad::profile_for(&info);
    println!(
        "NumberPad: {}",
        match &profile {
            Some(p) => format!("{} (hidraw {})", p.model, device::find_hidraw(&info).unwrap_or_else(|| "not found".into())),
            None => "none".into(),
        }
    );
    for c in CORNERS {
        let x = if c.is_right() { geo.width_mm - geo.corner_w } else { 0.0 };
        let y = if c.is_bottom() { geo.height_mm - geo.corner_h } else { 0.0 };
        println!("corner {:<12} x {:5.1}..{:5.1} mm  y {:5.1}..{:5.1} mm", c.name(), x, x + geo.corner_w, y, y + geo.corner_h);
    }
    if reader.grabbed_elsewhere() {
        println!("warning: another process holds an exclusive grab – no touches will arrive");
    }
    println!("Touch the pad; Ctrl+C to stop.");
    let mut tracker = Tracker::default();
    let mut zones: std::collections::HashMap<usize, String> = Default::default();
    while !STOP.load(Ordering::SeqCst) {
        let _ = sys::poll(&[reader.raw_fd()], 100);
        let mut frames = Vec::new();
        if let Err(e) = reader.read_frames(&mut frames) {
            eprintln!("read: {e}");
            return 1;
        }
        for f in frames {
            if f.resync {
                tracker.reset();
                zones.clear();
            }
            tracker.apply(&f.events, f.t);
            for c in tracker.contacts() {
                let (x, y) = geo.mm(c.x, c.y);
                if c.lifted {
                    zones.remove(&c.slot);
                    println!("slot {}: lifted after {:.0} ms", c.slot, (f.t - c.t0) * 1000.0);
                    continue;
                }
                let z = zone(&geo, profile.as_ref(), c.x, c.y);
                if zones.get(&c.slot) != Some(&z) {
                    println!("slot {} #{}: {:6.1} {:5.1} mm  ({:4} {:4})  {z}", c.slot, c.tid, x, y, c.x, c.y);
                    zones.insert(c.slot, z);
                }
            }
            tracker.remove_lifted();
        }
    }
    0
}

fn record(path: &str) -> i32 {
    install_signal_handlers();
    let Some((info, mut reader)) = open_touchpad() else { return 1 };
    let file = match std::fs::File::create(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{path}: {e}");
            return 1;
        }
    };
    let mut rec = match record::Recorder::new(std::io::BufWriter::new(file), &record::DeviceHeader::from_info(&info)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{path}: {e}");
            return 1;
        }
    };
    eprintln!("recording {} to {path}; Ctrl+C to stop", info.name);
    let mut n = 0usize;
    while !STOP.load(Ordering::SeqCst) {
        let _ = sys::poll(&[reader.raw_fd()], 100);
        let mut frames = Vec::new();
        if let Err(e) = reader.read_frames(&mut frames) {
            eprintln!("read: {e}");
            break;
        }
        for f in &frames {
            if let Err(e) = rec.frame(f) {
                eprintln!("{path}: {e}");
                return 1;
            }
            n += 1;
        }
    }
    eprintln!("{n} frames written");
    0
}

fn replay(path: &str) -> i32 {
    let cfg = config::load_readonly(&config::config_path());
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{path}: {e}");
            return 1;
        }
    };
    match record::replay(std::io::BufReader::new(file), &cfg) {
        Ok(out) => {
            let t0 = out.fired.first().map_or(0.0, |(t, _)| *t);
            for (t, f) in &out.fired {
                println!("{:8.3}s  {} -> {}", t - t0, f.trigger, f.action);
            }
            println!("{} gesture(s)", out.fired.len());
            0
        }
        Err(e) => {
            eprintln!("{path}: {e}");
            1
        }
    }
}

/// Fail-safe for ExecStopPost: pointer back on, NumberPad LED off.
fn restore() -> i32 {
    let info = match device::find_touchpad() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let mut code = 0;
    match zbus::blocking::Connection::session() {
        Ok(conn) => {
            if let Err(e) = executor::set_pointer(&conn, &node_name(&info), true) {
                eprintln!("KWin pointer switch failed: {e}");
                code = 1;
            }
        }
        Err(e) => {
            eprintln!("D-Bus: {e}");
            code = 1;
        }
    }
    if let (Some(p), Some(h)) = (numberpad::profile_for(&info), device::find_hidraw(&info)) {
        let _ = sys::hid_set_feature(&h, &numberpad::led_report(p.led_off));
    }
    code
}
