//! Single-contact gestures: corner hold and swipe, sliders with the continuation window, and
//! NumberPad key touches.

use super::outputs::{Diagnosis, Outputs, PanelEvent};
use super::{
    Axis, Engine, KEY_DOWN_S, MOVE_MM, PANEL_DEADZONE_MM, PANEL_GAIN, PANEL_STEP_MM, SLIDER_HYSTERESIS, SWIPE_MM, Slider,
};

/// Filter mode: a withheld corner touch shorter than this is replayed to the desktop as a tap.
pub const CORNER_TAP_S: f64 = 0.25;
/// Panel mode: a touch on a button column shorter than this (and without moving) presses it.
pub const PANEL_TAP_S: f64 = 0.6;
use crate::actions;
use crate::tracker::{Contact, Tracker};
use crate::triggers::{Corner, Dir, Edge, Trigger};

/// After a corner slider ends, a swipe along the same axis in the matching area band continues it.
#[derive(Clone, Debug)]
pub struct Window {
    pub trigger: String,
    pub action: String,
    pub when_running: Option<String>,
    pub axis: Axis,
    pub sign: f64,
    pub corner: Corner,
    pub until: f64,
}

/// A short tap on a corner with a tap-and-hold binding: it may be the first half of one.
#[derive(Clone, Debug)]
pub struct CornerTap {
    corner: Corner,
    tid: i32,
    /// Filter mode: withheld, replayed to the desktop as a click unless a tap-and-hold follows.
    withheld: bool,
    /// The next touch has to land on the same corner by then...
    until: f64,
    /// ...and this is it (slot).
    again: Option<usize>,
}

#[derive(Clone, Debug)]
pub enum Role {
    /// Not ours; KDE moves the pointer with it.
    Pointer,
    /// Landed in a corner with an active binding: hold or swipe pending.
    Corner(Corner),
    /// Landed in the continuation band while the window is open.
    Sticky { fallback: Box<Role>, win: Window },
    Slider { slider: Slider, corner: Corner },
    Key { codes: Vec<u16>, down: bool },
    /// Panel mode: a finger in the column of a panel slider. Until it moved past the dead zone
    /// (`engaged`), `x`/`y` is where it started; then `y` is where the last step was taken (mm).
    PanelSlider { kind: String, column: usize, x: f64, y: f64, engaged: bool },
    /// A gesture fired; the rest of this touch is ignored.
    Done,
    /// Rested next to a gesture the desktop got and was taken out of the clone meanwhile: hidden
    /// until it moves from here (device units), so lifting it is not a tap.
    Resting { x: i32, y: i32 },
}

