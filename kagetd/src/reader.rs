//! Passive (non-grabbing) evdev reader that assembles frames at SYN_REPORT.

use std::io;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};

use crate::device::TouchpadInfo;
use crate::sys::{self, InputEvent};

#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// CLOCK_MONOTONIC seconds of the SYN_REPORT.
    pub t: f64,
    pub events: Vec<InputEvent>,
    /// Events were lost before this frame (SYN_DROPPED): contact state must be reset.
    pub resync: bool,
}

pub struct Reader {
    fd: OwnedFd,
    pub info: TouchpadInfo,
    pending: Vec<InputEvent>,
    dropping: bool,
    resync: bool,
}

impl Reader {
    pub fn open(info: TouchpadInfo) -> io::Result<Self> {
        let fd = sys::open(&info.path, libc::O_RDONLY | libc::O_NONBLOCK)?;
        if let Err(e) = sys::set_monotonic_clock(fd.as_raw_fd()) {
            eprintln!("touchpad: cannot switch event clock to monotonic: {e}");
        }
        Ok(Self { fd, info, pending: Vec::new(), dropping: false, resync: false })
    }

    pub fn raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    /// Exclusive grab (filter mode). Released when the reader is dropped.
    pub fn grab(&self, on: bool) -> io::Result<()> {
        sys::grab(self.raw_fd(), on)
    }

    /// Is another process holding an exclusive grab? Probed by grabbing and releasing at once.
    pub fn grabbed_elsewhere(&self) -> bool {
        match sys::grab(self.raw_fd(), true) {
            Ok(()) => {
                let _ = sys::grab(self.raw_fd(), false);
                false
            }
            Err(e) => e.raw_os_error() == Some(libc::EBUSY),
        }
    }

    /// Read everything available and append complete frames. Errors (ENODEV/EIO) mean the device is gone.
    pub fn read_frames(&mut self, out: &mut Vec<Frame>) -> io::Result<()> {
        let mut evs = Vec::new();
        loop {
            evs.clear();
            if sys::read_events(self.raw_fd(), &mut evs)? == 0 {
                return Ok(());
            }
            for e in &evs {
                if e.type_ == sys::EV_SYN {
                    match e.code {
                        sys::SYN_DROPPED => {
                            self.dropping = true;
                            self.pending.clear();
                        }
                        sys::SYN_REPORT => {
                            if self.dropping {
                                // the frame after the drop restarts from a clean state
                                self.dropping = false;
                                self.resync = true;
                                self.pending.clear();
                                continue;
                            }
                            out.push(Frame {
                                t: e.time(),
                                events: std::mem::take(&mut self.pending),
                                resync: std::mem::take(&mut self.resync),
                            });
                        }
                        _ => {}
                    }
                } else if !self.dropping {
                    self.pending.push(*e);
                }
            }
        }
    }
}
