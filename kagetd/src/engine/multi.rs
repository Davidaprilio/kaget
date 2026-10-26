//! Sessions of three or more fingers: side drag, swipe (and swipe sliders), pinch, hold,
//! tap, double tap and tap-hold.

use std::collections::BTreeMap;

use super::outputs::Outputs;
use super::{Axis, Engine, SLIDER_HYSTERESIS, Slider};
use crate::actions;
use crate::tracker::Tracker;
use crate::geometry::Geometry;
use crate::triggers::{Dir, EDGES, Edge, Trigger};

/// All fingers of a session land within this time of the first one.
pub const LANDING_S: f64 = 0.08;
/// A tap or hold may not move more than this (per finger).
pub const STILL_MM: f64 = 3.0;

#[derive(Debug)]
enum State {
    Pending,
    Slider(Slider),
    /// Recognised (or not ours): ignore until all fingers are lifted.
    Done,
}

#[derive(Debug)]
struct Session {
    n: u32,
    t_first: f64,
    /// slot -> start position in mm
    start: BTreeMap<usize, (f64, f64)>,
    /// All fingers started inside this side-drag band.
    edge: Option<Edge>,
    state: State,
    max_move: f64,
    /// A tap that ended just before this session (double tap / tap-hold).
    after_tap: Option<Tap>,
    /// slot -> tracking id
    tids: BTreeMap<usize, i32>,
    /// Filter mode: the fingers are withheld from the desktop...
    withhold: bool,
    /// ...until it turns out to be a desktop gesture after all.
    revealed: bool,
    /// The finger count was checked against the bindings once the landing window closed.
    checked: bool,
}

/// A finished tap, kept so it can still be handed to the desktop.
#[derive(Clone, Debug)]
struct Tap {
    n: u32,
    tids: Vec<i32>,
    withheld: bool,
}

#[derive(Debug)]
struct PendingTap {
    tap: Tap,
    until: f64,
}

#[derive(Debug, Default)]
pub struct Multi {
    session: Option<Session>,
    pending_tap: Option<PendingTap>,
}

/// Movement of a session's fingers that are still down, relative to their start positions.
struct Motion {
    /// Centroid displacement (mm).
    d: (f64, f64),
    /// Mean distance to the centroid, start and now.
    spread0: f64,
    spread1: f64,
    max_move: f64,
    k: usize,
}

fn side_edge(geo: &Geometry, pts: &[(i32, i32)]) -> Option<Edge> {
    EDGES.into_iter().find(|&e| pts.iter().all(|&(x, y)| geo.in_side(e, x, y)))
}

fn mean(pts: &[(f64, f64)]) -> (f64, f64) {
    let n = pts.len().max(1) as f64;
    (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n)
}

type Point = (f64, f64);

/// `pairs`: (start, now) per finger.
fn motion(pairs: &[(Point, Point)]) -> Motion {
    let s: Vec<(f64, f64)> = pairs.iter().map(|p| p.0).collect();
    let c: Vec<(f64, f64)> = pairs.iter().map(|p| p.1).collect();
    let (c0, c1) = (mean(&s), mean(&c));
    let k = pairs.len();
    let mut spread0 = 0.0;
    let mut spread1 = 0.0;
    let mut max_move: f64 = 0.0;
    for (a, b) in pairs {
        let (ax, ay) = (a.0 - c0.0, a.1 - c0.1);
        let (bx, by) = (b.0 - c1.0, b.1 - c1.1);
        spread0 += ax.hypot(ay);
        spread1 += bx.hypot(by);
        max_move = max_move.max((b.0 - a.0).hypot(b.1 - a.1));
    }
    let kf = k.max(1) as f64;
    Motion {
        d: (c1.0 - c0.0, c1.1 - c0.1),
        spread0: spread0 / kf,
        spread1: spread1 / kf,
        max_move,
        k,
    }
}

impl Engine {
    pub(super) fn multi_owns(&self, slot: usize) -> bool {
        self.multi.session.as_ref().is_some_and(|s| s.start.contains_key(&slot))
    }

    pub(super) fn multi_hidden(&self, slot: usize) -> Option<bool> {
        let s = self.multi.session.as_ref().filter(|s| s.start.contains_key(&slot))?;
        Some(s.withhold && !s.revealed)
    }

