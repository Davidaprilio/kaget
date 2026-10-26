//! Engine scenarios (corners, sliders, NumberPad, multi-finger, filter mode).
//! Test device: 3996 x 2242 units at 31 units/mm.

use super::outputs::FakeOutputs;
use super::Engine;
use crate::config::{Binding, Config};
use crate::filter::Filter;
use crate::filter::tests::FakeSink;
use crate::geometry::Geometry;
use crate::npad::{KEY_NUMLOCK, NumberPad};
use crate::sys::{self, InputEvent, event};
use crate::tracker::Tracker;

const KP5: u16 = 76;
const BACKSPACE: u16 = 14;

struct Sim {
    tr: Tracker,
    e: Engine,
    out: FakeOutputs,
    tid: i32,
    /// Filter mode: the clone's side.
    filt: Option<Filter<FakeSink>>,
}

fn config(bindings: &[(&str, &str)]) -> Config {
    Config {
        bindings: bindings.iter().map(|&(t, a)| Binding { trigger: t.into(), action: a.into(), when_running: None }).collect(),
        ..Default::default()
    }
}

fn sim_with(cfg: Config, numberpad: bool) -> Sim {
    let geo = Geometry::new(0, 3996, 0, 2242, (3996.0 / 31.0, 2242.0 / 31.0), &cfg);
    let npad = numberpad.then(|| NumberPad::new(crate::numberpad::test_profile()));
    Sim { tr: Tracker::default(), e: Engine::new(cfg, geo, 5, false, npad), out: FakeOutputs::default(), tid: 0, filt: None }
}

fn sim(bindings: &[(&str, &str)]) -> Sim {
    sim_with(config(bindings), false)
}

fn abs(code: u16, v: i32) -> InputEvent {
    event(sys::EV_ABS, code, v)
}

impl Sim {
    fn frame(&mut self, evs: &[InputEvent], t: f64) {
        self.tr.apply(evs, t);
        self.e.frame(&self.tr, t, &mut self.out);
        self.forward(evs, t);
        self.tr.remove_lifted();
    }
    fn forward(&mut self, evs: &[InputEvent], t: f64) {
        let replays: Vec<i32> = self.out.replays.drain(..).flatten().collect();
        if let Some(f) = self.filt.as_mut() {
            for (tids, extra) in self.out.extra.drain(..) {
                f.add_fingers(&tids, &extra);
            }
            let e = &self.e;
            f.process(evs, t, &|slot| e.hidden(slot), &replays);
        }
    }
    fn clone_sink(&self) -> &FakeSink {
        self.filt.as_ref().unwrap().sink()
    }
    fn touch(&mut self, t: f64, slot: i32, x: i32, y: i32) {
        self.tid += 1;
        let tid = self.tid;
        self.frame(&[abs(sys::ABS_MT_SLOT, slot), abs(sys::ABS_MT_TRACKING_ID, tid),
                     abs(sys::ABS_MT_POSITION_X, x), abs(sys::ABS_MT_POSITION_Y, y)], t);
    }
    fn mv(&mut self, t: f64, slot: i32, x: i32, y: i32) {
        self.frame(&[abs(sys::ABS_MT_SLOT, slot), abs(sys::ABS_MT_POSITION_X, x), abs(sys::ABS_MT_POSITION_Y, y)], t);
    }
    fn lift(&mut self, t: f64, slot: i32) {
        self.frame(&[abs(sys::ABS_MT_SLOT, slot), abs(sys::ABS_MT_TRACKING_ID, -1)], t);
    }
    fn tick(&mut self, t: f64) {
        self.e.tick(&self.tr, t, &mut self.out);
        self.forward(&[], t);
    }
    /// Straight line from (x, y) by (dx, dy) per 8 ms frame, `n` frames.
    fn drag(&mut self, t: f64, slot: i32, from: (i32, i32), step: (i32, i32), n: i32) -> f64 {
        for i in 1..=n {
            self.mv(t + i as f64 * 0.008, slot, from.0 + i * step.0, from.1 + i * step.1);
        }
        t + n as f64 * 0.008
    }
    /// Several fingers moving together.
    fn drag_all(&mut self, t: f64, fingers: &[(i32, i32, i32)], step: (i32, i32), n: i32) -> f64 {
        for i in 1..=n {
            let mut evs = Vec::new();
            for &(slot, x, y) in fingers {
                evs.extend([abs(sys::ABS_MT_SLOT, slot), abs(sys::ABS_MT_POSITION_X, x + i * step.0),
                            abs(sys::ABS_MT_POSITION_Y, y + i * step.1)]);
            }
            self.frame(&evs, t + i as f64 * 0.008);
        }
        t + n as f64 * 0.008
    }
    fn touch_all(&mut self, t: f64, fingers: &[(i32, i32, i32)]) {
        for (i, &(slot, x, y)) in fingers.iter().enumerate() {
            self.touch(t + i as f64 * 0.01, slot, x, y);
        }
    }
    fn lift_all(&mut self, t: f64, fingers: &[(i32, i32, i32)]) {
        for &(slot, _, _) in fingers {
            self.lift(t, slot);
        }
    }
    fn hold(&mut self, t0: f64, t1: f64, slot: i32, x: i32, y: i32) {
        let mut t = t0;
        while t < t1 {
            t += 0.008;
            self.mv(t, slot, x, y);
        }
    }
}

// ---- M2: corners and sliders ------------------------------------------------------------------

const G: &[(&str, &str)] = &[
    ("corner.top_right.hold", "cmd:echo hold"),
    ("corner.top_left.swipe.right", "brightness.up"),
    ("corner.top_left.swipe.diagonal", "cmd:kcalc"),
    ("corner.top_right.swipe.left", "brightness.down"),
    ("corner.top_right.swipe.down", "volume.down"),
];

#[test]
fn corner_hold_fires_once() {
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.hold(0.0, 0.45, 0, 3900, 100);
    s.lift(0.45, 0);
    assert_eq!(s.out.fired.len(), 1);
    assert_eq!(s.out.fired[0].trigger, "corner.top_right.hold");
}

#[test]
fn corner_hold_fires_from_timer_without_frames() {
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.tick(0.2);
    assert!(s.out.fired.is_empty());
    s.tick(0.33);
    assert_eq!(s.out.actions(), ["cmd:echo hold"]);
}

#[test]
fn corner_tap_does_nothing() {
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.lift(0.1, 0);
    s.tick(0.5);
    assert!(s.out.fired.is_empty());
}

#[test]
fn brightness_slider_right_then_back() {
    let mut s = sim(G);
    s.touch(0.0, 0, 100, 100);
    let t = s.drag(0.0, 0, (100, 100), (40, 0), 25); // ~32 mm
    assert_eq!(s.out.count("brightness.up"), 4);
    s.drag(t, 0, (1100, 100), (-40, 0), 12); // back ~15 mm
    assert_eq!(s.out.count("brightness.down"), 2);
    assert!(s.out.fired.iter().all(|f| f.trigger == "corner.top_left.swipe.right"));
}

#[test]
fn brightness_slider_left_from_top_right() {
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.drag(0.0, 0, (3900, 100), (-40, 0), 25);
    assert_eq!(s.out.count("brightness.down"), 4);
    assert_eq!(s.out.fired.len(), 4);
}

#[test]
fn volume_slider_down_then_up() {
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    let t = s.drag(0.0, 0, (3900, 100), (0, 40), 29); // ~37 mm
    assert_eq!(s.out.count("volume.down"), 4);
    s.drag(t, 0, (3900, 1260), (0, -40), 15); // ~19 mm back
    assert_eq!(s.out.count("volume.up"), 2);
    assert_eq!(s.out.fired.len(), 6);
}

#[test]
fn resting_finger_on_grid_line_does_not_flicker() {
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.drag(0.0, 0, (3900, 100), (0, 40), 8); // y = 420, just past the first step
    for i in 0..40 {
        s.mv(0.1 + i as f64 * 0.008, 0, 3900, 420 + if i % 2 == 1 { 3 } else { -3 });
    }
    assert_eq!(s.out.fired.len(), 1);
}

