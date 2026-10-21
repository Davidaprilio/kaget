//! Contact bookkeeping per MT slot (protocol B).

use std::collections::BTreeMap;

use crate::sys::{self, InputEvent};

#[derive(Clone, Debug)]
pub struct Contact {
    pub slot: usize,
    pub tid: i32,
    /// Device units.
    pub x: i32,
    pub y: i32,
    pub x0: i32,
    pub y0: i32,
    pub t0: f64,
    pub tool: i32,
    /// Lifted in this frame; removed by `remove_lifted` after the frame is processed.
    pub lifted: bool,
    /// The start position was recorded in this frame.
    pub fresh: bool,
    /// X and Y are known (the contact takes part in gestures only then).
    pub ready: bool,
}

/// Last axis values per slot. evdev drops repeated values, so a new contact in a slot may only
/// report the axis that changed.
#[derive(Clone, Copy, Default, Debug)]
struct SlotAxes {
    x: Option<i32>,
    y: Option<i32>,
    tool: i32,
}

#[derive(Default, Debug)]
pub struct Tracker {
    slot: usize,
    axes: BTreeMap<usize, SlotAxes>,
    contacts: BTreeMap<usize, Contact>,
}

impl Tracker {
    /// Apply the events of one frame (everything before SYN_REPORT).
    pub fn apply(&mut self, events: &[InputEvent], now: f64) {
        for c in self.contacts.values_mut() {
            c.fresh = false;
        }
        for e in events {
            if e.type_ != sys::EV_ABS {
                continue;
            }
            match e.code {
                sys::ABS_MT_SLOT => self.slot = e.value.max(0) as usize,
                sys::ABS_MT_TRACKING_ID => {
                    if e.value < 0 {
                        if let Some(c) = self.contacts.get_mut(&self.slot) {
                            c.lifted = true;
                        }
                    } else {
                        let a = self.axes.get(&self.slot).copied().unwrap_or_default();
                        self.contacts.insert(
                            self.slot,
                            Contact {
                                slot: self.slot,
                                tid: e.value,
                                x: a.x.unwrap_or(0),
                                y: a.y.unwrap_or(0),
                                x0: 0,
                                y0: 0,
                                t0: now,
                                tool: a.tool,
                                lifted: false,
                                fresh: false,
                                ready: false,
                            },
                        );
                    }
                }
                sys::ABS_MT_POSITION_X | sys::ABS_MT_POSITION_Y | sys::ABS_MT_TOOL_TYPE => {
                    let a = self.axes.entry(self.slot).or_default();
                    match e.code {
                        sys::ABS_MT_POSITION_X => a.x = Some(e.value),
                        sys::ABS_MT_POSITION_Y => a.y = Some(e.value),
                        _ => a.tool = e.value,
                    }
                    if let Some(c) = self.contacts.get_mut(&self.slot) {
                        match e.code {
                            sys::ABS_MT_POSITION_X => c.x = e.value,
                            sys::ABS_MT_POSITION_Y => c.y = e.value,
                            _ => c.tool = e.value,
                        }
                    }
                }
                _ => {}
            }
        }
        for (slot, c) in self.contacts.iter_mut() {
            if !c.ready && !c.lifted {
                let a = self.axes.get(slot).copied().unwrap_or_default();
                if a.x.is_some() && a.y.is_some() {
                    c.ready = true;
                    c.fresh = true;
                    c.x0 = c.x;
                    c.y0 = c.y;
                }
            }
        }
    }

    pub fn remove_lifted(&mut self) {
        self.contacts.retain(|_, c| !c.lifted);
    }

    /// Forget all contacts (after SYN_DROPPED or a reconnect).
    pub fn reset(&mut self) {
        self.contacts.clear();
        self.axes.clear();
        self.slot = 0;
    }

    pub fn contacts(&self) -> impl Iterator<Item = &Contact> {
        self.contacts.values().filter(|c| c.ready)
    }

    pub fn get(&self, slot: usize) -> Option<&Contact> {
        self.contacts.get(&slot).filter(|c| c.ready)
    }

    /// Ready contacts that are still down.
    pub fn live(&self) -> usize {
        self.contacts().filter(|c| !c.lifted).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::event;

    fn abs(code: u16, v: i32) -> InputEvent {
        event(sys::EV_ABS, code, v)
    }

    #[test]
    fn new_contact_inherits_unchanged_axis() {
        let mut t = Tracker::default();
        t.apply(&[abs(sys::ABS_MT_SLOT, 0), abs(sys::ABS_MT_TRACKING_ID, 1),
                  abs(sys::ABS_MT_POSITION_X, 100), abs(sys::ABS_MT_POSITION_Y, 200)], 0.0);
        assert!(t.get(0).unwrap().fresh);
        t.apply(&[abs(sys::ABS_MT_TRACKING_ID, -1)], 0.1);
        assert!(t.get(0).unwrap().lifted);
        t.remove_lifted();
        assert_eq!(t.live(), 0);
        // same Y as before: the kernel sends only X
        t.apply(&[abs(sys::ABS_MT_TRACKING_ID, 2), abs(sys::ABS_MT_POSITION_X, 300)], 0.2);
        let c = t.get(0).unwrap();
        assert_eq!((c.x0, c.y0, c.tid), (300, 200, 2));
    }

    #[test]
    fn waits_for_both_axes() {
        let mut t = Tracker::default();
        t.apply(&[abs(sys::ABS_MT_SLOT, 1), abs(sys::ABS_MT_TRACKING_ID, 5), abs(sys::ABS_MT_POSITION_X, 10)], 0.0);
        assert!(t.get(1).is_none());
        t.apply(&[abs(sys::ABS_MT_POSITION_Y, 20)], 0.01);
        let c = t.get(1).unwrap();
        assert!(c.fresh && c.t0 == 0.0 && (c.x0, c.y0) == (10, 20));
    }
}
