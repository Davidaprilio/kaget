//! Filter mode: the real touchpad is grabbed and re-emitted through a uinput clone. Contacts the
//! engine marks as hidden are withheld from the clone; when they turn out to belong to the desktop
//! after all (a short corner tap, a 3-finger swipe bound to `system`), their frames are replayed.
//!
//! Contacts are keyed by tracking id, and clone slots are allocated independently of the real
//! slots, so a withheld touch can be replayed after its real slot was reused.
//! A contact the desktop already saw and that becomes hidden (the first two fingers of a 3-finger
//! session) is *frozen*: it stays down in the clone without moving. Its lift is delayed until it has
//! been down for `TAP_SAFE_S`, so libinput never turns it into a 2-finger tap.

use std::collections::{BTreeMap, BTreeSet};

use crate::sys::{self, InputEvent};

/// libinput's tap timeout is 180 ms; a contact that stays down longer is never a tap.
const TAP_SAFE_S: f64 = 0.25;
/// Withheld contacts that lifted are kept this long for a replay (double tap wait).
const KEEP_LIFTED_S: f64 = 2.0;
/// Replays are paced and squeezed into at most this many frames.
const MAX_REPLAY_FRAMES: usize = 20;

/// Where the clone's frames go (the uinput device, or a recorder in tests).
pub trait Sink {
    /// One complete frame, SYN_REPORT included.
    fn write(&mut self, frame: &[InputEvent]);
    /// Called between replayed frames so they get distinct timestamps.
    fn pace(&mut self) {}
}

#[derive(Debug)]
struct Contact {
    /// Latest MT axis values (everything but slot and tracking id).
    axes: BTreeMap<u16, i32>,
    /// State to announce when the clone gets this contact late (hidden from the start, or
    /// lifted from the clone while frozen).
    start_axes: BTreeMap<u16, i32>,
    clone_slot: Option<usize>,
    /// Forwarded live.
    visible: bool,
    /// When it went down in the clone.
    t_clone: f64,
    /// Lifted on the real device (kept while withheld, for a replay).
    lifted: Option<f64>,
}

#[derive(Debug, Default)]
struct HistFrame {
    updates: Vec<(i32, u16, i32)>,
    lifts: Vec<i32>,
}

pub struct Filter<S: Sink> {
    sink: S,
    /// Keys the clone declares (only those BTN_TOOL_* are synthesized).
    keys: BTreeSet<u16>,
    real_slot: usize,
    slot_tid: BTreeMap<usize, i32>,
    slot_axes: BTreeMap<usize, BTreeMap<u16, i32>>,
    contacts: BTreeMap<i32, Contact>,
    clone_slots: Vec<Option<i32>>,
    clone_pos: Vec<(i32, i32)>,
    clone_cur: i32,
    history: Vec<HistFrame>,
    tool: BTreeMap<u16, i32>,
    abs_xy: Option<(i32, i32)>,
    suppressed: BTreeSet<u16>,
    deferred: Vec<(i32, f64)>,
    out: Vec<InputEvent>,
}

const TOOL_KEYS: [u16; 6] = [
    sys::BTN_TOUCH,
    sys::BTN_TOOL_FINGER,
    sys::BTN_TOOL_DOUBLETAP,
    sys::BTN_TOOL_TRIPLETAP,
    sys::BTN_TOOL_QUADTAP,
    sys::BTN_TOOL_QUINTTAP,
];

fn is_mt_axis(code: u16) -> bool {
    code > sys::ABS_MT_SLOT && code <= sys::ABS_MT_LAST && code != sys::ABS_MT_TRACKING_ID
}