#[test]
fn diagonal_runs_once() {
    let mut s = sim(G);
    s.touch(0.0, 0, 100, 100);
    s.drag(0.0, 0, (100, 100), (40, 25), 19);
    s.lift(0.2, 0);
    assert_eq!(s.out.actions(), ["cmd:kcalc"]);
}

#[test]
fn unbound_direction_does_nothing() {
    let mut s = sim(G);
    s.touch(0.0, 0, 100, 100);
    s.drag(0.0, 0, (100, 100), (0, 40), 14); // top_left.swipe.down is not bound here
    s.lift(0.15, 0);
    s.tick(0.6);
    assert!(s.out.fired.is_empty());
}

#[test]
fn swipe_off_the_touchpad_from_a_corner_is_pointer_movement() {
    let mut s = filter_sim(G, false, false);
    // top right corner, ~10 mm up to the edge: there is no corner.top_right.swipe.up
    s.touch(0.0, 0, 3800, 300);
    s.drag(0.0, 0, (3800, 300), (0, -30), 10);
    assert_eq!(s.e.role(0), "pointer");
    s.lift(0.1, 0);
    s.tick(0.6);
    assert!(s.out.fired.is_empty());
    assert_eq!(clone_axis(&s, 1, sys::ABS_MT_POSITION_Y).last(), Some(&0), "the desktop got the movement");
}

#[test]
fn diagonal_off_the_touchpad_is_not_a_diagonal() {
    let mut s = sim(G);
    // top left corner, moving down and to the left: not towards the centre
    s.touch(0.0, 0, 240, 100);
    s.drag(0.0, 0, (240, 100), (-20, 25), 9);
    s.lift(0.1, 0);
    assert!(s.out.fired.is_empty());
}

#[test]
fn unbound_corner_is_ignored() {
    let mut s = sim(G);
    s.touch(0.0, 0, 100, 2200); // bottom left: no bindings
    s.hold(0.0, 0.5, 0, 100, 2200);
    assert!(s.out.fired.is_empty());
    assert_eq!(s.e.role(0), "pointer");
}

fn volume_from_corner(s: &mut Sim, t: f64) {
    s.touch(t, 0, 3900, 100);
    s.drag(t, 0, (3900, 100), (0, 40), 14);
    s.lift(t + 0.15, 0);
}

#[test]
fn continuation_swipe_up_in_right_band() {
    let mut s = sim(G);
    volume_from_corner(&mut s, 0.0);
    s.out.clear();
    s.touch(1.0, 0, 3500, 1600);
    s.drag(1.0, 0, (3500, 1600), (0, -40), 15); // up ~19 mm
    s.lift(1.2, 0);
    assert!(s.out.count("volume.up") >= 2, "{:?}", s.out.actions());
    assert!(s.out.fired.iter().all(|f| f.action == "volume.up"));
    // the window was refreshed: another continuation 1.5 s later still works
    s.touch(2.7, 0, 3600, 800);
    s.drag(2.7, 0, (3600, 800), (0, 40), 9);
    s.lift(2.8, 0);
    assert!(s.out.count("volume.down") >= 1);
}

#[test]
fn continuation_first_step_is_single_in_both_directions() {
    let mut s = sim(G);
    volume_from_corner(&mut s, 0.0);
    s.out.clear();
    s.touch(1.0, 0, 3500, 1000);
    s.drag(1.0, 0, (3500, 1000), (0, -40), 5); // ~6.5 mm: just past the 4.8 mm threshold
    assert_eq!(s.out.actions(), ["volume.up"]);
    s.lift(1.1, 0);
    s.out.clear();
    s.touch(1.5, 0, 3500, 1000);
    s.drag(1.5, 0, (3500, 1000), (0, 40), 5);
    assert_eq!(s.out.actions(), ["volume.down"]);
}

#[test]
fn after_window_nothing() {
    let mut s = sim(G);
    volume_from_corner(&mut s, 0.0);
    s.out.clear();
    s.touch(2.5, 0, 3500, 1600);
    s.drag(2.5, 0, (3500, 1600), (0, -40), 15);
    s.lift(2.7, 0);
    assert!(s.out.fired.is_empty());
}

#[test]
fn touch_outside_band_closes_window() {
    let mut s = sim(G);
    volume_from_corner(&mut s, 0.0);
    s.out.clear();
    s.touch(0.5, 0, 1000, 1000);
    s.lift(0.55, 0);
    s.touch(0.8, 0, 3500, 1600);
    s.drag(0.8, 0, (3500, 1600), (0, -40), 15);
    s.lift(1.0, 0);
    assert!(s.out.fired.is_empty());
}

#[test]
fn movement_across_axis_closes_window() {
    let mut s = sim(G);
    volume_from_corner(&mut s, 0.0);
    s.out.clear();
    s.touch(0.5, 0, 3500, 1600);
    s.drag(0.5, 0, (3500, 1600), (-40, 0), 10);
    s.lift(0.6, 0);
    s.touch(0.8, 0, 3500, 1600);
    s.drag(0.8, 0, (3500, 1600), (0, -40), 15);
    assert!(s.out.fired.is_empty());
}

#[test]
fn second_finger_cancels_corner_candidate() {
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.touch(0.02, 1, 2000, 1000);
    s.hold(0.02, 0.5, 0, 3900, 100);
    s.drag(0.5, 0, (3900, 100), (-40, 0), 20);
    s.lift(0.7, 0);
    s.lift(0.7, 1);
    assert!(s.out.fired.is_empty());
}

#[test]
fn disabled_engine_does_nothing() {
    let mut c = config(G);
    c.general.enabled = false;
    let mut s = sim_with(c, false);
    s.touch(0.0, 0, 3900, 100);
    s.hold(0.0, 0.45, 0, 3900, 100);
    assert!(s.out.fired.is_empty());
}

#[test]
fn pause_keeps_only_the_pause_binding() {
    let mut s = sim(&[("corner.top_left.hold", "kaget.pause"), ("corner.top_right.hold", "cmd:x")]);
    s.touch(0.0, 0, 100, 100);
    s.hold(0.0, 0.4, 0, 100, 100);
    s.lift(0.4, 0);
    assert!(s.e.paused);
    s.touch(1.0, 0, 3900, 100);
    s.hold(1.0, 1.4, 0, 3900, 100);
    s.lift(1.4, 0);
    assert_eq!(s.out.actions(), ["kaget.pause"]);
    s.touch(2.0, 0, 100, 100);
    s.hold(2.0, 2.4, 0, 100, 100);
    assert!(!s.e.paused);
}

// ---- corner tap-and-hold ----------------------------------------------------------------------

const TH: &[(&str, &str)] = &[
    ("corner.top_right.taphold", "cmd:th"),
    ("corner.top_right.hold", "cmd:hold"),
    ("corner.top_right.swipe.down", "volume.down"),
];

fn corner_tap(s: &mut Sim, t: f64) {
    s.touch(t, 0, 3900, 100);
    s.lift(t + 0.1, 0);
}

#[test]
fn corner_taphold_fires_instead_of_hold() {
    let mut s = sim(TH);
    corner_tap(&mut s, 0.0);
    s.touch(0.25, 0, 3900, 100);
    s.hold(0.25, 0.65, 0, 3900, 100);
    s.lift(0.7, 0);
    s.tick(1.5);
    assert_eq!(s.out.actions(), ["cmd:th"]);
}

#[test]
fn corner_hold_without_a_tap_stays_a_hold() {
    let mut s = sim(TH);
    s.touch(0.0, 0, 3900, 100);
    s.hold(0.0, 0.45, 0, 3900, 100);
    assert_eq!(s.out.actions(), ["cmd:hold"]);
}

