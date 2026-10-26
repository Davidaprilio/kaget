//! Gesture engine. Pure: no I/O, time is passed in, everything external goes through `Outputs`.
//!
//! Single-contact gestures (corners, sliders, NumberPad keys) live in `corners.rs`, sessions of three
//! or more fingers in `multi.rs`.

pub mod corners;
pub mod multi;
pub mod outputs;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::actions::Action;
use crate::config::Config;
use crate::geometry::Geometry;
use crate::npad::NumberPad;
use crate::tracker::Tracker;
use crate::triggers::Trigger;

use corners::Touch;
use outputs::{Fired, Outputs, PanelEvent};

/// Movement that cancels a hold or a key touch.
pub const MOVE_MM: f64 = 4.838;
/// Movement that turns a corner touch into a swipe.
pub const SWIPE_MM: f64 = 7.0;
/// Fraction of a slider step; keeps a resting finger from flickering.
pub const SLIDER_HYSTERESIS: f64 = 0.1;
/// A NumberPad key touch held this long presses the key until lift.
pub const KEY_DOWN_S: f64 = 0.12;
/// Panel mode: moving a finger 1/1.25 of the pad's height covers a slider's whole range.
pub const PANEL_GAIN: f64 = 1.25;
/// Panel mode: a finger has to move this far vertically before its slider follows, so a resting,
/// trembling finger changes nothing.
pub const PANEL_DEADZONE_MM: f64 = 2.0;
/// Panel mode: after that, movement is passed on in steps of this size (tremble below it is dropped).
pub const PANEL_STEP_MM: f64 = 0.5;

#[derive(Clone, Debug)]
struct Bound {
    action: String,
    when_running: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// Position-mapped slider: one step per grid line crossed, in either direction.
#[derive(Clone, Debug)]
pub struct Slider {
    pub trigger: String,
    pub action: String,
    pub inverse: String,
    pub when_running: Option<String>,
    pub axis: Axis,
    /// +1 when moving towards +x / +y runs `action`.
    pub sign: f64,
    /// Grid origin along the axis, in mm.
    pub origin: f64,
    pub level: i32,
    pub step: f64,
}

impl Slider {
    /// Fire the steps crossed since the last call. `pos` is the position along the axis in mm.
    fn slide(&mut self, pos: f64, out: &mut Vec<(String, String, Option<String>)>) {
        let h = SLIDER_HYSTERESIS;
        let raw = (pos - self.origin) * self.sign / self.step;
        while raw >= self.level as f64 + 1.0 + h {
            self.level += 1;
            out.push((self.trigger.clone(), self.action.clone(), self.when_running.clone()));
        }
        while raw < self.level as f64 - h {
            self.level -= 1;
            out.push((self.trigger.clone(), self.inverse.clone(), self.when_running.clone()));
        }
    }
}

pub struct Engine {
    pub cfg: Config,
    pub geo: Geometry,
    /// Passive mode: KDE sees every touch, so system-owned triggers stay with KDE. In filter mode
    /// the engine also decides which contacts are withheld from the desktop (`hidden`).
    pub passive: bool,
    pub max_fingers: u32,
    pub semi_mt: bool,
    pub enabled: bool,
    pub paused: bool,
    pub npad: Option<NumberPad>,
    numlock: bool,
    bindings: HashMap<String, Bound>,
    /// Filter mode: triggers bound to `none` (the gesture is swallowed, KDE does not get it).
    swallow: HashSet<String>,
    touches: BTreeMap<usize, Touch>,
    window: Option<corners::Window>,
    multi: multi::Multi,
    /// Steps produced while sliding, fired after the borrow of the slider ends.
    steps: Vec<(String, String, Option<String>)>,
    /// Time of the current frame / tick.
    now: f64,
    /// Passive mode: the pointer is switched off in KWin (NumberPad or panel mode).
    pointer_off: bool,
    /// Panel mode: the touchpad is divided into one slider column per panel control.
    panel_open: bool,
    panel_last: f64,
}

impl Engine {
    pub fn new(cfg: Config, geo: Geometry, max_fingers: u32, semi_mt: bool, npad: Option<NumberPad>) -> Self {
        let mut e = Self {
            enabled: cfg.general.enabled,
            cfg,
            geo,
            passive: true,
            max_fingers,
            semi_mt,
            paused: false,
            npad,
            numlock: false,
            bindings: HashMap::new(),
            swallow: HashSet::new(),
            touches: BTreeMap::new(),
            window: None,
            multi: multi::Multi::default(),
            steps: Vec::new(),
            now: 0.0,
            pointer_off: false,
            panel_open: false,
            panel_last: 0.0,
        };
        e.rebuild_bindings();
        e
    }

