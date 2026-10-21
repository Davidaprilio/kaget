//! uinput devices: a clone of the real touchpad and a virtual keyboard.

use std::io;
use std::os::fd::{AsRawFd, OwnedFd};

use crate::device::TouchpadInfo;
use crate::sys::{self, InputEvent, InputId};

/// Clone of the real touchpad for filter mode: same name, IDs, keys, axes and properties, so
/// desktop touchpad settings (keyed by name/vendor/product) apply to it.
pub struct VirtualTouchpad {
    fd: OwnedFd,
}

impl VirtualTouchpad {
    pub fn new(src: &TouchpadInfo) -> io::Result<Self> {
        let b = sys::UinputBuilder::new()?;
        for ev in [sys::EV_SYN, sys::EV_KEY, sys::EV_ABS, sys::EV_MSC] {
            b.evbit(ev)?;
        }
        for &k in &src.keys {
            b.keybit(k)?;
        }
        for &m in &src.msc {
            b.mscbit(m)?;
        }
        for &p in &src.props {
            b.propbit(p)?;
        }
        for &(code, info) in &src.abs {
            b.abs(code, info)?;
        }
        let fd = b.create(src.id, &src.name)?;
        Ok(Self { fd })
    }

    /// Write one complete frame (SYN_REPORT included).
    pub fn write_frame(&self, evs: &[InputEvent]) -> io::Result<()> {
        sys::write_events(self.fd.as_raw_fd(), evs)
    }
}

impl Drop for VirtualTouchpad {
    fn drop(&mut self) {
        sys::uinput_destroy(self.fd.as_raw_fd());
    }
}

/// Virtual keyboard for key/shortcut actions. It declares LED_NUML so the compositor pushes
/// the NumLock state to it (used by the NumberPad module).
pub struct VirtualKeyboard {
    fd: OwnedFd,
}

impl VirtualKeyboard {
    pub fn new(keys: &[u16]) -> io::Result<Self> {
        let b = sys::UinputBuilder::new()?;
        for ev in [sys::EV_SYN, sys::EV_KEY, sys::EV_LED] {
            b.evbit(ev)?;
        }
        b.ledbit(sys::LED_NUML)?;
        for &k in keys {
            b.keybit(k)?;
        }
        // BUS_VIRTUAL keeps libinput from pairing it with the touchpad for disable-while-typing.
        let id = InputId { bustype: sys::BUS_VIRTUAL, vendor: 0x4b41, product: 0x4754, version: 1 };
        Ok(Self { fd: b.create(id, "KAGeT virtual keyboard")? })
    }

    pub fn raw_fd(&self) -> i32 {
        self.fd.as_raw_fd()
    }

    pub fn send(&self, events: &[(u16, i32)]) -> io::Result<()> {
        let mut evs: Vec<InputEvent> = events.iter().map(|&(c, v)| sys::event(sys::EV_KEY, c, v)).collect();
        evs.push(sys::event(sys::EV_SYN, sys::SYN_REPORT, 0));
        sys::write_events(self.fd.as_raw_fd(), &evs)
    }

    /// Press all codes in order, release them in reverse order.
    pub fn tap(&self, codes: &[u16]) -> io::Result<()> {
        let down: Vec<(u16, i32)> = codes.iter().map(|&c| (c, 1)).collect();
        let up: Vec<(u16, i32)> = codes.iter().rev().map(|&c| (c, 0)).collect();
        self.send(&down)?;
        self.send(&up)
    }

    /// Latest NumLock LED value pushed by the compositor since the last call, if any.
    pub fn read_numlock(&self) -> Option<bool> {
        let mut evs = Vec::new();
        while let Ok(n) = sys::read_events(self.fd.as_raw_fd(), &mut evs) {
            if n == 0 {
                break;
            }
        }
        evs.iter().rev().find(|e| e.type_ == sys::EV_LED && e.code == sys::LED_NUML).map(|e| e.value != 0)
    }
}

impl Drop for VirtualKeyboard {
    fn drop(&mut self) {
        sys::uinput_destroy(self.fd.as_raw_fd());
    }
}