#[test]
fn corner_touch_too_long_after_the_tap_is_a_hold() {
    let mut s = sim(TH);
    corner_tap(&mut s, 0.0);
    s.touch(0.5, 0, 3900, 100); // double_tap_ms (300) after the lift is over
    s.hold(0.5, 0.9, 0, 3900, 100);
    assert_eq!(s.out.actions(), ["cmd:hold"]);
}

#[test]
fn corner_taphold_needs_the_same_corner() {
    let mut s = sim(&[("corner.top_right.taphold", "cmd:th"), ("corner.top_left.hold", "cmd:left")]);
    corner_tap(&mut s, 0.0);
    s.touch(0.2, 0, 100, 100);
    s.hold(0.2, 0.6, 0, 100, 100);
    assert_eq!(s.out.actions(), ["cmd:left"]);
}

#[test]
fn filter_corner_taphold_never_reaches_the_desktop() {
    let mut s = filter_sim(TH, false, false);
    corner_tap(&mut s, 0.0);
    s.touch(0.25, 0, 3900, 100);
    s.hold(0.25, 0.65, 0, 3900, 100);
    s.lift(0.7, 0);
    s.tick(3.0);
    assert_eq!(s.out.actions(), ["cmd:th"]);
    assert!(s.clone_sink().frames.is_empty(), "no click, no touch");
}

#[test]
fn filter_corner_tap_clicks_once_no_taphold_follows() {
    let mut s = filter_sim(TH, false, false);
    corner_tap(&mut s, 0.0);
    s.tick(0.3);
    assert!(s.clone_sink().frames.is_empty(), "held back while a tap-and-hold may follow");
    s.tick(0.45);
    assert_eq!(s.clone_sink().touched(), [1].into());
    assert!(s.clone_sink().down().is_empty());
    assert!(s.out.fired.is_empty());
}

#[test]
fn filter_corner_double_tap_is_two_clicks() {
    let mut s = filter_sim(TH, false, false);
    corner_tap(&mut s, 0.0);
    corner_tap(&mut s, 0.2);
    assert_eq!(s.clone_sink().touched(), [1, 2].into());
    assert!(s.clone_sink().down().is_empty());
    s.tick(1.0);
    assert!(s.out.fired.is_empty());
}

#[test]
fn filter_corner_tap_then_swipe_clicks_and_slides() {
    let mut s = filter_sim(TH, false, false);
    corner_tap(&mut s, 0.0);
    s.touch(0.2, 0, 3900, 100);
    s.drag(0.2, 0, (3900, 100), (0, 40), 14);
    assert!(s.out.count("volume.down") >= 1);
    assert!(!s.out.fired.iter().any(|f| f.action == "cmd:th" || f.action == "cmd:hold"));
    assert_eq!(s.clone_sink().touched(), [1].into(), "the tap was a click, the swipe stays hidden");
}

// ---- diagnoses for the Test page --------------------------------------------------------------

fn only_diag(s: &Sim) -> &super::outputs::Diagnosis {
    assert_eq!(s.out.diags.len(), 1, "{:#?}", s.out.diags);
    &s.out.diags[0]
}

#[test]
fn diag_fired_swipe() {
    let mut s = filter_sim(USER, true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (0, 40), 12);
    assert!(s.out.diags.is_empty(), "reported when the fingers lift");
    s.lift_all(0.2, THREE);
    let d = only_diag(&s);
    assert_eq!((d.kind, d.fingers, d.trigger.as_str(), d.result, d.action.as_str()), ("multi", 3, "swipe.3.down", "fired", "cmd:shot"));
    assert!(d.travel_mm > 12.0 && d.span_ms > 0.0);
}

#[test]
fn diag_swipe_too_short() {
    let mut s = filter_sim(USER, true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (0, 40), 6); // ~7.7 mm
    s.lift_all(0.2, THREE);
    let d = only_diag(&s);
    assert_eq!((d.result, d.reason, d.need_mm), ("none", "short", 12.0));
    assert!((d.travel_mm - 7.7).abs() < 0.2, "{}", d.travel_mm);
}

#[test]
fn diag_late_finger() {
    let mut s = filter_sim(USER, true, false);
    for (&(slot, x, y), t) in FOUR.iter().zip([0.0, 0.02, 0.04, 0.3]) {
        s.touch(t, slot, x, y);
    }
    s.lift_all(0.5, FOUR);
    let d = only_diag(&s);
    assert_eq!((d.result, d.reason), ("kde", "late_finger"));
    assert!((d.late_ms - 300.0).abs() < 1.0, "{}", d.late_ms);
}

#[test]
fn diag_fingers_not_together() {
    let mut s = sim(&[("hold.3", "cmd:hold")]);
    s.touch(0.0, 0, 1500, 1000);
    s.touch(0.05, 1, 2000, 1000);
    s.touch(0.3, 2, 2500, 1000);
    s.lift_all(0.5, THREE);
    let d = only_diag(&s);
    assert_eq!((d.result, d.reason), ("kde", "not_together"));
    assert!((d.span_ms - 300.0).abs() < 1.0);
}

#[test]
fn diag_nothing_bound_for_that_many_fingers() {
    let mut s = filter_sim(&[("swipe.4.left", "cmd:x")], true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (0, 40), 3);
    s.lift_all(0.2, THREE);
    let d = only_diag(&s);
    assert_eq!((d.result, d.reason), ("kde", "count"));
}

#[test]
fn diag_tap_goes_to_kde() {
    let mut s = filter_sim(USER, true, false);
    s.touch_all(0.0, THREE);
    s.lift_all(0.1, THREE);
    let d = only_diag(&s);
    assert_eq!((d.trigger.as_str(), d.result, d.reason), ("tap.3", "kde", "unbound"));
}

#[test]
fn diag_handover() {
    let mut s = filter_sim(&[("swipe.3.up", "system:4")], true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (0, -40), 12);
    s.lift_all(0.2, THREE);
    let d = only_diag(&s);
    assert_eq!((d.trigger.as_str(), d.result, d.action.as_str(), d.reason), ("swipe.3.up", "kde", "system:4", "handover"));
}

#[test]
fn diag_corner_outcomes() {
    // hold
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.hold(0.0, 0.45, 0, 3900, 100);
    s.lift(0.5, 0);
    let d = only_diag(&s);
    assert_eq!((d.kind, d.corner, d.trigger.as_str(), d.result), ("corner", "top_right", "corner.top_right.hold", "fired"));
    // a slider: one report with its steps
    let mut s = sim(G);
    s.touch(0.0, 0, 100, 100);
    s.drag(0.0, 0, (100, 100), (40, 0), 25);
    s.lift(0.3, 0);
    let d = only_diag(&s);
    assert_eq!((d.result, d.steps), ("slider", 4));
    // too short for a swipe
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.drag(0.0, 0, (3900, 100), (-40, 0), 5); // 6.5 mm
    s.lift(0.1, 0);
    let d = only_diag(&s);
    assert_eq!((d.result, d.reason, d.need_mm), ("none", "short", 7.0));
    // off the touchpad
    let mut s = sim(G);
    s.touch(0.0, 0, 3800, 300);
    s.drag(0.0, 0, (3800, 300), (0, -30), 10);
    s.lift(0.1, 0);
    assert_eq!((only_diag(&s).result, only_diag(&s).reason), ("kde", "off_pad"));
    // a tap, and one lifted before the hold time
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.lift(0.1, 0);
    assert_eq!(only_diag(&s).reason, "tap");
    let mut s = sim(G);
    s.touch(0.0, 0, 3900, 100);
    s.hold(0.0, 0.28, 0, 3900, 100);
    s.lift(0.29, 0);
    let d = only_diag(&s);
    assert_eq!((d.result, d.reason, d.need_ms), ("none", "early", 320.0));
}

#[test]
fn diag_corner_taphold_is_one_report() {
    let mut s = sim(TH);
    corner_tap(&mut s, 0.0);
    assert!(s.out.diags.is_empty(), "the tap waits");
    s.touch(0.25, 0, 3900, 100);
    s.hold(0.25, 0.65, 0, 3900, 100);
    s.lift(0.7, 0);
    let d = only_diag(&s);
    assert_eq!((d.trigger.as_str(), d.result), ("corner.top_right.taphold", "fired"));
}

