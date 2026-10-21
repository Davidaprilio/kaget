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
fn rotate_three_clockwise() {
    let mut s = sim(&[("rotate.3.cw", "cmd:cw"), ("rotate.3.ccw", "cmd:ccw")]);
    let c = (2000.0, 1100.0);
    let r = 400.0;
    let fingers: Vec<(i32, f64)> = vec![(0, 0.0), (1, 120.0), (2, 240.0)];
    let at = |a: f64| ((c.0 + r * a.to_radians().cos()) as i32, (c.1 + r * a.to_radians().sin()) as i32);
    for (i, &(slot, a)) in fingers.iter().enumerate() {
        let (x, y) = at(a);
        s.touch(i as f64 * 0.01, slot, x, y);
    }
    for i in 1..=20 {
        let mut evs = Vec::new();
        for &(slot, a) in &fingers {
            let (x, y) = at(a + 2.0 * i as f64); // +y is down: increasing angle is clockwise
            evs.extend([abs(sys::ABS_MT_SLOT, slot), abs(sys::ABS_MT_POSITION_X, x), abs(sys::ABS_MT_POSITION_Y, y)]);
        }
        s.frame(&evs, 0.05 + i as f64 * 0.008);
    }
    assert_eq!(s.out.actions(), ["cmd:cw"]);
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
    let mut slot_tid = std::collections::BTreeMap::new();
    let mut slot = 0;
    let mut xs = Vec::new();
    for e in s.clone_sink().events().filter(|e| e.type_ == sys::EV_ABS) {
        match e.code {
            sys::ABS_MT_SLOT => slot = e.value,
            sys::ABS_MT_TRACKING_ID => {
                slot_tid.insert(slot, e.value);
            }
            sys::ABS_MT_POSITION_X if slot_tid.get(&slot) == Some(&tid) => xs.push(e.value),
            _ => {}
        }
    }
    xs
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