impl Role {
    /// The corner of a corner touch (also of one that may continue a slider).
    fn corner(&self) -> Option<Corner> {
        match self {
            Role::Corner(c) => Some(*c),
            Role::Sticky { fallback, .. } => fallback.corner(),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Touch {
    pub role: Role,
    pub tid: i32,
    pub t0: f64,
    /// What it did, reported when it lifts (corner touches and band sliders).
    pub diag: Option<Diagnosis>,
}

impl Touch {
    pub fn release(&mut self, out: &mut dyn Outputs) {
        if let Role::Key { codes, down: true } = &self.role {
            release_key(codes, out);
        }
        self.role = Role::Pointer;
    }

    pub fn on_panel(&self) -> bool {
        matches!(self.role, Role::PanelSlider { .. })
    }

    /// Busy with a single-finger gesture of its own (it does not join a multi-finger gesture late).
    pub fn engaged(&self) -> bool {
        matches!(self.role, Role::Slider { .. } | Role::PanelSlider { .. } | Role::Done)
    }

    /// Panel mode ended under the finger: ignore the rest of the touch.
    pub fn leave_panel(&mut self) {
        if self.on_panel() {
            self.role = Role::Done;
        }
    }

    /// Filter mode: everything but plain pointer movement is withheld from the desktop.
    pub fn hidden(&self) -> bool {
        !matches!(self.role, Role::Pointer)
    }

    pub fn role_name(&self) -> &'static str {
        match self.role {
            Role::Pointer => "pointer",
            Role::Corner(_) => "corner",
            Role::Sticky { .. } | Role::Slider { .. } => "slider",
            Role::Key { .. } => "key",
            Role::PanelSlider { .. } => "panel",
            Role::Done => "gesture",
            Role::Resting { .. } => "resting",
        }
    }
}

fn corner_diag(corner: Corner, trigger: String, (result, action, reason): (&'static str, String, &'static str)) -> Diagnosis {
    Diagnosis { kind: "corner", fingers: 1, corner: corner.name(), trigger, result, action, reason, ..Default::default() }
}

fn release_key(codes: &[u16], out: &mut dyn Outputs) {
    let rev: Vec<u16> = codes.iter().rev().copied().collect();
    out.key(&rev, false);
}

/// Direction of a swipe that started in `corner`; `None` when it heads off the touchpad.
pub fn swipe_direction(dx: f64, dy: f64, corner: Corner) -> Option<Dir> {
    let dir = if dx.abs() >= 2.0 * dy.abs() {
        if dx > 0.0 { Dir::Right } else { Dir::Left }
    } else if dy.abs() >= 2.0 * dx.abs() {
        if dy > 0.0 { Dir::Down } else { Dir::Up }
    } else if (dx < 0.0) == corner.is_right() && (dy < 0.0) == corner.is_bottom() {
        Dir::Diagonal
    } else {
        return None;
    };
    corner.swipe_dirs().contains(&dir).then_some(dir)
}

impl Engine {
    fn hold_s(&self) -> f64 {
        self.cfg.corners.hold_ms as f64 / 1000.0
    }

    fn mm(&self, c: &Contact) -> (f64, f64) {
        self.geo.mm(c.x, c.y)
    }

    fn moved_mm(&self, c: &Contact) -> f64 {
        self.geo.dist_mm((c.x, c.y), (c.x0, c.y0))
    }

    fn delta_mm(&self, c: &Contact) -> (f64, f64) {
        ((c.x - c.x0) as f64 / self.geo.ux, (c.y - c.y0) as f64 / self.geo.uy)
    }

    fn corner_active(&self, corner: Corner) -> bool {
        self.is_bound(&Trigger::CornerHold(corner).to_string())
            || self.is_bound(&Trigger::CornerTapHold(corner).to_string())
            || corner.swipe_dirs().iter().any(|&d| self.is_bound(&Trigger::CornerSwipe(corner, d).to_string()))
    }

    /// Panel mode: the slider column under a point (the pad is split evenly, in panel order).
    fn panel_role(&self, x: i32, y: i32) -> Option<Role> {
        let kinds = &self.cfg.panel.controls;
        if !self.panel_open || kinds.is_empty() {
            return None;
        }
        let (mx, my) = self.geo.mm(x, y);
        let n = kinds.len();
        let column = ((mx / (self.geo.width_mm / n as f64)).floor().max(0.0) as usize).min(n - 1);
        Some(Role::PanelSlider { kind: kinds[column].clone(), column, x: mx, y: my, engaged: false })
    }

    fn classify_base(&self, c: &Contact) -> Role {
        if let Some(corner) = self.geo.corner_at(c.x, c.y)
            && self.corner_active(corner)
        {
            return Role::Corner(corner);
        }
        if let Some(r) = self.panel_role(c.x, c.y) {
            return r;
        }
        if let Some(np) = &self.npad
            && np.active
            && np.profile.in_area(c.x, c.y)
            && let Some(k) = np.profile.key_at(c.x, c.y)
        {
            return Role::Key { codes: k.codes.clone(), down: false };
        }
        Role::Pointer
    }

    fn in_window_band(&self, w: &Window, c: &Contact) -> bool {
        let edge = match w.axis {
            Axis::X => Edge::Top,
            Axis::Y if w.corner.is_right() => Edge::Right,
            Axis::Y => Edge::Left,
        };
        self.geo.in_area(edge, c.x, c.y)
    }

    fn classify(&mut self, c: &Contact, now: f64) -> Role {
        let base = self.classify_base(c);
        if self.panel_open {
            return base;
        }
        if let Some(w) = &self.window {
            if now < w.until && self.in_window_band(w, c) {
                return Role::Sticky { fallback: Box::new(base), win: w.clone() };
            }
            self.window = None; // window over, or the user touched elsewhere
        }
        base
    }

    fn along(&self, c: &Contact, axis: Axis) -> f64 {
        let (x, y) = self.mm(c);
        if axis == Axis::X { x } else { y }
    }

    /// The corner tap did not start a tap-and-hold: it was a plain tap (a click for the desktop).
    pub(super) fn flush_corner_tap(&mut self, out: &mut dyn Outputs) {
        if let Some(t) = self.corner_tap.take() {
            out.diagnosis(&corner_diag(t.corner, String::new(), ("kde", String::new(), "tap")));
            if t.withheld {
                out.replay(&[t.tid]);
            }
        }
    }

    fn is_tap_again(&self, slot: usize) -> bool {
        self.corner_tap.as_ref().is_some_and(|p| p.again == Some(slot))
    }

    /// A touch landed while a corner tap waits: the second half of a tap-and-hold if it is alone
    /// on the same corner in time, otherwise the tap was just a tap.
    fn corner_tap_landing(&mut self, c: &Contact, alone: bool, now: f64, out: &mut dyn Outputs) {
        let Some(p) = self.corner_tap.as_mut() else { return };
        if alone && p.again.is_none() && now <= p.until && self.geo.corner_at(c.x, c.y) == Some(p.corner) {
            p.again = Some(c.slot);
        } else {
            self.flush_corner_tap(out);
        }
    }

    /// A corner touch rested for the hold time: tap-and-hold right after a tap there, else hold;
    /// drifted without a swipe: leave it to KDE (or the panel).
    fn corner_held(&mut self, slot: usize, c: &Contact, corner: Corner, out: &mut dyn Outputs) -> (Role, Diagnosis) {
        let moved = self.moved_mm(c);
        let still = moved < MOVE_MM;
        if self.is_tap_again(slot) {
            let taphold = Trigger::CornerTapHold(corner).to_string();
            let outcome = self.outcome(&taphold);
            if still && self.fire_trigger(&taphold, out) {
                // the tap was its first half: no click
                self.corner_tap = None;
                return (Role::Done, corner_diag(corner, taphold, outcome));
            }
            self.flush_corner_tap(out);
        }
        let hold = Trigger::CornerHold(corner).to_string();
        let outcome = self.outcome(&hold);
        if still && self.fire_trigger(&hold, out) {
            return (Role::Done, corner_diag(corner, hold, outcome));
        }
        let diag = if still {
            corner_diag(corner, hold, ("kde", String::new(), "unbound"))
        } else {
            let d = corner_diag(corner, String::new(), ("kde", String::new(), "drifted"));
            Diagnosis { travel_mm: moved, need_mm: SWIPE_MM, ..d }
        };
        if let Some(r @ Role::PanelSlider { column, .. }) = self.panel_role(c.x, c.y) {
            out.panel(PanelEvent::Focus(Some(column)));
            (r, diag)
        } else {
            (Role::Pointer, diag)
        }
    }

    /// A corner touch moved far enough: start its swipe action, a slider, or leave it to KDE.
    fn corner_swipe(&mut self, c: &Contact, corner: Corner, out: &mut dyn Outputs) -> (Role, Diagnosis) {
        let (dx, dy) = self.delta_mm(c);
        let Some(dir) = swipe_direction(dx, dy, corner) else {
            return (Role::Pointer, corner_diag(corner, String::new(), ("kde", String::new(), "off_pad")));
        };
        let trigger = Trigger::CornerSwipe(corner, dir).to_string();
        let Some(b) = self.bound(&trigger).cloned() else {
            return (Role::Pointer, corner_diag(corner, trigger, ("kde", String::new(), "unbound")));
        };
        if let Some(inverse) = actions::inverse(&b.action)
            && dir != Dir::Diagonal
        {
            let diag = corner_diag(corner, trigger.clone(), ("slider", b.action.clone(), ""));
            let axis = if matches!(dir, Dir::Left | Dir::Right) { Axis::X } else { Axis::Y };
            let sign = if matches!(dir, Dir::Left | Dir::Up) { -1.0 } else { 1.0 };
            let step = self.cfg.slider.step_mm;
            let (x0, y0) = self.geo.mm(c.x0, c.y0);
            let start = if axis == Axis::X { x0 } else { y0 };
            let mut slider = Slider {
                trigger,
                action: b.action,
                inverse: inverse.into(),
                when_running: b.when_running,
                axis,
                sign,
                // grid origin chosen so the first step fires at SWIPE_MM, then one per step
                origin: start + sign * (SWIPE_MM - step),
                level: 0,
                step,
            };
            slider.slide(self.along(c, axis), &mut self.steps);
            self.flush_steps(out);
            return (Role::Slider { slider, corner }, diag);
        }
        let diag = corner_diag(corner, trigger.clone(), ("fired", b.action.clone(), ""));
        self.fire(&trigger, &b.action, b.when_running, out);
        (Role::Done, diag)
    }

    /// Panel mode: vertical finger movement adjusts the column's control (up = more).
    #[allow(clippy::too_many_arguments)]
    fn panel_slide(
        &mut self,
        c: &Contact,
        kind: String,
        column: usize,
        (x, y): (f64, f64),
        engaged: bool,
        now: f64,
        out: &mut dyn Outputs,
    ) -> Role {
        if !self.panel_open {
            return Role::Done;
        }
        if crate::config::panel_slot_action(&kind).is_some() {
            // a button column: only a tap counts (on lift)
            return Role::PanelSlider { kind, column, x, y, engaged };
        }
        let (cx, cy) = self.mm(c);
        if !engaged {
            let (dx, dy) = (cx - x, cy - y);
            if dy.abs() < PANEL_DEADZONE_MM || dy.abs() < dx.abs() {
                return Role::PanelSlider { kind, column, x, y, engaged };
            }
            // real vertical movement: the slider follows from here on, without a jump
            self.panel_last = now;
            return Role::PanelSlider { kind, column, x: cx, y: cy, engaged: true };
        }
        if (y - cy).abs() < PANEL_STEP_MM {
            return Role::PanelSlider { kind, column, x, y, engaged };
        }
        self.panel_last = now;
        let delta = (y - cy) / self.geo.height_mm * 100.0 * PANEL_GAIN;
        out.panel(PanelEvent::Adjust { kind: kind.clone(), delta });
        Role::PanelSlider { kind, column, x, y: cy, engaged }
    }

    /// A continuation candidate moved: a swipe along the slider axis resumes the slider,
    /// anything else gives the touch its normal role.
    fn continue_or_fallback(&mut self, c: &Contact, fallback: Role, win: Window, out: &mut dyn Outputs) -> Role {
        let (dx, dy) = self.delta_mm(c);
        let (along, across) = if win.axis == Axis::Y { (dy, dx) } else { (dx, dy) };
        if along.abs() >= 2.0 * across.abs() {
            let Some(inverse) = actions::inverse(&win.action) else { return Role::Pointer };
            let step = self.cfg.slider.step_mm;
            let pos = self.along(c, win.axis);
            // origin placed so exactly one step fires right away in the direction of movement
            let h = SLIDER_HYSTERESIS;
            let raw = if along * win.sign > 0.0 { 1.0 + 2.0 * h } else { -2.0 * h };
            let mut slider = Slider {
                trigger: win.trigger,
                action: win.action,
                inverse: inverse.into(),
                when_running: win.when_running,
                axis: win.axis,
                sign: win.sign,
                origin: pos - raw * step * win.sign,
                level: 0,
                step,
            };
            slider.slide(pos, &mut self.steps);
            self.flush_steps(out);
            return Role::Slider { slider, corner: win.corner };
        }
        self.window = None;
        match fallback {
            // a key touch that moved is pointer movement
            Role::Key { .. } | Role::Pointer => Role::Pointer,
            // corner roles keep being evaluated as corner gestures
            other => other,
        }
    }

    pub(super) fn single_frame(&mut self, tr: &Tracker, now: f64, out: &mut dyn Outputs) {
        let live = tr.live();
        // 1. new contacts
        for c in tr.contacts().filter(|c| c.fresh && !c.lifted) {
            let alone = live == 1;
            self.corner_tap_landing(c, alone, now, out);
            if self.multi_owns(c.slot) {
                continue;
            }
            let role = if alone { self.classify(c, now) } else { Role::Pointer };
            if !alone {
                // a second finger turns pending touches into a 2-finger gesture (owned by the system)
                for t in self.touches.values_mut() {
                    if matches!(t.role, Role::Corner(_) | Role::Sticky { .. } | Role::Key { .. }) {
                        t.release(out);
                    }
                }
            }
            if let Role::PanelSlider { column, .. } = role {
                self.panel_last = now;
                out.panel(PanelEvent::Focus(Some(column)));
            }
            self.touches.insert(c.slot, Touch { role, tid: c.tid, t0: c.t0, diag: None });
        }
        // 2. movement
        let slots: Vec<usize> = self.touches.keys().copied().collect();
        for &slot in &slots {
            let Some(c) = tr.get(slot).filter(|c| !c.lifted) else { continue };
            let Some(mut t) = self.touches.remove(&slot) else { continue };
            let moved = self.moved_mm(c);
            let steps = self.fired_steps;
            t.role = match t.role {
                Role::Key { down: false, .. } if moved > MOVE_MM => Role::Pointer,
                Role::Resting { x, y } if self.geo.dist_mm((c.x, c.y), (x, y)) > MOVE_MM => Role::Pointer,
                Role::Sticky { fallback, win } if moved > MOVE_MM => {
                    self.continue_or_fallback(c, *fallback, win, out)
                }
                // panel mode: a drag that starts in a corner still moves that column's slider
                Role::Corner(corner) if self.panel_open && moved > MOVE_MM => {
                    match self.panel_role(c.x0, c.y0) {
                        Some(Role::PanelSlider { kind, column, x, y, engaged }) => {
                            out.panel(PanelEvent::Focus(Some(column)));
                            self.panel_slide(c, kind, column, (x, y), engaged, now, out)
                        }
                        _ => Role::Corner(corner),
                    }
                }
                Role::Corner(corner) if moved > SWIPE_MM => {
                    let (role, diag) = self.corner_swipe(c, corner, out);
                    t.diag = Some(Diagnosis { travel_mm: moved, ..diag });
                    role
                }
                Role::PanelSlider { kind, column, x, y, engaged } => {
                    self.panel_slide(c, kind, column, (x, y), engaged, now, out)
                }
                Role::Slider { mut slider, corner } => {
                    slider.slide(self.along(c, slider.axis), &mut self.steps);
                    self.flush_steps(out);
                    Role::Slider { slider, corner }
                }
                r => r,
            };
            if t.diag.is_none()
                && let Role::Slider { slider, corner } = &t.role
            {
                // a slider continued in its band
                t.diag = Some(corner_diag(*corner, slider.trigger.clone(), ("slider", slider.action.clone(), "continued")));
            }
            if let Some(d) = t.diag.as_mut() {
                d.steps += self.fired_steps.wrapping_sub(steps);
            }
            if self.is_tap_again(slot) && t.role.corner().is_none() {
                // the second touch became a swipe or pointer movement
                self.flush_corner_tap(out);
            }
            self.touches.insert(slot, t);
        }
        // 3. timers
        self.single_timers(tr, now, out);
        // 4. lifts
        for &slot in &slots {
            if !tr.get(slot).is_some_and(|c| c.lifted) {
                continue;
            }
            let Some(mut t) = self.touches.remove(&slot) else { continue };
            let tap = now - t.t0 < CORNER_TAP_S;
            // panel mode: a tap on a button column runs its action
            if let Role::PanelSlider { kind, .. } = &t.role
                && let Some(action) = crate::config::panel_slot_action(kind)
                && now - t.t0 < PANEL_TAP_S
                && tr.get(slot).is_some_and(|c| self.moved_mm(c) < MOVE_MM)
            {
                let action = action.to_string();
                self.panel_last = now;
                self.fire("panel", &action, None, out);
            }
            let still = tr.get(slot).is_some_and(|c| self.moved_mm(c) < MOVE_MM);
            let again = self.is_tap_again(slot);
            if again {
                // tapped twice
                self.flush_corner_tap(out);
            }
            let tap_corner = t
                .role
                .corner()
                .filter(|&c| tap && still && !again && self.is_bound(&Trigger::CornerTapHold(c).to_string()));
            // a tap held back for a tap-and-hold reports itself once that is clear
            let diag = t.diag.take().or_else(|| match t.role.corner() {
                Some(corner) if tap_corner.is_none() => Some(if tap && still {
                    corner_diag(corner, String::new(), ("kde", String::new(), "tap"))
                } else if !still {
                    Diagnosis { need_mm: SWIPE_MM, ..corner_diag(corner, String::new(), ("none", String::new(), "short")) }
                } else {
                    let d = corner_diag(corner, String::new(), ("none", String::new(), "early"));
                    Diagnosis { need_ms: self.cfg.corners.hold_ms as f64, ..d }
                }),
                _ => None,
            });
            if let Some(mut d) = diag {
                d.duration_ms = (now - t.t0) * 1000.0;
                if d.travel_mm == 0.0 {
                    d.travel_mm = tr.get(slot).map_or(0.0, |c| self.moved_mm(c));
                }
                out.diagnosis(&d);
            }
            let idle = self.on_lift(t.role, now, out);
            if let Some(corner) = tap_corner {
                // may be the first half of a tap-and-hold: hold the click back until that is clear
                let until = now + self.cfg.taps.double_tap_ms as f64 / 1000.0;
                self.corner_tap = Some(CornerTap { corner, tid: t.tid, withheld: !self.passive, until, again: None });
            } else if idle && tap && !self.passive {
                // a short withheld touch was a tap after all: let the desktop click
                out.replay(&[t.tid]);
            }
        }
    }

    /// Returns true if the touch did nothing (a corner or band tap the desktop may want).
    fn on_lift(&mut self, role: Role, now: f64, out: &mut dyn Outputs) -> bool {
        match role {
            Role::Sticky { fallback, .. } => {
                // lifted without a swipe: a tap in its normal role
                self.window = None;
                return self.on_lift(*fallback, now, out);
            }
            Role::Corner(_) | Role::Pointer => return true,
            Role::PanelSlider { kind, .. } => {
                // the idle time before panel mode ends counts from the lift
                self.panel_last = now;
                out.panel(PanelEvent::Focus(None));
                if kind == "scale" {
                    out.panel(PanelEvent::Commit);
                }
            }
            Role::Key { codes, down } => {
                if !down {
                    out.key(&codes, true);
                }
                release_key(&codes, out);
            }
            Role::Slider { slider, corner } if self.cfg.slider.continue_s > 0.0 => {
                self.window = Some(Window {
                    trigger: slider.trigger,
                    action: slider.action,
                    when_running: slider.when_running,
                    axis: slider.axis,
                    sign: slider.sign,
                    corner,
                    until: now + self.cfg.slider.continue_s,
                });
            }
            _ => {}
        }
        false
    }

    pub(super) fn single_timers(&mut self, tr: &Tracker, now: f64, out: &mut dyn Outputs) {
        if self.corner_tap.as_ref().is_some_and(|p| p.again.is_none() && now > p.until) {
            self.flush_corner_tap(out);
        }
        let hold_s = self.hold_s();
        let slots: Vec<usize> = self.touches.keys().copied().collect();
        for slot in slots {
            let Some(c) = tr.get(slot).filter(|c| !c.lifted) else { continue };
            let Some(mut t) = self.touches.remove(&slot) else { continue };
            let held = now - c.t0;
            t.role = match t.role {
                Role::Key { codes, down: false } if held >= KEY_DOWN_S => {
                    out.key(&codes, true);
                    Role::Key { codes, down: true }
                }
                Role::Sticky { fallback, .. } if held >= hold_s => {
                    // resting finger in the band: behave as without the window
                    self.window = None;
                    match *fallback {
                        Role::Key { codes, .. } => {
                            out.key(&codes, true);
                            Role::Key { codes, down: true }
                        }
                        other => other,
                    }
                }
                Role::Corner(corner) if held >= hold_s => {
                    let (role, diag) = self.corner_held(slot, c, corner, out);
                    t.diag = Some(diag);
                    role
                }
                r => r,
            };
            self.touches.insert(slot, t);
        }
    }
}