// ---- M5: NumberPad ----------------------------------------------------------------------------

const NP: &[(&str, &str)] = &[
    ("corner.top_right.hold", "numberpad.toggle"),
    ("corner.top_left.hold", "numberpad.brightness_cycle"),
    ("corner.top_right.swipe.down", "volume.down"),
];

fn numberpad(active: bool) -> Sim {
    let mut s = sim_with(config(NP), true);
    if active {
        s.e.set_numlock(true, &mut s.out);
    }
    s.out.clear();
    s
}

#[test]
fn tap_kp5_sends_key() {
    let mut s = numberpad(true);
    s.touch(0.0, 0, 1200, 1000);
    s.mv(0.03, 0, 1205, 1002);
    s.lift(0.06, 0);
    assert_eq!(s.out.keys, [(KP5, true), (KP5, false)]);
}

#[test]
fn hold_backspace_down_until_lift() {
    let mut s = numberpad(true);
    s.touch(0.0, 0, 3400, 500);
    s.hold(0.0, 0.3, 0, 3400, 500);
    assert_eq!(s.out.keys, [(BACKSPACE, true)]);
    s.lift(0.3, 0);
    assert_eq!(s.out.keys, [(BACKSPACE, true), (BACKSPACE, false)]);
}

#[test]
fn percent_key_sends_shift_combo_in_order() {
    let mut s = numberpad(true);
    s.touch(0.0, 0, 3400, 1500);
    s.lift(0.05, 0);
    assert_eq!(s.out.keys, [(42, true), (6, true), (6, false), (42, false)]);
}

#[test]
fn slide_from_key_sends_nothing() {
    let mut s = numberpad(true);
    s.touch(0.0, 0, 1200, 1000);
    s.drag(0.0, 0, (1200, 1000), (40, 0), 7);
    s.lift(0.1, 0);
    assert!(s.out.keys.is_empty());
}

#[test]
fn inactive_key_area_is_pointer() {
    let mut s = numberpad(false);
    s.touch(0.0, 0, 1200, 1000);
    s.lift(0.05, 0);
    assert!(s.out.keys.is_empty());
}

#[test]
fn toggle_sends_numlock_and_switches_led_and_pointer() {
    let mut s = numberpad(false);
    s.touch(0.0, 0, 3900, 100);
    s.hold(0.0, 0.45, 0, 3900, 100);
    s.lift(0.45, 0);
    assert_eq!(s.out.keys, [(KEY_NUMLOCK, true), (KEY_NUMLOCK, false)]);
    assert!(s.e.npad.as_ref().unwrap().active);
    assert_eq!(s.out.leds, [0x40, 0x01]);
    assert_eq!(s.out.pointer, [false]);
    assert!(s.out.fired[0].local);
    // the compositor echoes NumLock: no second switch
    s.e.set_numlock(true, &mut s.out);
    assert_eq!(s.out.pointer, [false]);
    // toggle off again
    s.touch(1.0, 0, 3900, 100);
    s.hold(1.0, 1.45, 0, 3900, 100);
    assert_eq!(s.out.pointer, [false, true]);
    assert_eq!(*s.out.leds.last().unwrap(), 0x00);
}

#[test]
fn brightness_cycle_only_while_active() {
    let mut s = numberpad(false);
    s.touch(0.0, 0, 100, 100);
    s.hold(0.0, 0.45, 0, 100, 100);
    s.lift(0.45, 0);
    assert!(s.out.leds.is_empty() && s.out.fired.is_empty());
    let mut s = numberpad(true);
    s.touch(0.0, 0, 100, 100);
    s.hold(0.0, 0.45, 0, 100, 100);
    s.lift(0.45, 0);
    assert_eq!(s.out.leds, [0x41, 0x01]);
}

#[test]
fn shutdown_restores_pointer() {
    let mut s = numberpad(true);
    s.e.shutdown(&mut s.out);
    assert_eq!(s.out.pointer, [true]);
    assert_eq!(s.out.leds, [0x00]);
}

#[test]
fn tap_on_key_in_continuation_window_still_types() {
    let mut s = numberpad(true);
    volume_from_corner(&mut s, 0.0);
    s.out.clear();
    s.touch(0.8, 0, 3400, 500);
    s.lift(0.85, 0);
    assert_eq!(s.out.keys, [(BACKSPACE, true), (BACKSPACE, false)]);
}

#[test]
fn continuation_over_keys_moves_slider_not_keys() {
    let mut s = numberpad(true);
    volume_from_corner(&mut s, 0.0);
    s.out.clear();
    s.touch(1.0, 0, 3500, 1600);
    s.drag(1.0, 0, (3500, 1600), (0, -40), 15);
    s.lift(1.2, 0);
    assert!(s.out.keys.is_empty());
    assert!(s.out.count("volume.up") >= 2);
}

#[test]
fn second_finger_releases_held_key() {
    let mut s = numberpad(true);
    s.touch(0.0, 0, 3400, 500);
    s.hold(0.0, 0.2, 0, 3400, 500);
    s.touch(0.2, 1, 1200, 1000);
    assert_eq!(s.out.keys, [(BACKSPACE, true), (BACKSPACE, false)]);
    s.lift(0.3, 0);
    s.lift(0.3, 1);
    assert_eq!(s.out.keys.len(), 2);
}

// ---- M6: multi-finger -------------------------------------------------------------------------

const THREE: &[(i32, i32, i32)] = &[(0, 1500, 1000), (1, 2000, 1000), (2, 2500, 1000)];
const FIVE: &[(i32, i32, i32)] = &[(0, 1000, 1200), (1, 1500, 900), (2, 2000, 800), (3, 2500, 900), (4, 3000, 1200)];

#[test]
fn two_fingers_never_start_a_session() {
    let mut s = sim(&[("doubletap.3", "cmd:a"), ("hold.3", "cmd:b")]);
    s.touch(0.0, 0, 1500, 1000);
    s.touch(0.01, 1, 2000, 1000);
    s.hold(0.01, 0.8, 0, 1500, 1000);
    s.lift(0.8, 0);
    s.lift(0.8, 1);
    s.tick(1.5);
    assert!(s.out.fired.is_empty());
}

#[test]
fn system_owned_never_fires_in_passive_mode() {
    let mut s = sim(&[("swipe.3.left", "cmd:a"), ("tap.3", "cmd:b")]);
    s.touch_all(0.0, THREE);
    s.lift_all(0.1, THREE);
    s.touch_all(1.0, THREE);
    s.drag_all(1.02, THREE, (-40, 0), 15);
    s.lift_all(1.2, THREE);
    s.tick(2.0);
    assert!(s.out.fired.is_empty());
}

#[test]
fn hold_three() {
    let mut s = sim(&[("hold.3", "cmd:hold")]);
    s.touch_all(0.0, THREE);
    s.tick(0.4);
    assert!(s.out.fired.is_empty());
    s.tick(0.52);
    assert_eq!(s.out.actions(), ["cmd:hold"]);
    s.tick(0.9);
    s.lift_all(1.0, THREE);
    assert_eq!(s.out.fired.len(), 1);
}

#[test]
fn doubletap_three() {
    let mut s = sim(&[("doubletap.3", "cmd:dt")]);
    s.touch_all(0.0, THREE);
    s.lift_all(0.1, THREE);
    s.touch_all(0.25, THREE);
    s.lift_all(0.35, THREE);
    s.tick(1.0);
    assert_eq!(s.out.actions(), ["cmd:dt"]);
}