    /// Switch between passive and filter mode (system-owned triggers become bindable in filter
    /// mode when `general.override_system_gestures` is on).
    pub fn set_filter(&mut self, on: bool) {
        self.passive = !on;
        self.rebuild_bindings();
    }

    pub fn reconfigure(&mut self, cfg: Config, geo: Geometry, out: &mut dyn Outputs) {
        let enabled = cfg.general.enabled;
        self.cfg = cfg;
        self.geo = geo;
        self.window = None;
        self.rebuild_bindings();
        self.set_enabled(enabled, out);
    }

    fn rebuild_bindings(&mut self) {
        self.bindings.clear();
        self.swallow.clear();
        let system_bindable = !self.passive && self.cfg.general.override_system_gestures;
        for b in &self.cfg.bindings {
            let a = b.action.trim();
            let Ok(t) = Trigger::parse(&b.trigger) else { continue };
            let reachable = (system_bindable || !t.system_owned()) && t.available(self.max_fingers, self.semi_mt).is_ok();
            if a.is_empty() || a == "none" || a == "system" {
                self.bindings.remove(&b.trigger);
                if a == "none" && reachable && !self.passive {
                    self.swallow.insert(b.trigger.clone());
                } else {
                    self.swallow.remove(&b.trigger);
                }
                continue;
            }
            self.swallow.remove(&b.trigger);
            let Ok(action) = Action::parse(a) else { continue };
            // `system:<n>` needs the clone (filter mode) and a gesture the desktop knows
            let handover = matches!(t, Trigger::Swipe(..) | Trigger::PinchIn(_) | Trigger::PinchOut(_));
            let usable = reachable
                && (self.npad.is_some() || !matches!(action, Action::NumberPad(_)))
                && (!matches!(action, Action::SystemAs(_)) || (!self.passive && handover));
            if usable {
                self.bindings.insert(b.trigger.clone(), Bound { action: a.into(), when_running: b.when_running.clone() });
            } else {
                self.bindings.remove(&b.trigger);
            }
        }
    }

    /// The active binding for a trigger, honouring pause.
    fn bound(&self, trigger: &str) -> Option<&Bound> {
        let b = self.bindings.get(trigger)?;
        if self.paused && b.action != "kaget.pause" {
            return None;
        }
        Some(b)
    }

    fn is_bound(&self, trigger: &str) -> bool {
        self.bound(trigger).is_some()
    }

    /// Bound to `none` in filter mode: recognised and swallowed.
    fn is_swallowed(&self, trigger: &str) -> bool {
        !self.paused && self.swallow.contains(trigger)
    }

    /// Filter mode: is this contact withheld from the desktop?
    pub fn hidden(&self, slot: usize) -> bool {
        if self.passive || !self.enabled {
            return false;
        }
        if let Some(h) = self.multi_hidden(slot) {
            return h;
        }
        self.touches.get(&slot).is_some_and(|t| t.hidden())
    }

    /// Run the binding of `trigger` (if any).
    fn fire_trigger(&mut self, trigger: &str, out: &mut dyn Outputs) -> bool {
        let Some(b) = self.bound(trigger).cloned() else { return false };
        self.fire(trigger, &b.action, b.when_running, out)
    }

