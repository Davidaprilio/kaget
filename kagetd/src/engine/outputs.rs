//! Everything the engine does to the outside world goes through `Outputs`, so tests can record it.

use serde::Serialize;

/// A binding that ran.
#[derive(Clone, Debug, PartialEq)]
pub struct Fired {
    pub trigger: String,
    /// The action string that ran (the inverse when a slider moves back).
    pub action: String,
    pub when_running: Option<String>,
    /// Already handled inside the daemon (NumberPad, pause); only reported.
    pub local: bool,
}

/// Contacts handed to the desktop and the offsets (device units, from their centroid) of the
/// synthesized fingers that follow them (`Outputs::replay_as`).
pub type Handover = (Vec<i32>, Vec<(i32, i32)>);

/// What became of one touch sequence (a corner touch or a gesture of 3+ fingers), for the Test
/// page (`Diagnosis` signal).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Diagnosis {
    /// `multi` (3+ fingers) or `corner`.
    pub kind: &'static str,
    pub fingers: u32,
    /// Corner name (`corner`).
    pub corner: &'static str,
    /// What it was recognised as; empty if nothing.
    pub trigger: String,
    /// `fired`, `slider`, `kde` (left to the desktop), `swallowed` or `none`.
    pub result: &'static str,
    pub action: String,
    /// Why, for `kde` and `none`: `unbound`, `handover`, `count` (nothing bound for that many
    /// fingers), `late_finger`, `not_together`, `busy` (a slider was running), `short` (did not
    /// move far enough), `slow_tap`, `short_hold`, `tap` (a click), `early` (lifted before the
    /// hold time), `off_pad` (corner swipe off the touchpad), `drifted` (moved during a hold);
    /// `continued` for a slider continued in its band.
    pub reason: &'static str,
    /// Measurements, 0 when they do not apply: how far apart the fingers landed, how late a
    /// finger came, travel (of the centroid for 3+ fingers), touch duration, the missed
    /// threshold (`need_*`), slider steps.
    pub span_ms: f64,
    pub late_ms: f64,
    pub travel_mm: f64,
    pub need_mm: f64,
    pub duration_ms: f64,
    pub need_ms: f64,
    pub steps: u32,
}

/// Touchpad panel mode (see `Engine::set_panel`).
#[derive(Clone, Debug, PartialEq)]
pub enum PanelEvent {
    Open,
    Close,
    /// A finger moved in a slider column; `delta` is in percent of that control's range
    /// (for `scale`: percent of the 50–200 % range).
    Adjust { kind: String, delta: f64 },
    /// Column under the finger (for highlighting), `None` when it lifts.
    Focus(Option<usize>),
    /// The finger on the `scale` column lifted: save the size.
    Commit,
}

pub trait Outputs {
    fn fired(&mut self, f: &Fired);
    /// Press (`down`) or release key codes on the virtual keyboard.
    fn key(&mut self, codes: &[u16], down: bool);
    /// NumberPad LED feature report value.
    fn led(&mut self, value: u8);
    /// Switch the touchpad pointer on/off in the compositor.
    fn pointer(&mut self, enabled: bool);
    /// Something visible through GetState changed (NumberPad, pause).
    fn state_changed(&mut self);
    /// Filter mode: hand withheld contacts (by tracking id) to the desktop after all, replaying
    /// their frames (a short corner tap, a 3-finger swipe that turned out to be KDE's).
    fn replay(&mut self, _tids: &[i32]) {}
    /// Like `replay`, plus synthesized fingers at these offsets (device units) from the centroid of
    /// `tids` that follow them, so the desktop sees a gesture with more fingers (`system:<n>`).
    fn replay_as(&mut self, tids: &[i32], _extra: &[(i32, i32)]) {
        self.replay(tids);
    }
    fn panel(&mut self, _ev: PanelEvent) {}
    /// `system:<n>:window`: KWin's desktop switch is starting (`true`; keep the active window on
    /// screen) or its fingers lifted (`false`; leave the window on the desktop it ends up on).
    fn carry_window(&mut self, _start: bool) {}
    fn diagnosis(&mut self, _d: &Diagnosis) {}
}

/// Records outputs for tests and `--replay`.
#[derive(Default, Debug)]
pub struct FakeOutputs {
    pub fired: Vec<Fired>,
    pub keys: Vec<(u16, bool)>,
    pub leds: Vec<u8>,
    pub pointer: Vec<bool>,
    pub replays: Vec<Vec<i32>>,
    /// `replay_as`: the contacts and the synthesized fingers that follow them.
    pub extra: Vec<Handover>,
    pub panel: Vec<PanelEvent>,
    pub carry: Vec<bool>,
    pub diags: Vec<Diagnosis>,
}

#[cfg(test)]
impl FakeOutputs {
    pub fn actions(&self) -> Vec<&str> {
        self.fired.iter().map(|f| f.action.as_str()).collect()
    }
    pub fn count(&self, action: &str) -> usize {
        self.fired.iter().filter(|f| f.action == action).count()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

impl Outputs for FakeOutputs {
    fn fired(&mut self, f: &Fired) {
        self.fired.push(f.clone());
    }
    fn key(&mut self, codes: &[u16], down: bool) {
        self.keys.extend(codes.iter().map(|&c| (c, down)));
    }
    fn led(&mut self, value: u8) {
        self.leds.push(value);
    }
    fn pointer(&mut self, enabled: bool) {
        self.pointer.push(enabled);
    }
    fn state_changed(&mut self) {}
    fn replay(&mut self, tids: &[i32]) {
        self.replays.push(tids.to_vec());
    }
    fn replay_as(&mut self, tids: &[i32], extra: &[(i32, i32)]) {
        self.replays.push(tids.to_vec());
        self.extra.push((tids.to_vec(), extra.to_vec()));
    }
    fn panel(&mut self, ev: PanelEvent) {
        self.panel.push(ev);
    }
    fn carry_window(&mut self, start: bool) {
        self.carry.push(start);
    }
    fn diagnosis(&mut self, d: &Diagnosis) {
        self.diags.push(d.clone());
    }
}
