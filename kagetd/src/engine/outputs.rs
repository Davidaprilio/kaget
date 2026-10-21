//! Everything the engine does to the outside world goes through `Outputs`, so tests can record it.

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
}

/// Records outputs for tests and `--replay`.
#[derive(Default, Debug)]
pub struct FakeOutputs {
    pub fired: Vec<Fired>,
    pub keys: Vec<(u16, bool)>,
    pub leds: Vec<u8>,
    pub pointer: Vec<bool>,
    pub replays: Vec<Vec<i32>>,
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
}