    /// Run an action. NumberPad and pause actions are handled here, the rest goes to `out`.
    fn fire(&mut self, trigger: &str, action: &str, when_running: Option<String>, out: &mut dyn Outputs) -> bool {
        let parsed = match Action::parse(action) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{trigger}: {e}");
                return false;
            }
        };
        let local = match parsed {
            Action::None | Action::System | Action::SystemAs(_) => return false,
            Action::Pause => {
                self.paused = !self.paused;
                eprintln!("gestures {}", if self.paused { "paused" } else { "resumed" });
                out.state_changed();
                true
            }
            Action::NumberPad(op) => {
                let Some(np) = self.npad.as_mut() else { return false };
                if !np.op(op, out) {
                    return false;
                }
                self.update_pointer(out);
                out.state_changed();
                true
            }
            Action::Panel => {
                let open = !self.panel_open;
                self.set_panel(open, self.now, out);
                true
            }
            _ => false,
        };
        out.fired(&Fired { trigger: trigger.into(), action: action.into(), when_running, local });
        true
    }

    fn flush_steps(&mut self, out: &mut dyn Outputs) {
        for (t, a, w) in std::mem::take(&mut self.steps) {
            self.fire(&t, &a, w, out);
        }
    }

    /// Process one frame. The tracker already holds this frame's events; lifted contacts are
    /// removed by the caller afterwards.
    pub fn frame(&mut self, tr: &Tracker, now: f64, out: &mut dyn Outputs) {
        self.now = now;
        if !self.enabled {
            self.touches.clear();
            return;
        }
        self.multi_frame(tr, now, out);
        self.single_frame(tr, now, out);
        self.flush_steps(out);
        self.touches.retain(|slot, _| tr.get(*slot).is_some_and(|c| !c.lifted));
    }

    /// Timers without a new frame.
    pub fn tick(&mut self, tr: &Tracker, now: f64, out: &mut dyn Outputs) {
        self.now = now;
        if !self.enabled {
            return;
        }
        self.single_timers(tr, now, out);
        self.multi_timers(tr, now, out);
        self.flush_steps(out);
        // panel mode ends after a while without slider use (never under a sliding finger)
        let hide_s = self.cfg.panel.auto_hide_s;
        if self.panel_open && hide_s > 0.0 && now - self.panel_last >= hide_s && !self.touches.values().any(|t| t.on_panel()) {
            self.set_panel(false, now, out);
        }
    }

    /// Open or close panel mode. In passive mode the pointer is switched off meanwhile.
    pub fn set_panel(&mut self, open: bool, now: f64, out: &mut dyn Outputs) {
        self.now = now;
        self.panel_last = now;
        if open == self.panel_open {
            return;
        }
        self.panel_open = open;
        eprintln!("panel mode {}", if open { "on" } else { "off" });
        out.panel(if open { PanelEvent::Open } else { PanelEvent::Close });
        if !open {
            for t in self.touches.values_mut() {
                t.leave_panel();
            }
        }
        self.update_pointer(out);
        out.state_changed();
    }

    /// The panel window was used (mouse): keep panel mode open.
    pub fn panel_activity(&mut self, now: f64) {
        self.panel_last = now;
    }

    pub fn panel_open(&self) -> bool {
        self.panel_open
    }

    /// Passive mode: the pointer is off while the NumberPad or panel mode is on.
    fn update_pointer(&mut self, out: &mut dyn Outputs) {
        let numberpad = self.npad.as_ref().is_some_and(|n| n.active);
        let off = self.passive && self.enabled && (numberpad || self.panel_open);
        if off != self.pointer_off {
            self.pointer_off = off;
            out.pointer(!off);
        }
    }

    /// Forget all touches (device lost or events dropped); releases held keys.
    pub fn reset(&mut self, out: &mut dyn Outputs) {
        for t in self.touches.values_mut() {
            t.release(out);
        }
        self.touches.clear();
        self.multi = multi::Multi::default();
    }

    pub fn set_numlock(&mut self, on: bool, out: &mut dyn Outputs) {
        self.numlock = on;
        if !self.enabled {
            return;
        }
        if let Some(np) = self.npad.as_mut()
            && np.set_active(on, out)
        {
            self.update_pointer(out);
            out.state_changed();
        }
    }

    pub fn set_enabled(&mut self, on: bool, out: &mut dyn Outputs) {
        if on == self.enabled {
            return;
        }
        self.enabled = on;
        if !on {
            self.reset(out);
            self.window = None;
        }
        let active = on && self.numlock;
        if let Some(np) = self.npad.as_mut() {
            np.set_active(active, out);
        }
        if !on {
            self.set_panel(false, self.now, out);
        }
        self.update_pointer(out);
        out.state_changed();
    }

    /// RunAction for actions the daemon handles itself (NumberPad, pause, panel). False if not local.
    pub fn run_local(&mut self, action: &str, out: &mut dyn Outputs) -> bool {
        match Action::parse(action) {
            Ok(Action::NumberPad(_)) | Ok(Action::Pause) | Ok(Action::Panel) => {
                self.fire("dbus", action, None, out);
                true
            }
            _ => false,
        }
    }

    pub fn shutdown(&mut self, out: &mut dyn Outputs) {
        self.reset(out);
        if self.panel_open {
            self.panel_open = false;
            out.panel(PanelEvent::Close);
        }
        match self.npad.as_mut() {
            Some(np) => np.shutdown(out), // switches the pointer back on
            None if self.pointer_off => out.pointer(true),
            None => {}
        }
        self.pointer_off = false;
    }

    /// Role of a contact for the live view (`Touches` signal).
    pub fn role(&self, slot: usize) -> &'static str {
        if let Some(r) = self.multi_role(slot) {
            return r;
        }
        self.touches.get(&slot).map(|t| t.role_name()).unwrap_or("pointer")
    }
}