#[test]
fn doubletap_too_slow_is_two_taps() {
    let mut s = sim(&[("doubletap.4", "cmd:dt"), ("tap.4", "cmd:tap")]);
    let four = &[(0, 1500, 1000), (1, 2000, 1000), (2, 2500, 1000), (3, 3000, 1000)];
    s.touch_all(0.0, four);
    s.lift_all(0.1, four);
    s.tick(0.3);
    assert!(s.out.fired.is_empty());
    s.tick(0.45);
    assert_eq!(s.out.actions(), ["cmd:tap"]);
    s.touch_all(1.0, four);
    s.lift_all(1.1, four);
    s.tick(2.0);
    assert_eq!(s.out.actions(), ["cmd:tap", "cmd:tap"]);
}

#[test]
fn taphold_three() {
    let mut s = sim(&[("taphold.3", "cmd:th"), ("hold.3", "cmd:hold")]);
    s.touch_all(0.0, THREE);
    s.lift_all(0.1, THREE);
    s.touch_all(0.3, THREE);
    s.tick(0.9);
    assert_eq!(s.out.actions(), ["cmd:th"]);
}

#[test]
fn tap_four_waits_only_when_doubletap_bound() {
    let mut s = sim(&[("tap.4", "cmd:tap")]);
    let four = &[(0, 1500, 1000), (1, 2000, 1000), (2, 2500, 1000), (3, 3000, 1000)];
    s.touch_all(0.0, four);
    s.lift_all(0.1, four);
    assert_eq!(s.out.actions(), ["cmd:tap"]);
}

#[test]
fn slow_tap_is_not_a_tap() {
    let mut s = sim(&[("tap.4", "cmd:tap")]);
    let four = &[(0, 1500, 1000), (1, 2000, 1000), (2, 2500, 1000), (3, 3000, 1000)];
    s.touch_all(0.0, four);
    s.lift_all(0.4, four);
    assert!(s.out.fired.is_empty());
}

#[test]
fn fingers_not_landing_together_are_ignored() {
    let mut s = sim(&[("hold.3", "cmd:hold")]);
    s.touch(0.0, 0, 1500, 1000);
    s.touch(0.05, 1, 2000, 1000);
    s.touch(0.3, 2, 2500, 1000);
    s.tick(1.5);
    assert!(s.out.fired.is_empty());
}

#[test]
fn swipe_five_fires_once() {
    let mut s = sim(&[("swipe.5.up", "kwin:Overview")]);
    s.touch_all(0.0, FIVE);
    s.drag_all(0.05, FIVE, (0, -30), 20);
    s.lift_all(0.3, FIVE);
    assert_eq!(s.out.actions(), ["kwin:Overview"]);
}

#[test]
fn swipe_five_slider() {
    let mut s = sim(&[("swipe.5.right", "volume.up")]);
    s.touch_all(0.0, FIVE);
    let t = s.drag_all(0.05, FIVE, (31, 0), 20); // 20 mm
    let ups = s.out.count("volume.up");
    assert!((2..=3).contains(&ups), "{ups}");
    let moved: Vec<(i32, i32, i32)> = FIVE.iter().map(|&(sl, x, y)| (sl, x + 620, y)).collect();
    s.drag_all(t, &moved, (-31, 0), 10);
    assert_eq!(s.out.count("volume.down"), 1);
}

#[test]
fn pinch_five_in_and_out() {
    let mut s = sim(&[("pinch.5.in", "cmd:in"), ("pinch.5.out", "cmd:out")]);
    s.touch_all(0.0, FIVE);
    // move every finger 40 % of the way to the centroid (2000, 1000)
    for i in 1..=10 {
        let mut evs = Vec::new();
        for &(slot, x, y) in FIVE {
            let f = 0.04 * i as f64;
            evs.extend([abs(sys::ABS_MT_SLOT, slot),
                        abs(sys::ABS_MT_POSITION_X, x + ((2000 - x) as f64 * f) as i32),
                        abs(sys::ABS_MT_POSITION_Y, y + ((1000 - y) as f64 * f) as i32)]);
        }
        s.frame(&evs, 0.05 + i as f64 * 0.008);
    }
    s.lift_all(0.3, FIVE);
    assert_eq!(s.out.actions(), ["cmd:in"]);
}

#[test]
fn sidedrag_top_three() {
    let mut s = sim(&[("sidedrag.top.3", "kwin:Overview")]);
    let top = &[(0, 1500, 60), (1, 2000, 50), (2, 2500, 60)];
    s.touch_all(0.0, top);
    assert_eq!(s.e.role(0), "sidedrag");
    s.drag_all(0.05, top, (0, 30), 15); // 14.5 mm inward
    s.lift_all(0.3, top);
    assert_eq!(s.out.actions(), ["kwin:Overview"]);
}

#[test]
fn sidedrag_needs_all_fingers_in_band() {
    let mut s = sim(&[("sidedrag.top.3", "kwin:Overview")]);
    let top = &[(0, 1500, 60), (1, 2000, 400), (2, 2500, 60)];
    s.touch_all(0.0, top);
    s.drag_all(0.05, top, (0, 30), 15);
    assert!(s.out.fired.is_empty());
}

#[test]
fn semi_mt_has_no_multi_finger_gestures() {
    let cfg = config(&[("hold.3", "cmd:hold")]);
    let geo = Geometry::new(0, 3996, 0, 2242, (128.9, 72.3), &cfg);
    let mut s = Sim { tr: Tracker::default(), e: Engine::new(cfg, geo, 5, true, None), out: FakeOutputs::default(), tid: 0, filt: None };
    s.touch_all(0.0, THREE);
    s.tick(1.0);
    assert!(s.out.fired.is_empty());
}

// ---- filter mode: withholding and overriding KDE gestures -----------------------------------------

const TOOLS: &[u16] = &[0x14a, 0x145, 0x14d, 0x14e, 0x14f, 0x148];

fn filter_sim(bindings: &[(&str, &str)], override_system: bool, numberpad: bool) -> Sim {
    let mut c = config(bindings);
    c.general.mode = "filter".into();
    c.general.override_system_gestures = override_system;
    let mut s = sim_with(c, numberpad);
    s.e.set_filter(true);
    s.filt = Some(Filter::new(FakeSink::default(), 5, TOOLS));
    s
}

/// Positions the clone ever reported for a tracking id.
fn clone_xs(s: &Sim, tid: i32) -> Vec<i32> {
    clone_axis(s, tid, sys::ABS_MT_POSITION_X)
}

fn clone_axis(s: &Sim, tid: i32, code: u16) -> Vec<i32> {
    let mut slot_tid = std::collections::BTreeMap::new();
    let mut slot = 0;
    let mut vs = Vec::new();
    for e in s.clone_sink().events().filter(|e| e.type_ == sys::EV_ABS) {
        match e.code {
            sys::ABS_MT_SLOT => slot = e.value,
            sys::ABS_MT_TRACKING_ID => {
                slot_tid.insert(slot, e.value);
            }
            c if c == code && slot_tid.get(&slot) == Some(&tid) => vs.push(e.value),
            _ => {}
        }
    }
    vs
}

#[test]
fn filter_pointer_passes_through_at_once() {
    let mut s = filter_sim(G, false, false);
    s.touch(0.0, 0, 2000, 1000);
    assert_eq!(s.clone_sink().down(), [1].into());
    s.drag(0.0, 0, (2000, 1000), (10, 0), 3);
    assert_eq!(clone_xs(&s, 1), [2000, 2010, 2020, 2030]);
}

#[test]
fn filter_corner_hold_and_slider_stay_hidden() {
    let mut s = filter_sim(G, false, false);
    s.touch(0.0, 0, 3900, 100);
    s.hold(0.0, 0.45, 0, 3900, 100);
    s.lift(0.45, 0);
    s.touch(1.0, 0, 100, 100);
    s.drag(1.0, 0, (100, 100), (40, 0), 25);
    s.lift(1.3, 0);
    s.tick(2.0);
    assert_eq!(s.out.fired.len(), 5);
    assert!(s.clone_sink().frames.is_empty(), "nothing reached the desktop");
}