impl<S: Sink> Filter<S> {
    pub fn new(sink: S, slots: usize, keys: &[u16]) -> Self {
        let n = slots.max(1);
        Self {
            sink,
            keys: keys.iter().copied().collect(),
            real_slot: 0,
            slot_tid: BTreeMap::new(),
            slot_axes: BTreeMap::new(),
            contacts: BTreeMap::new(),
            clone_slots: vec![None; n],
            clone_pos: vec![(0, 0); n],
            clone_cur: -1,
            history: Vec::new(),
            tool: BTreeMap::new(),
            abs_xy: None,
            suppressed: BTreeSet::new(),
            deferred: Vec::new(),
            out: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn sink(&self) -> &S {
        &self.sink
    }

    // ---- emission -------------------------------------------------------------------------------

    fn push(&mut self, type_: u16, code: u16, value: i32) {
        self.out.push(sys::event(type_, code, value));
    }

    fn select(&mut self, cs: usize) {
        if self.clone_cur != cs as i32 {
            self.push(sys::EV_ABS, sys::ABS_MT_SLOT, cs as i32);
            self.clone_cur = cs as i32;
        }
    }

    fn emit_axis(&mut self, cs: usize, code: u16, value: i32) {
        self.select(cs);
        self.push(sys::EV_ABS, code, value);
        match code {
            sys::ABS_MT_POSITION_X => self.clone_pos[cs].0 = value,
            sys::ABS_MT_POSITION_Y => self.clone_pos[cs].1 = value,
            _ => {}
        }
    }

    /// Put a contact down in a free clone slot with the given axes.
    fn announce(&mut self, tid: i32, axes: &BTreeMap<u16, i32>, now: f64) -> Option<usize> {
        let cs = self.clone_slots.iter().position(Option::is_none)?;
        self.clone_slots[cs] = Some(tid);
        self.select(cs);
        self.push(sys::EV_ABS, sys::ABS_MT_TRACKING_ID, tid);
        for (&code, &v) in axes {
            self.emit_axis(cs, code, v);
        }
        if let Some(c) = self.contacts.get_mut(&tid) {
            c.clone_slot = Some(cs);
            c.t_clone = now;
        }
        Some(cs)
    }

    fn lift_clone(&mut self, tid: i32) {
        let Some(cs) = self.contacts.get_mut(&tid).and_then(|c| c.clone_slot.take()) else { return };
        self.select(cs);
        self.push(sys::EV_ABS, sys::ABS_MT_TRACKING_ID, -1);
        self.clone_slots[cs] = None;
    }

    /// BTN_TOUCH / BTN_TOOL_* and ABS_X/Y follow the contacts that are down in the clone.
    fn aggregate(&mut self) {
        let down: Vec<usize> = (0..self.clone_slots.len()).filter(|&i| self.clone_slots[i].is_some()).collect();
        let n = down.len();
        let want = [n > 0, n == 1, n == 2, n == 3, n == 4, n >= 5];
        for (code, on) in TOOL_KEYS.into_iter().zip(want) {
            let v = on as i32;
            if self.keys.contains(&code) && self.tool.get(&code).copied().unwrap_or(0) != v {
                self.push(sys::EV_KEY, code, v);
                self.tool.insert(code, v);
            }
        }
        if let Some(&cs) = down.first() {
            let p = self.clone_pos[cs];
            if self.abs_xy != Some(p) {
                self.push(sys::EV_ABS, sys::ABS_X, p.0);
                self.push(sys::EV_ABS, sys::ABS_Y, p.1);
                self.abs_xy = Some(p);
            }
        }
    }

    fn flush(&mut self) {
        self.aggregate();
        if self.out.is_empty() {
            return;
        }
        self.push(sys::EV_SYN, sys::SYN_REPORT, 0);
        let frame = std::mem::take(&mut self.out);
        self.sink.write(&frame);
    }

    // ---- replay ---------------------------------------------------------------------------------

    /// Hand withheld contacts to the clone: announce those it does not have, replay their history.
    fn reveal(&mut self, tids: &[i32], now: f64) {
        let set: BTreeSet<i32> =
            tids.iter().copied().filter(|t| self.contacts.get(t).is_some_and(|c| !c.visible)).collect();
        if set.is_empty() {
            return;
        }
        self.flush();
        for &tid in &set {
            if self.contacts[&tid].clone_slot.is_none() {
                let axes = self.contacts[&tid].start_axes.clone();
                self.announce(tid, &axes, now);
            }
            self.deferred.retain(|(t, _)| *t != tid);
        }
        let frames: Vec<&HistFrame> =
            self.history.iter().filter(|f| f.updates.iter().any(|u| set.contains(&u.0)) || f.lifts.iter().any(|t| set.contains(t))).collect();
        // squeeze into chunks; later absolute values win inside a chunk
        let chunk = frames.len().div_ceil(MAX_REPLAY_FRAMES).max(1);
        let chunks: Vec<HistFrame> = frames
            .chunks(chunk)
            .map(|c| {
                let mut merged: BTreeMap<(i32, u16), i32> = BTreeMap::new();
                let mut lifts = Vec::new();
                for f in c {
                    for &(t, code, v) in f.updates.iter().filter(|u| set.contains(&u.0)) {
                        merged.insert((t, code), v);
                    }
                    lifts.extend(f.lifts.iter().copied().filter(|t| set.contains(t)));
                }
                HistFrame { updates: merged.into_iter().map(|((t, c), v)| (t, c, v)).collect(), lifts }
            })
            .collect();
        for HistFrame { updates, lifts } in chunks {
            self.flush();
            self.sink.pace();
            for (tid, code, v) in updates {
                if let Some(cs) = self.contacts.get(&tid).and_then(|c| c.clone_slot) {
                    self.emit_axis(cs, code, v);
                }
            }
            for tid in lifts {
                self.lift_clone(tid);
            }
        }
        self.flush();
        for &tid in &set {
            let c = self.contacts.get_mut(&tid).expect("revealed contact");
            if c.lifted.is_some() {
                self.contacts.remove(&tid);
            } else {
                c.visible = true;
            }
        }
        self.prune_history();
    }

    fn prune_history(&mut self) {
        let keep: BTreeSet<i32> = self.contacts.iter().filter(|(_, c)| !c.visible).map(|(t, _)| *t).collect();
        if keep.is_empty() {
            self.history.clear();
            return;
        }
        for f in &mut self.history {
            f.updates.retain(|u| keep.contains(&u.0));
            f.lifts.retain(|t| keep.contains(t));
        }
        self.history.retain(|f| !f.updates.is_empty() || !f.lifts.is_empty());
    }

    // ---- frames ---------------------------------------------------------------------------------

    /// Process one real frame (or none, for timers). `hidden(real_slot)` is the engine's decision
    /// for live contacts, `replays` the tracking ids the engine handed back to the desktop.
    pub fn process(&mut self, events: &[InputEvent], now: f64, hidden: &dyn Fn(usize) -> bool, replays: &[i32]) {
        // 1. parse
        let mut updates: Vec<(i32, u16, i32)> = Vec::new();
        let mut lifts: Vec<i32> = Vec::new();
        let mut fresh: Vec<i32> = Vec::new();
        let mut buttons: Vec<(u16, i32)> = Vec::new();
        let mut passthrough: Vec<InputEvent> = Vec::new();
        for e in events {
            match (e.type_, e.code) {
                (sys::EV_ABS, sys::ABS_MT_SLOT) => self.real_slot = e.value.max(0) as usize,
                (sys::EV_ABS, sys::ABS_MT_TRACKING_ID) => {
                    if let Some(old) = self.slot_tid.remove(&self.real_slot) {
                        lifts.push(old);
                    }
                    if e.value >= 0 {
                        let axes = self.slot_axes.get(&self.real_slot).cloned().unwrap_or_default();
                        self.slot_tid.insert(self.real_slot, e.value);
                        self.contacts.insert(
                            e.value,
                            Contact { start_axes: axes.clone(), axes, clone_slot: None, visible: false, t_clone: now, lifted: None },
                        );
                        fresh.push(e.value);
                    }
                }
                (sys::EV_ABS, code) if is_mt_axis(code) => {
                    self.slot_axes.entry(self.real_slot).or_default().insert(code, e.value);
                    if let Some(&tid) = self.slot_tid.get(&self.real_slot) {
                        if let Some(c) = self.contacts.get_mut(&tid) {
                            c.axes.insert(code, e.value);
                        }
                        updates.push((tid, code, e.value));
                    }
                }
                (sys::EV_ABS, sys::ABS_X | sys::ABS_Y) => {}
                (sys::EV_KEY, code) if TOOL_KEYS.contains(&code) => {}
                (sys::EV_KEY, code) => buttons.push((code, e.value)),
                _ => passthrough.push(*e),
            }
        }
        for tid in &fresh {
            if let Some(c) = self.contacts.get_mut(tid) {
                c.start_axes = c.axes.clone();
            }
        }
        // 2. the engine handed contacts back
        self.reveal(replays, now);
        // 3. visibility of live contacts
        let live: Vec<(usize, i32)> = self.slot_tid.iter().map(|(&s, &t)| (s, t)).collect();
        let mut announced: BTreeSet<i32> = BTreeSet::new();
        for (slot, tid) in live {
            let show = !hidden(slot);
            let Some(c) = self.contacts.get(&tid) else { continue };
            if show && !c.visible {
                if c.clone_slot.is_none() && !self.history.iter().any(|f| f.updates.iter().any(|u| u.0 == tid)) {
                    // new (or no history yet): just put it down
                    let axes = c.axes.clone();
                    self.announce(tid, &axes, now);
                    self.contacts.get_mut(&tid).unwrap().visible = true;
                    announced.insert(tid);
                } else {
                    self.reveal(&[tid], now);
                }
            } else if !show && c.visible {
                // the desktop already has it: freeze it there
                let c = self.contacts.get_mut(&tid).unwrap();
                c.visible = false;
                c.start_axes = c.axes.clone();
            }
        }
        // 4. live updates
        for &(tid, code, v) in &updates {
            if announced.contains(&tid) {
                continue;
            }
            if let Some(cs) = self.contacts.get(&tid).filter(|c| c.visible).and_then(|c| c.clone_slot) {
                self.emit_axis(cs, code, v);
            }
        }
        // 5. lifts
        for &tid in &lifts {
            let Some(c) = self.contacts.get_mut(&tid) else { continue };
            if c.visible {
                self.lift_clone(tid);
                self.contacts.remove(&tid);
            } else {
                c.lifted = Some(now);
                if c.clone_slot.is_some() {
                    self.deferred.push((tid, c.t_clone + TAP_SAFE_S));
                }
            }
        }
        // 6. history of withheld contacts
        let withheld: BTreeSet<i32> = self.contacts.iter().filter(|(_, c)| !c.visible).map(|(t, _)| *t).collect();
        if withheld.is_empty() {
            self.history.clear();
        } else {
            let f = HistFrame {
                updates: updates.iter().copied().filter(|u| withheld.contains(&u.0)).collect(),
                lifts: lifts.iter().copied().filter(|t| withheld.contains(t)).collect(),
            };
            if !f.updates.is_empty() || !f.lifts.is_empty() {
                self.history.push(f);
            }
        }
        // 7. delayed lifts of frozen contacts, expiry of withheld lifted ones
        let due: Vec<i32> = self.deferred.iter().filter(|(_, at)| now >= *at).map(|(t, _)| *t).collect();
        self.deferred.retain(|(_, at)| now < *at);
        for tid in due {
            self.lift_clone(tid);
        }
        let expired: Vec<i32> = self
            .contacts
            .iter()
            .filter(|(_, c)| c.clone_slot.is_none() && c.lifted.is_some_and(|t| now - t > KEEP_LIFTED_S))
            .map(|(t, _)| *t)
            .collect();
        if !expired.is_empty() {
            for t in expired {
                self.contacts.remove(&t);
            }
            self.prune_history();
        }
        // 8. buttons: a click while only withheld fingers are down belongs to them
        let any_visible = self.contacts.values().any(|c| c.visible);
        let any_hidden_live = self.contacts.values().any(|c| !c.visible && c.lifted.is_none());
        for (code, v) in buttons {
            if v != 0 && any_hidden_live && !any_visible {
                self.suppressed.insert(code);
            } else if v == 0 && self.suppressed.remove(&code) {
            } else {
                self.push(sys::EV_KEY, code, v);
            }
        }
        if !self.out.is_empty() || !passthrough.is_empty() && any_visible {
            self.out.extend(passthrough);
        }
        self.flush();
    }

    /// Lift everything in the clone and forget all state (events were dropped, or shutdown).
    pub fn reset(&mut self) {
        let tids: Vec<i32> = self.clone_slots.iter().flatten().copied().collect();
        for t in tids {
            self.lift_clone(t);
        }
        self.clone_slots.iter_mut().for_each(|s| *s = None);
        self.suppressed.clear();
        self.flush();
        self.contacts.clear();
        self.slot_tid.clear();
        self.history.clear();
        self.deferred.clear();
    }
}

impl Sink for crate::uinput::VirtualTouchpad {
    fn write(&mut self, frame: &[InputEvent]) {
        if let Err(e) = self.write_frame(frame) {
            eprintln!("touchpad clone: {e}");
        }
    }
    fn pace(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Records the clone's frames and decodes the resulting contact state.
    #[derive(Default)]
    pub struct FakeSink {
        pub frames: Vec<Vec<InputEvent>>,
    }

    impl Sink for FakeSink {
        fn write(&mut self, frame: &[InputEvent]) {
            assert_eq!(frame.last().map(|e| (e.type_, e.code)), Some((sys::EV_SYN, sys::SYN_REPORT)));
            self.frames.push(frame.to_vec());
        }
    }

    impl FakeSink {
        /// Tracking ids that ever went down in the clone.
        pub fn touched(&self) -> BTreeSet<i32> {
            self.events().filter(|e| e.code == sys::ABS_MT_TRACKING_ID && e.value >= 0).map(|e| e.value).collect()
        }
        /// Tracking ids down in the clone after all frames.
        pub fn down(&self) -> BTreeSet<i32> {
            let mut slot = 0;
            let mut slots: BTreeMap<i32, i32> = BTreeMap::new();
            for e in self.events() {
                match e.code {
                    sys::ABS_MT_SLOT if e.type_ == sys::EV_ABS => slot = e.value,
                    sys::ABS_MT_TRACKING_ID if e.type_ == sys::EV_ABS => {
                        if e.value >= 0 {
                            slots.insert(slot, e.value);
                        } else {
                            slots.remove(&slot);
                        }
                    }
                    _ => {}
                }
            }
            slots.values().copied().collect()
        }
        pub fn events(&self) -> impl Iterator<Item = &InputEvent> {
            self.frames.iter().flatten()
        }
        pub fn key(&self, code: u16) -> Vec<i32> {
            self.events().filter(|e| e.type_ == sys::EV_KEY && e.code == code).map(|e| e.value).collect()
        }
    }

    fn abs(code: u16, v: i32) -> InputEvent {
        sys::event(sys::EV_ABS, code, v)
    }

    fn keys() -> Vec<u16> {
        TOOL_KEYS.to_vec()
    }

    fn down(slot: i32, tid: i32, x: i32, y: i32) -> Vec<InputEvent> {
        vec![abs(sys::ABS_MT_SLOT, slot), abs(sys::ABS_MT_TRACKING_ID, tid), abs(sys::ABS_MT_POSITION_X, x), abs(sys::ABS_MT_POSITION_Y, y)]
    }

    fn up(slot: i32) -> Vec<InputEvent> {
        vec![abs(sys::ABS_MT_SLOT, slot), abs(sys::ABS_MT_TRACKING_ID, -1)]
    }

    fn mv(slot: i32, x: i32) -> Vec<InputEvent> {
        vec![abs(sys::ABS_MT_SLOT, slot), abs(sys::ABS_MT_POSITION_X, x)]
    }

    #[test]
    fn visible_contacts_pass_through_with_tool_keys() {
        let mut f = Filter::new(FakeSink::default(), 5, &keys());
        let no = |_: usize| false;
        f.process(&down(0, 1, 100, 200), 0.0, &no, &[]);
        f.process(&mv(0, 150), 0.01, &no, &[]);
        f.process(&up(0), 0.02, &no, &[]);
        let s = f.sink();
        assert_eq!(s.touched(), [1].into());
        assert!(s.down().is_empty());
        assert_eq!(s.key(sys::BTN_TOOL_FINGER), [1, 0]);
        assert!(s.events().any(|e| e.code == sys::ABS_MT_POSITION_X && e.value == 150));
    }

    #[test]
    fn hidden_contact_never_reaches_clone_unless_replayed() {
        let mut f = Filter::new(FakeSink::default(), 5, &keys());
        let yes = |_: usize| true;
        f.process(&down(0, 1, 100, 200), 0.0, &yes, &[]);
        f.process(&up(0), 0.1, &yes, &[]);
        assert!(f.sink().frames.is_empty());
        // replayed as a tap
        f.process(&[], 0.12, &yes, &[1]);
        assert_eq!(f.sink().touched(), [1].into());
        assert!(f.sink().down().is_empty());
        assert_eq!(f.sink().key(sys::BTN_TOUCH), [1, 0]);
    }

    #[test]
    fn revealed_contact_replays_its_movement() {
        let mut f = Filter::new(FakeSink::default(), 5, &keys());
        let hide = std::cell::Cell::new(true);
        let h = |_: usize| hide.get();
        f.process(&down(0, 1, 100, 200), 0.0, &h, &[]);
        for i in 1..=5 {
            f.process(&mv(0, 100 + i * 50), i as f64 * 0.01, &h, &[]);
        }
        assert!(f.sink().frames.is_empty());
        hide.set(false);
        f.process(&mv(0, 400), 0.06, &h, &[]);
        let xs: Vec<i32> = f.sink().events().filter(|e| e.code == sys::ABS_MT_POSITION_X).map(|e| e.value).collect();
        assert_eq!(xs.first(), Some(&100));
        assert_eq!(xs.last(), Some(&400));
        assert!(xs.contains(&350));
        assert_eq!(f.sink().down(), [1].into());
    }

    #[test]
    fn frozen_contact_lift_waits_for_tap_timeout() {
        let mut f = Filter::new(FakeSink::default(), 5, &keys());
        let hide = std::cell::Cell::new(false);
        let h = |_: usize| hide.get();
        f.process(&down(0, 1, 100, 200), 0.0, &h, &[]);
        hide.set(true);
        f.process(&mv(0, 300), 0.02, &h, &[]);
        f.process(&up(0), 0.1, &h, &[]);
        assert_eq!(f.sink().down(), [1].into(), "frozen, still down");
        assert!(!f.sink().events().any(|e| e.code == sys::ABS_MT_POSITION_X && e.value == 300));
        f.process(&[], 0.2, &h, &[]);
        assert_eq!(f.sink().down(), [1].into());
        f.process(&[], 0.26, &h, &[]);
        assert!(f.sink().down().is_empty());
    }

    #[test]
    fn real_slot_reuse_does_not_confuse_replay() {
        let mut f = Filter::new(FakeSink::default(), 5, &keys());
        let yes = |_: usize| true;
        f.process(&down(0, 1, 100, 200), 0.0, &yes, &[]);
        f.process(&up(0), 0.05, &yes, &[]);
        let no = |_: usize| false;
        f.process(&down(0, 2, 900, 900), 0.1, &no, &[]);
        f.process(&[], 0.15, &no, &[1]);
        let s = f.sink();
        assert_eq!(s.touched(), [1, 2].into());
        assert_eq!(s.down(), [2].into());
    }

    #[test]
    fn click_with_only_withheld_fingers_is_suppressed() {
        let mut f = Filter::new(FakeSink::default(), 5, &keys());
        let yes = |_: usize| true;
        f.process(&down(0, 1, 100, 200), 0.0, &yes, &[]);
        f.process(&[sys::event(sys::EV_KEY, sys::BTN_LEFT, 1)], 0.05, &yes, &[]);
        f.process(&[sys::event(sys::EV_KEY, sys::BTN_LEFT, 0)], 0.1, &yes, &[]);
        assert!(f.sink().key(sys::BTN_LEFT).is_empty());
        let no = |_: usize| false;
        f.process(&down(1, 2, 500, 500), 0.2, &no, &[]);
        f.process(&[sys::event(sys::EV_KEY, sys::BTN_LEFT, 1)], 0.25, &no, &[]);
        assert_eq!(f.sink().key(sys::BTN_LEFT), [1]);
    }

    #[test]
    fn long_replay_is_squeezed_and_ends_in_place() {
        let mut f = Filter::new(FakeSink::default(), 5, &keys());
        let yes = |_: usize| true;
        f.process(&down(0, 1, 0, 0), 0.0, &yes, &[]);
        for i in 1..=100 {
            f.process(&mv(0, i * 10), i as f64 * 0.008, &yes, &[]);
        }
        f.process(&[], 0.9, &yes, &[1]);
        assert!(f.sink().frames.len() <= MAX_REPLAY_FRAMES + 2, "{}", f.sink().frames.len());
        let last_x = f.sink().events().filter(|e| e.code == sys::ABS_MT_POSITION_X).map(|e| e.value).last();
        assert_eq!(last_x, Some(1000));
    }
}
