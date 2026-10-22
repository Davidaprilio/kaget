//! NumberPad runtime: on/off state, LED and pointer switching. Key touches are recognised by the
//! engine (they compete with corner gestures); this module owns everything else.

use crate::actions::NumberPadOp;
use crate::engine::outputs::Outputs;
use crate::numberpad::Profile;

pub const KEY_NUMLOCK: u16 = 69;

pub struct NumberPad {
    pub profile: Profile,
    pub active: bool,
    /// Index into `profile.levels`.
    pub level: usize,
}

impl NumberPad {
    pub fn new(profile: Profile) -> Self {
        Self { profile, active: false, level: 0 }
    }

    /// Bring the hardware into a known state: LED off, pointer on (recovers from a crash).
    pub fn start(&self, out: &mut dyn Outputs) {
        for &v in &self.profile.init {
            out.led(v);
        }
        out.led(self.profile.led_off);
        out.pointer(true);
    }

    pub fn shutdown(&mut self, out: &mut dyn Outputs) {
        if self.active {
            self.active = false;
            out.led(self.profile.led_off);
        }
        out.pointer(true);
    }

    /// Follow the NumLock state. Returns true if it changed (the engine then switches the pointer).
    pub fn set_active(&mut self, on: bool, out: &mut dyn Outputs) -> bool {
        if on == self.active {
            return false;
        }
        self.active = on;
        if on {
            if let Some(&l) = self.profile.levels.get(self.level) {
                out.led(l);
            }
            out.led(self.profile.led_on);
        } else {
            out.led(self.profile.led_off);
        }
        eprintln!("NumberPad {}", if on { "on" } else { "off" });
        true
    }

    fn set_level(&mut self, level: usize, out: &mut dyn Outputs) {
        self.level = level;
        out.led(self.profile.levels[level]);
        out.led(if self.active { self.profile.led_on } else { self.profile.led_off });
    }

    /// Run a NumberPad action. Returns false if it does nothing in the current state.
    pub fn op(&mut self, op: NumberPadOp, out: &mut dyn Outputs) -> bool {
        let n = self.profile.levels.len();
        match op {
            NumberPadOp::Toggle => {
                out.key(&[KEY_NUMLOCK], true);
                out.key(&[KEY_NUMLOCK], false);
                // The compositor echoes the NumLock LED back; flip now in case it does not.
                self.set_active(!self.active, out);
                true
            }
            _ if !self.active || n == 0 => false,
            NumberPadOp::BrightnessCycle => {
                self.set_level((self.level + 1) % n, out);
                true
            }
            NumberPadOp::BrightnessUp | NumberPadOp::BrightnessDown => {
                let up = op == NumberPadOp::BrightnessUp;
                let target = if up { (self.level + 1).min(n - 1) } else { self.level.saturating_sub(1) };
                if target == self.level {
                    return false;
                }
                self.set_level(target, out);
                true
            }
        }
    }
}