#[test]
fn filter_short_corner_tap_is_replayed_as_tap() {
    let mut s = filter_sim(G, false, false);
    s.touch(0.0, 0, 3900, 100);
    s.lift(0.1, 0);
    assert!(s.out.fired.is_empty());
    assert_eq!(s.clone_sink().touched(), [1].into());
    assert!(s.clone_sink().down().is_empty());
}

#[test]
fn filter_unbound_corner_swipe_is_revealed_with_its_path() {
    let mut s = filter_sim(G, false, false);
    s.touch(0.0, 0, 100, 100);
    s.drag(0.0, 0, (100, 100), (0, 40), 14); // top_left.swipe.down is not bound in G
    let xs = clone_xs(&s, 1);
    assert_eq!(xs.first(), Some(&100));
    assert_eq!(s.clone_sink().down(), [1].into());
    assert!(s.out.fired.is_empty());
}

#[test]
fn filter_two_finger_scroll_is_live() {
    let mut s = filter_sim(G, true, false);
    let two = &[(0, 1500, 1000), (1, 2000, 1000)];
    s.touch_all(0.0, two);
    assert_eq!(s.clone_sink().down(), [1, 2].into());
    s.drag_all(0.02, two, (0, 20), 5);
    assert!(s.clone_sink().events().filter(|e| e.code == sys::ABS_MT_POSITION_Y && e.value == 1100).count() == 2);
}

#[test]
fn filter_overrides_kde_three_finger_swipe() {
    let mut s = filter_sim(&[("swipe.3.left", "kwin:Switch One Desktop to the Right")], true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (-40, 0), 15);
    s.lift_all(0.2, THREE);
    assert_eq!(s.out.actions(), ["kwin:Switch One Desktop to the Right"]);
    let c = s.clone_sink();
    assert!(!c.touched().contains(&3), "third finger never reached KDE");
    // the first two were already down: frozen in place, never moved
    assert_eq!(clone_xs(&s, 1), [1500]);
    assert_eq!(clone_xs(&s, 2), [2000]);
    assert_eq!(s.clone_sink().down(), [1, 2].into(), "lift delayed past libinput's tap timeout");
    s.tick(0.3);
    assert!(s.clone_sink().down().is_empty());
}

#[test]
fn filter_other_directions_still_reach_kde() {
    let mut s = filter_sim(&[("swipe.3.left", "kwin:Switch One Desktop to the Right"), ("swipe.3.up", "system")], true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (0, -40), 15);
    assert!(s.out.fired.is_empty());
    let c = s.clone_sink();
    assert_eq!(c.down(), [1, 2, 3].into(), "revealed: KDE gets the swipe");
    assert!(c.events().any(|e| e.code == sys::ABS_MT_POSITION_Y && e.value == 1000 - 15 * 40));
}

#[test]
fn filter_without_override_leaves_system_swipes_alone() {
    let mut s = filter_sim(&[("swipe.3.left", "cmd:x")], false, false);
    s.touch_all(0.0, THREE);
    assert_eq!(s.clone_sink().down(), [1, 2, 3].into());
    s.drag_all(0.03, THREE, (-40, 0), 15);
    assert!(s.out.fired.is_empty());
}

#[test]
fn filter_three_finger_tap_goes_to_kde_unless_doubled() {
    let mut s = filter_sim(&[("doubletap.3", "cmd:dt")], true, false);
    s.touch_all(0.0, THREE);
    s.lift_all(0.1, THREE);
    assert!(!s.clone_sink().touched().contains(&3), "held back while waiting for a second tap");
    s.tick(0.5);
    assert!(s.clone_sink().touched().contains(&3), "single tap replayed: libinput's middle click");
    assert!(s.out.fired.is_empty());
    // a real double tap never reaches KDE
    let mut s = filter_sim(&[("doubletap.3", "cmd:dt")], true, false);
    s.touch_all(0.0, THREE);
    s.lift_all(0.1, THREE);
    s.touch_all(0.25, THREE);
    s.lift_all(0.35, THREE);
    s.tick(1.0);
    assert_eq!(s.out.actions(), ["cmd:dt"]);
    let t = s.clone_sink().touched();
    assert!(!t.contains(&3) && !t.contains(&6), "{t:?}");
}

#[test]
fn filter_none_swallows_a_kde_gesture() {
    let mut s = filter_sim(&[("swipe.4.up", "none")], true, false);
    let four = &[(0, 1500, 1000), (1, 2000, 1000), (2, 2500, 1000), (3, 3000, 1000)];
    s.touch_all(0.0, four);
    s.drag_all(0.05, four, (0, -40), 15);
    s.lift_all(0.3, four);
    s.tick(1.0);
    assert!(s.out.fired.is_empty());
    let t = s.clone_sink().touched();
    assert!(!t.contains(&3) && !t.contains(&4));
    assert!(s.clone_sink().down().is_empty());
}

// ---- multi-finger robustness: sloppy landings, bouncing fingers, imperfect swipes -----------------

const USER: &[(&str, &str)] = &[
    ("swipe.3.down", "cmd:shot"),
    ("swipe.4.left", "kwin:Window One Desktop to the Right"),
    ("swipe.4.right", "kwin:Window One Desktop to the Left"),
];
const FOUR: &[(i32, i32, i32)] = &[(0, 1500, 1000), (1, 2000, 900), (2, 2500, 900), (3, 3000, 1000)];

#[test]
fn late_fourth_finger_still_makes_a_four_finger_swipe() {
    let mut s = filter_sim(USER, true, false);
    for (&(slot, x, y), t) in FOUR.iter().zip([0.0, 0.03, 0.06, 0.15]) {
        s.touch(t, slot, x, y);
    }
    s.drag_all(0.16, FOUR, (-40, 0), 15); // ~19 mm left
    s.lift_all(0.4, FOUR);
    s.tick(1.0);
    assert_eq!(s.out.actions(), ["kwin:Window One Desktop to the Right"]);
    assert_eq!(s.out.fired[0].trigger, "swipe.4.left");
    let t = s.clone_sink().touched();
    assert!(!t.contains(&3) && !t.contains(&4), "KDE never got the gesture: {t:?}");
}

#[test]
fn staggered_three_fingers_still_swipe() {
    let mut s = filter_sim(USER, true, false);
    for (&(slot, x, y), t) in THREE.iter().zip([0.0, 0.07, 0.16]) {
        s.touch(t, slot, x, y);
    }
    s.drag_all(0.17, THREE, (0, 40), 12); // ~15 mm down
    assert_eq!(s.out.actions(), ["cmd:shot"]);
}

#[test]
fn slider_plus_two_fingers_is_not_a_session() {
    let mut s = sim(G);
    s.touch(0.0, 0, 100, 100);
    let t = s.drag(0.0, 0, (100, 100), (40, 0), 10); // top left corner: brightness slider running
    let before = s.out.count("brightness.up");
    assert!(before >= 1);
    s.touch(t + 0.01, 1, 2000, 1500);
    s.touch(t + 0.02, 2, 2500, 1500);
    s.drag(t + 0.02, 0, (500, 100), (40, 0), 15);
    assert!(s.out.count("brightness.up") > before, "the slider keeps going");
    assert!(s.out.fired.iter().all(|f| f.trigger == "corner.top_left.swipe.right"));
}

#[test]
fn bouncing_finger_keeps_the_swipe() {
    let mut s = filter_sim(USER, true, false);
    s.touch_all(0.0, THREE);
    s.tick(0.25);
    let t = s.drag_all(0.25, THREE, (0, 20), 6); // ~4 mm down
    // the middle finger loses contact for a moment and comes back as a new contact
    s.lift(t, 1);
    s.touch(t + 0.03, 1, 2000, 1000 + 6 * 20 + 20);
    let moved: Vec<(i32, i32, i32)> = THREE.iter().map(|&(sl, x, y)| (sl, x, y + 6 * 20 + 20)).collect();
    s.drag_all(t + 0.03, &moved, (0, 40), 10);
    assert_eq!(s.out.actions(), ["cmd:shot"]);
    assert!(!s.clone_sink().touched().contains(&3), "not handed to KDE");
}