    /// Filter mode: could a binding match a session of `n` fingers (`exact`: exactly n, else n or
    /// more because fingers may still land) starting in side-drag band `edge`?
    fn multi_could_match(&self, n: u32, edge: Option<Edge>, exact: bool) -> bool {
        let bound = self.bindings.keys().filter(|k| self.is_bound(k));
        let swallowed = self.swallow.iter().filter(|k| self.is_swallowed(k));
        bound.chain(swallowed).filter_map(|k| Trigger::parse(k).ok()).any(|t| {
            let f = t.fingers();
            let count_ok = if exact { f == n } else { f >= n };
            f >= 3
                && count_ok
                && match t {
                    Trigger::SideDrag(e, _) => Some(e) == edge,
                    _ => true,
                }
        })
    }

    /// The withheld fingers belong to the desktop after all: replay them.
    fn reveal(s: &mut Session, out: &mut dyn Outputs) {
        if s.withhold && !s.revealed {
            s.revealed = true;
            let tids: Vec<i32> = s.tids.values().copied().collect();
            out.replay(&tids);
        }
    }

    fn tap_of(s: &Session) -> Tap {
        Tap { n: s.n, tids: s.tids.values().copied().collect(), withheld: s.withhold && !s.revealed }
    }

    pub(super) fn multi_role(&self, slot: usize) -> Option<&'static str> {
        let s = self.multi.session.as_ref().filter(|s| s.start.contains_key(&slot))?;
        Some(match s.state {
            State::Pending if s.edge.is_some() => "sidedrag",
            State::Slider(_) => "slider",
            _ => "gesture",
        })
    }

    fn multi_enabled(&self) -> bool {
        !self.semi_mt && self.max_fingers >= 3
    }

    /// The first fingers of a pending tap belong to an earlier gesture after all: run that tap.
    fn recognised(&mut self, s: &mut Session, out: &mut dyn Outputs) {
        if let Some(first) = s.after_tap.take() {
            self.flush_tap(&first, out);
        }
    }

    /// A tap that turned out not to start a double tap / tap-hold runs as a plain tap; unbound
    /// (or `system`) withheld taps go to the desktop (e.g. libinput's 3-finger middle click).
    fn flush_tap(&mut self, tap: &Tap, out: &mut dyn Outputs) {
        let t = Trigger::Tap(tap.n).to_string();
        if self.fire_trigger(&t, out) || self.is_swallowed(&t) {
            return;
        }
        if tap.withheld {
            out.replay(&tap.tids);
        }
    }

    /// A recognised trigger: fire it, swallow it, or hand the fingers to the desktop.
    fn take_or_reveal(&mut self, s: &mut Session, trigger: &str, out: &mut dyn Outputs) -> State {
        if !self.fire_trigger(trigger, out) && !self.is_swallowed(trigger) {
            Self::reveal(s, out);
        }
        State::Done
    }

    pub(super) fn multi_frame(&mut self, tr: &Tracker, now: f64, out: &mut dyn Outputs) {
        if !self.multi_enabled() {
            return;
        }
        let live: Vec<_> = tr.contacts().filter(|c| !c.lifted).collect();
        let fresh = live.iter().any(|c| c.fresh);
        if self.multi.session.is_none() {
            if live.len() < 3 || !fresh {
                return;
            }
            let t_first = live.iter().map(|c| c.t0).fold(f64::INFINITY, f64::min);
            let landed_together = live.iter().all(|c| c.t0 - t_first <= LANDING_S);
            let pts: Vec<(i32, i32)> = live.iter().map(|c| (c.x0, c.y0)).collect();
            let edge = side_edge(&self.geo, &pts);
            let after_tap = match self.multi.pending_tap.take() {
                Some(p) if now <= p.until => Some(p.tap),
                Some(p) => {
                    self.flush_tap(&p.tap, out);
                    None
                }
                None => None,
            };
            let start = live.iter().map(|c| (c.slot, self.geo.mm(c.x0, c.y0))).collect();
            if landed_together {
                // the fingers now belong to the session
                for c in &live {
                    if let Some(mut t) = self.touches.remove(&c.slot) {
                        t.release(out);
                    }
                }
            }
            let n = live.len() as u32;
            let withhold = !self.passive && landed_together && self.multi_could_match(n, edge, false);
            self.multi.session = Some(Session {
                n,
                t_first,
                start,
                edge,
                // fingers that did not land together (e.g. a slider plus two more): not a session
                state: if landed_together { State::Pending } else { State::Done },
                max_move: 0.0,
                after_tap,
                tids: live.iter().map(|c| (c.slot, c.tid)).collect(),
                withhold,
                revealed: false,
                checked: false,
            });
        } else if let Some(s) = self.multi.session.as_mut() {
            for c in live.iter().filter(|c| c.fresh) {
                s.start.insert(c.slot, self.geo.mm(c.x0, c.y0));
                s.tids.insert(c.slot, c.tid);
                if c.t0 - s.t_first <= LANDING_S && matches!(s.state, State::Pending) {
                    s.n += 1;
                } else {
                    // finger count changed mid-gesture: not ours
                    s.state = State::Done;
                    Self::reveal(s, out);
                }
                self.touches.remove(&c.slot);
            }
            if s.edge.is_some() {
                let pts: Vec<(i32, i32)> =
                    live.iter().filter(|c| s.start.contains_key(&c.slot)).map(|c| (c.x0, c.y0)).collect();
                s.edge = side_edge(&self.geo, &pts);
            }
        }
        self.multi_classify(tr, now, out);
        if tr.contacts().all(|c| c.lifted) {
            self.multi_end(now, out);
        }
    }

    fn session_motion(&self, tr: &Tracker) -> Option<Motion> {
        let s = self.multi.session.as_ref()?;
        let pairs: Vec<_> = s
            .start
            .iter()
            .filter_map(|(&slot, &p)| {
                let c = tr.get(slot)?;
                Some((p, self.geo.mm(c.x, c.y)))
            })
            .collect();
        (!pairs.is_empty()).then(|| motion(&pairs))
    }

    fn multi_classify(&mut self, tr: &Tracker, now: f64, out: &mut dyn Outputs) {
        let Some(m) = self.session_motion(tr) else { return };
        let Some(mut s) = self.multi.session.take() else { return };
        s.max_move = s.max_move.max(m.max_move);
        s.state = match s.state {
            State::Slider(mut sl) => {
                let pos = if sl.axis == Axis::X { m.d.0 } else { m.d.1 };
                sl.slide(pos, &mut self.steps);
                self.flush_steps(out);
                State::Slider(sl)
            }
            State::Pending if now - s.t_first >= LANDING_S => {
                if !s.checked {
                    s.checked = true;
                    if s.withhold && !self.multi_could_match(s.n, s.edge, true) {
                        Self::reveal(&mut s, out);
                    }
                }
                self.recognise(&mut s, &m, now, out)
            }
            st => st,
        };
        self.multi.session = Some(s);
    }

    fn recognise(&mut self, s: &mut Session, m: &Motion, now: f64, out: &mut dyn Outputs) -> State {
        let n = s.n;
        let g = self.cfg.gestures.clone();
        // side drag: all fingers started in the same band and moved inward
        if let Some(edge) = s.edge
            && n <= 4
        {
            let inward = match edge {
                Edge::Top => m.d.1,
                Edge::Bottom => -m.d.1,
                Edge::Left => m.d.0,
                Edge::Right => -m.d.0,
            };
            let trigger = Trigger::SideDrag(edge, n).to_string();
            if inward >= self.cfg.side_drag.min_travel_mm && (self.is_bound(&trigger) || self.is_swallowed(&trigger)) {
                self.recognised(s, out);
                return self.take_or_reveal(s, &trigger, out);
            }
        }
        let dist = m.d.0.hypot(m.d.1);
        if dist >= g.swipe_mm {
            self.recognised(s, out);
            let dir = if m.d.0.abs() >= m.d.1.abs() {
                if m.d.0 > 0.0 { Dir::Right } else { Dir::Left }
            } else if m.d.1 > 0.0 {
                Dir::Down
            } else {
                Dir::Up
            };
            let trigger = Trigger::Swipe(n, dir).to_string();
            let Some(b) = self.bound(&trigger).cloned() else { return self.take_or_reveal(s, &trigger, out) };
            if let Some(inverse) = actions::inverse(&b.action) {
                let axis = if matches!(dir, Dir::Left | Dir::Right) { Axis::X } else { Axis::Y };
                let sign = if matches!(dir, Dir::Left | Dir::Up) { -1.0 } else { 1.0 };
                let step = self.cfg.slider.step_mm;
                let pos = if axis == Axis::X { m.d.0 } else { m.d.1 };
                let mut sl = Slider {
                    trigger,
                    action: b.action,
                    inverse: inverse.into(),
                    when_running: b.when_running,
                    axis,
                    sign,
                    // first step right away, then one per slider step
                    origin: pos - sign * (1.0 + 2.0 * SLIDER_HYSTERESIS) * step,
                    level: 0,
                    step,
                };
                sl.slide(pos, &mut self.steps);
                self.flush_steps(out);
                return State::Slider(sl);
            }
            self.fire(&trigger, &b.action, b.when_running, out);
            return State::Done;
        }
        if m.k >= 2 && m.spread0 > 1.0 {
            let ratio = m.spread1 / m.spread0;
            let pinch = if ratio >= g.pinch_ratio {
                Some(Trigger::PinchOut(n))
            } else if ratio <= 1.0 / g.pinch_ratio {
                Some(Trigger::PinchIn(n))
            } else {
                None
            };
            if let Some(t) = pinch {
                self.recognised(s, out);
                return self.take_or_reveal(s, &t.to_string(), out);
            }
        }
        self.multi_hold(s, now, out)
    }

    /// Fingers still for `taps.hold_ms`: hold, or tap-hold right after a tap.
    fn multi_hold(&mut self, s: &mut Session, now: f64, out: &mut dyn Outputs) -> State {
        if s.max_move >= STILL_MM || now - s.t_first < self.cfg.taps.hold_ms as f64 / 1000.0 {
            return State::Pending;
        }
        let n = s.n;
        let taphold = Trigger::TapHold(n).to_string();
        if s.after_tap.as_ref().is_some_and(|t| t.n == n)
            && (self.fire_trigger(&taphold, out) || self.is_swallowed(&taphold))
        {
            s.after_tap = None;
            return State::Done;
        }
        let hold = Trigger::Hold(n).to_string();
        if self.is_bound(&hold) || self.is_swallowed(&hold) {
            self.recognised(s, out);
            self.fire_trigger(&hold, out);
            return State::Done;
        }
        State::Pending
    }

    /// All fingers lifted.
    fn multi_end(&mut self, now: f64, out: &mut dyn Outputs) {
        let Some(mut s) = self.multi.session.take() else { return };
        let n = s.n;
        let is_tap = matches!(s.state, State::Pending)
            && now - s.t_first <= self.cfg.taps.max_tap_ms as f64 / 1000.0
            && s.max_move < STILL_MM;
        let first = s.after_tap.take();
        if !is_tap {
            if let Some(first) = first
                && matches!(s.state, State::Pending)
            {
                self.flush_tap(&first, out);
            }
            return;
        }
        let tap = Self::tap_of(&s);
        match first {
            Some(first) if first.n == n => {
                let dt = Trigger::DoubleTap(n).to_string();
                if !self.fire_trigger(&dt, out) && !self.is_swallowed(&dt) {
                    self.flush_tap(&first, out);
                    self.flush_tap(&tap, out);
                }
                return;
            }
            Some(first) => self.flush_tap(&first, out),
            None => {}
        }
        let waits = [Trigger::DoubleTap(n), Trigger::TapHold(n)]
            .iter()
            .any(|t| self.is_bound(&t.to_string()) || self.is_swallowed(&t.to_string()));
        if waits {
            let until = now + self.cfg.taps.double_tap_ms as f64 / 1000.0;
            self.multi.pending_tap = Some(PendingTap { tap, until });
        } else {
            self.flush_tap(&tap, out);
        }
    }

    pub(super) fn multi_timers(&mut self, tr: &Tracker, now: f64, out: &mut dyn Outputs) {
        if let Some(p) = &self.multi.pending_tap
            && self.multi.session.is_none()
            && now > p.until
        {
            let tap = p.tap.clone();
            self.multi.pending_tap = None;
            self.flush_tap(&tap, out);
        }
        if self.multi.session.as_ref().is_some_and(|s| matches!(s.state, State::Pending)) {
            let motion = self.session_motion(tr);
            let Some(mut s) = self.multi.session.take() else { return };
            if now - s.t_first >= LANDING_S {
                if let Some(m) = motion {
                    s.max_move = s.max_move.max(m.max_move);
                }
                s.state = self.multi_hold(&mut s, now, out);
            }
            self.multi.session = Some(s);
        }
    }
}