#[test]
fn bouncing_finger_does_not_jerk_a_slider() {
    let mut s = filter_sim(&[("swipe.3.right", "volume.up")], true, false);
    s.touch_all(0.0, THREE);
    let t = s.drag_all(0.03, THREE, (31, 0), 20); // 20 mm right
    let ups = s.out.count("volume.up");
    assert!(ups >= 2, "{ups}");
    s.lift(t, 2);
    s.touch(t + 0.02, 2, 2500 + 620, 1000);
    let moved: Vec<(i32, i32, i32)> = THREE.iter().map(|&(sl, x, y)| (sl, x + 620, y)).collect();
    s.drag_all(t + 0.02, &moved, (31, 0), 10); // 10 mm further right
    assert_eq!(s.out.count("volume.down"), 0, "no step back");
    assert!(s.out.count("volume.up") > ups, "the slider keeps going");
    assert!(!s.clone_sink().touched().contains(&3), "not handed to KDE");
}

#[test]
fn extra_finger_after_a_fired_swipe_does_not_hand_it_to_kde() {
    let mut s = filter_sim(&[("swipe.3.left", "kwin:Switch One Desktop to the Right")], true, false);
    s.touch_all(0.0, THREE);
    let t = s.drag_all(0.03, THREE, (-40, 0), 15);
    assert_eq!(s.out.fired.len(), 1);
    s.touch(t + 0.05, 3, 3000, 1500);
    s.lift_all(t + 0.1, THREE);
    s.lift(t + 0.1, 3);
    s.tick(t + 1.0);
    let c = s.clone_sink();
    assert!(!c.touched().contains(&3), "{:?}", c.touched());
    assert_eq!(clone_xs(&s, 1), [1500], "the frozen fingers never moved");
}

// ---- system:<n>: KWin's own gesture (and its animation) with a different finger count -------------

/// Tracking id of the first synthesized finger in the clone.
const EXTRA: i32 = 0xfff0;
const QUADTAP: u16 = 0x14f;

#[test]
fn three_finger_swipe_up_reaches_kde_as_four_fingers() {
    let mut s = filter_sim(&[("swipe.3.up", "system:4"), ("swipe.3.down", "cmd:shot")], true, false);
    s.touch_all(0.0, THREE);
    let t = s.drag_all(0.03, THREE, (0, -40), 12); // ~15 mm up: recognised, handed over
    assert_eq!(s.out.actions(), ["system:4"]);
    assert!(s.out.fired[0].local);
    let c = s.clone_sink();
    assert_eq!(c.down(), [1, 2, 3, EXTRA].into(), "KDE sees four fingers");
    assert_eq!(c.key(QUADTAP), [1]);
    // in line with the hand, one finger spacing right of it, and it took the whole way up
    let xs = clone_xs(&s, EXTRA);
    assert!(xs.iter().all(|&x| x == 3000), "{xs:?}");
    let ys = clone_axis(&s, EXTRA, sys::ABS_MT_POSITION_Y);
    assert_eq!((ys.first(), ys.last()), (Some(&1000), Some(&(1000 - 12 * 40))));
    // it keeps following the hand live
    let moved: Vec<(i32, i32, i32)> = THREE.iter().map(|&(sl, x, y)| (sl, x, y - 12 * 40)).collect();
    let t = s.drag_all(t, &moved, (0, -40), 5);
    assert_eq!(clone_axis(&s, EXTRA, sys::ABS_MT_POSITION_Y).last(), Some(&(1000 - 17 * 40)));
    // the first lifted finger ends it
    s.lift(t + 0.01, 0);
    assert_eq!(s.clone_sink().down(), [2, 3].into());
    s.lift(t + 0.02, 1);
    s.lift(t + 0.02, 2);
    assert!(s.clone_sink().down().is_empty());
    assert_eq!(s.out.fired.len(), 1);
}

#[test]
fn synthesized_finger_goes_where_there_is_room() {
    let mut s = filter_sim(&[("swipe.3.down", "system:4")], true, false);
    let right = &[(0, 2800, 800), (1, 3250, 700), (2, 3700, 800)];
    s.touch_all(0.0, right);
    s.drag_all(0.03, right, (0, 40), 12);
    assert_eq!(s.clone_sink().down(), [1, 2, 3, EXTRA].into());
    let x = *clone_xs(&s, EXTRA).last().unwrap();
    assert_eq!(x, 2800 - 450, "left of the hand, the right edge is too close");
}

#[test]
fn other_directions_stay_three_finger_gestures() {
    let mut s = filter_sim(&[("swipe.3.up", "system:4"), ("swipe.3.down", "system:4")], true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (-40, 0), 12);
    assert!(s.out.fired.is_empty());
    assert_eq!(s.clone_sink().down(), [1, 2, 3].into());
}

#[test]
fn four_finger_desktop_switch_takes_the_window_along() {
    let mut s = filter_sim(&[("swipe.4.left", "system:4:window")], true, false);
    s.touch_all(0.0, FOUR);
    let t = s.drag_all(0.05, FOUR, (-40, 0), 12);
    assert_eq!(s.out.actions(), ["system:4:window"]);
    assert_eq!(s.out.carry, [true], "pinned as KWin's switch starts");
    assert_eq!(s.clone_sink().down(), [1, 2, 3, 4].into(), "four real fingers: nothing synthesized");
    let moved: Vec<(i32, i32, i32)> = FOUR.iter().map(|&(sl, x, y)| (sl, x - 12 * 40, y)).collect();
    let t = s.drag_all(t, &moved, (-40, 0), 10);
    s.lift_all(t + 0.01, FOUR);
    assert_eq!(s.out.carry, [true, false], "let go once the fingers lifted");
    assert_eq!(s.out.fired.len(), 1);
}

#[test]
fn three_finger_desktop_switch_takes_the_window_along() {
    let mut s = filter_sim(&[("swipe.3.right", "system:4:window")], true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (40, 0), 12);
    assert_eq!(s.out.carry, [true]);
    assert_eq!(s.clone_sink().down(), [1, 2, 3, EXTRA].into());
}

#[test]
fn window_carry_is_only_for_sideways_swipes() {
    let mut s = filter_sim(&[("swipe.3.up", "system:4:window")], true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (0, -40), 12);
    assert!(s.out.fired.is_empty() && s.out.carry.is_empty());
}

#[test]
fn reset_lets_a_carried_window_go() {
    let mut s = filter_sim(&[("swipe.3.left", "system:4:window")], true, false);
    s.touch_all(0.0, THREE);
    s.drag_all(0.03, THREE, (-40, 0), 12);
    s.e.reset(&mut s.out);
    assert_eq!(s.out.carry, [true, false]);
}

#[test]
fn system_as_needs_filter_mode() {
    let mut s = sim(&[("swipe.5.up", "system:4")]);
    s.touch_all(0.0, FIVE);
    s.drag_all(0.05, FIVE, (0, -30), 20);
    assert!(s.out.fired.is_empty() && s.out.extra.is_empty());
}

#[test]
fn filter_numberpad_keys_are_withheld_and_pointer_stays_on() {
    let mut s = filter_sim(NP, false, true);
    s.e.set_numlock(true, &mut s.out);
    s.touch(0.0, 0, 1200, 1000);
    s.lift(0.06, 0);
    assert_eq!(s.out.keys, [(KP5, true), (KP5, false)]);
    assert!(s.clone_sink().frames.is_empty());
    // a key touch that moves is pointer movement (hybrid mode)
    s.touch(1.0, 0, 1200, 1000);
    s.drag(1.0, 0, (1200, 1000), (40, 0), 7);
    assert_eq!(s.clone_sink().down(), [2].into());
    assert_eq!(s.out.keys.len(), 2);
}

// ---- panel mode: the touchpad becomes the control panel's sliders ---------------------------------

use super::outputs::PanelEvent;

const PANEL: &[(&str, &str)] = &[("corner.top_right.hold", "kaget.panel"), ("corner.top_right.swipe.down", "volume.down")];

/// Sum of the Adjust deltas for one control.
fn adjusted(s: &Sim, kind: &str) -> f64 {
    s.out
        .panel
        .iter()
        .filter_map(|e| match e {
            PanelEvent::Adjust { kind: k, delta } if k == kind => Some(*delta),
            _ => None,
        })
        .sum()
}

fn open_panel(s: &mut Sim) {
    s.touch(0.0, 0, 3900, 100);
    s.hold(0.0, 0.4, 0, 3900, 100);
    s.lift(0.4, 0);
}

#[test]
fn panel_opens_from_its_trigger_and_turns_the_pointer_off() {
    let mut s = sim(PANEL);
    open_panel(&mut s);
    assert!(s.e.panel_open());
    assert_eq!(s.out.panel, [PanelEvent::Open]);
    assert_eq!(s.out.pointer, [false]);
    assert!(s.out.fired[0].local);
}

#[test]
fn panel_columns_move_their_slider() {
    let mut s = sim(PANEL);
    open_panel(&mut s);
    // 4 columns of 32 mm: x = 40 mm is the second one (volume); up 20 mm
    s.touch(0.5, 0, 1240, 1500);
    assert!(s.out.panel.contains(&PanelEvent::Focus(Some(1))));
    s.drag(0.5, 0, (1240, 1500), (0, -31), 20);
    let v = adjusted(&s, "volume");
    // the first 2 mm are the dead zone, then it follows the finger
    let per_mm = 100.0 * super::PANEL_GAIN / (2242.0 / 31.0);
    let want = (20.0 - super::PANEL_DEADZONE_MM) * per_mm;
    assert!((v - want).abs() < 1.0, "{v} vs {want}");
    assert_eq!(adjusted(&s, "brightness"), 0.0);
    // moving back down lowers it again, at once (no second dead zone)
    s.drag(0.7, 0, (1240, 880), (0, 31), 10);
    assert!((adjusted(&s, "volume") - (want - 10.0 * per_mm)).abs() < 1.0);
    s.lift(0.9, 0);
    assert_eq!(s.out.panel.last(), Some(&PanelEvent::Focus(None)));
    assert!(s.out.fired.iter().all(|f| f.action == "kaget.panel"), "no gestures in panel mode");
}

#[test]
fn panel_ignores_a_trembling_finger() {
    let mut s = sim(PANEL);
    open_panel(&mut s);
    s.touch(0.5, 0, 1240, 1500);
    // ±1.3 mm wobble around the touch point, a fraction of a mm sideways
    for i in 0..60 {
        let dy = [0, 25, -40, 15, 40, -25][i % 6];
        s.mv(0.5 + i as f64 * 0.008, 0, 1240 + (i % 3) as i32 * 5, 1500 + dy);
    }
    assert_eq!(adjusted(&s, "volume"), 0.0);
    // once really moving, sub-step tremble is still dropped: 0.3 mm back and forth
    s.drag(1.0, 0, (1240, 1500), (0, -31), 6);
    let before = s.out.panel.len();
    for i in 0..30 {
        s.mv(1.1 + i as f64 * 0.008, 0, 1240, 1314 + if i % 2 == 0 { 9 } else { -9 });
    }
    assert_eq!(s.out.panel.len(), before, "no events for tremble below a step");
}

#[test]
fn panel_closes_after_idle_time_but_not_under_a_finger() {
    let mut s = sim(PANEL);
    open_panel(&mut s);
    s.touch(1.0, 0, 200, 1500);
    s.tick(5.0);
    assert!(s.e.panel_open(), "finger still on a column");
    s.lift(5.0, 0);
    s.tick(7.5);
    assert!(s.e.panel_open());
    s.tick(8.1);
    assert!(!s.e.panel_open());
    assert_eq!(s.out.panel.iter().filter(|e| **e == PanelEvent::Close).count(), 1);
    assert_eq!(s.out.pointer, [false, true]);
}

#[test]
fn panel_trigger_hold_again_closes_it() {
    let mut s = sim(PANEL);
    open_panel(&mut s);
    s.touch(1.0, 0, 3900, 100);
    s.hold(1.0, 1.4, 0, 3900, 100);
    assert!(!s.e.panel_open());
    assert_eq!(s.out.pointer, [false, true]);
}

#[test]
fn panel_drag_from_a_corner_moves_the_slider_not_the_corner_gesture() {
    let mut s = sim(PANEL);
    open_panel(&mut s);
    s.touch(1.0, 0, 3900, 100); // top right corner = scale column
    s.drag(1.0, 0, (3900, 100), (0, 40), 20);
    s.lift(1.2, 0);
    assert!(adjusted(&s, "scale") < -10.0, "scale went down with the finger");
    assert!(!s.out.fired.iter().any(|f| f.action == "volume.down"));
    assert!(s.out.panel.contains(&PanelEvent::Commit));
    assert!(s.e.panel_open());
}

#[test]
fn panel_keeps_pointer_off_while_numberpad_is_on() {
    let mut s = sim_with(config(&[("corner.top_left.hold", "kaget.panel"), ("corner.top_right.hold", "numberpad.toggle")]), true);
    s.e.set_numlock(true, &mut s.out);
    assert_eq!(s.out.pointer, [false]);
    s.touch(0.0, 0, 100, 100);
    s.hold(0.0, 0.4, 0, 100, 100);
    s.lift(0.4, 0);
    s.tick(4.0); // panel closed by idle time
    assert!(!s.e.panel_open());
    assert_eq!(s.out.pointer, [false], "NumberPad still needs the pointer off");
}

#[test]
fn panel_in_filter_mode_withholds_instead_of_switching_the_pointer() {
    let mut s = filter_sim(PANEL, false, false);
    open_panel(&mut s);
    s.touch(1.0, 0, 1240, 1500);
    s.drag(1.0, 0, (1240, 1500), (0, -31), 10);
    assert!(s.out.pointer.is_empty());
    assert!(s.clone_sink().frames.is_empty(), "panel touches never reach the desktop");
    assert!(adjusted(&s, "volume") > 0.0);
}

#[test]
fn panel_button_column_runs_its_action_on_a_tap() {
    let mut c = config(PANEL);
    c.panel.controls = vec!["brightness".into(), "action:media.play_pause".into()];
    let mut s = sim_with(c, false);
    open_panel(&mut s);
    // two columns: the right half is the button
    s.touch(1.0, 0, 3000, 1500);
    assert!(s.out.panel.contains(&PanelEvent::Focus(Some(1))));
    s.lift(1.1, 0);
    assert_eq!(s.out.actions(), ["kaget.panel", "media.play_pause"]);
    assert_eq!(s.out.fired[1].trigger, "panel");
    // dragging or holding it does nothing, and it never moves a slider
    s.touch(2.0, 0, 3000, 1500);
    s.drag(2.0, 0, (3000, 1500), (0, -31), 20);
    s.lift(2.2, 0);
    s.touch(3.0, 0, 3000, 1500);
    s.hold(3.0, 3.8, 0, 3000, 1500);
    s.lift(3.8, 0);
    assert_eq!(s.out.fired.len(), 2);
    assert!(!s.out.panel.iter().any(|e| matches!(e, PanelEvent::Adjust { .. })));
}

#[test]
fn panel_slots_follow_the_configured_order() {
    let mut c = config(PANEL);
    c.panel.controls = vec!["volume".into(), "scale".into(), "brightness".into()];
    let mut s = sim_with(c, false);
    open_panel(&mut s);
    // brightness is now the rightmost of three columns
    s.touch(1.0, 0, 3600, 1500);
    s.drag(1.0, 0, (3600, 1500), (0, -31), 10);
    s.lift(1.1, 0);
    assert!(adjusted(&s, "brightness") > 0.0);
    assert_eq!(adjusted(&s, "volume"), 0.0);
}
