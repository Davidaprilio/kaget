//! Raw Linux input ioctls and structs (evdev, uinput, hidraw) on top of libc.
//! Some constants and uinput calls are only used by the touchpad clone of filter mode.
#![allow(dead_code)]

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_MSC: u16 = 0x04;
pub const EV_LED: u16 = 0x11;
pub const SYN_REPORT: u16 = 0;
pub const SYN_DROPPED: u16 = 3;
pub const LED_NUML: u16 = 0x00;

pub const BTN_LEFT: u16 = 0x110;
pub const BTN_TOOL_FINGER: u16 = 0x145;
pub const BTN_TOOL_QUINTTAP: u16 = 0x148;
pub const BTN_TOUCH: u16 = 0x14a;
pub const BTN_TOOL_DOUBLETAP: u16 = 0x14d;
pub const BTN_TOOL_TRIPLETAP: u16 = 0x14e;
pub const BTN_TOOL_QUADTAP: u16 = 0x14f;

pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_MT_SLOT: u16 = 0x2f;
pub const ABS_MT_POSITION_X: u16 = 0x35;
pub const ABS_MT_POSITION_Y: u16 = 0x36;
pub const ABS_MT_TOOL_TYPE: u16 = 0x37;
pub const ABS_MT_TRACKING_ID: u16 = 0x39;
pub const ABS_MT_FIRST: u16 = 0x2f;
pub const ABS_MT_LAST: u16 = 0x3d;

pub const INPUT_PROP_POINTER: u32 = 0x00;
pub const INPUT_PROP_SEMI_MT: u32 = 0x03;

pub const BUS_VIRTUAL: u16 = 0x06;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct InputEvent {
    pub sec: libc::time_t,
    pub usec: libc::suseconds_t,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct AbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

#[repr(C)]
struct UinputAbsSetup {
    code: u16,
    absinfo: AbsInfo,
}

const IOC_WRITE: libc::c_ulong = 1;
const IOC_READ: libc::c_ulong = 2;

const fn ioc(dir: libc::c_ulong, typ: u8, nr: u8, size: usize) -> libc::c_ulong {
    (dir << 30) | ((size as libc::c_ulong) << 16) | ((typ as libc::c_ulong) << 8) | nr as libc::c_ulong
}

const EVIOCGRAB: libc::c_ulong = ioc(IOC_WRITE, b'E', 0x90, 4);
const EVIOCSCLOCKID: libc::c_ulong = ioc(IOC_WRITE, b'E', 0xa0, 4);
const EVIOCGID: libc::c_ulong = ioc(IOC_READ, b'E', 0x02, std::mem::size_of::<InputId>());
const fn eviocgname(len: usize) -> libc::c_ulong {
    ioc(IOC_READ, b'E', 0x06, len)
}
const fn eviocgprop(len: usize) -> libc::c_ulong {
    ioc(IOC_READ, b'E', 0x09, len)
}
const fn eviocgbit(ev: u16, len: usize) -> libc::c_ulong {
    ioc(IOC_READ, b'E', 0x20 + ev as u8, len)
}
const fn eviocgabs(code: u16) -> libc::c_ulong {
    ioc(IOC_READ, b'E', 0x40 + code as u8, std::mem::size_of::<AbsInfo>())
}

const UI_SET_EVBIT: libc::c_ulong = ioc(IOC_WRITE, b'U', 100, 4);
const UI_SET_KEYBIT: libc::c_ulong = ioc(IOC_WRITE, b'U', 101, 4);
const UI_SET_ABSBIT: libc::c_ulong = ioc(IOC_WRITE, b'U', 103, 4);
const UI_SET_MSCBIT: libc::c_ulong = ioc(IOC_WRITE, b'U', 104, 4);
const UI_SET_LEDBIT: libc::c_ulong = ioc(IOC_WRITE, b'U', 105, 4);
const UI_SET_PROPBIT: libc::c_ulong = ioc(IOC_WRITE, b'U', 110, 4);
const UI_ABS_SETUP: libc::c_ulong = ioc(IOC_WRITE, b'U', 4, std::mem::size_of::<UinputAbsSetup>());
const UI_DEV_SETUP: libc::c_ulong = ioc(IOC_WRITE, b'U', 3, std::mem::size_of::<UinputSetup>());
const UI_DEV_CREATE: libc::c_ulong = ioc(0, b'U', 1, 0);
const UI_DEV_DESTROY: libc::c_ulong = ioc(0, b'U', 2, 0);

const fn hidiocsfeature(len: usize) -> libc::c_ulong {
    ioc(IOC_WRITE | IOC_READ, b'H', 0x06, len)
}

fn check(r: libc::c_int) -> io::Result<libc::c_int> {
    if r < 0 { Err(io::Error::last_os_error()) } else { Ok(r) }
}

pub fn open(path: &str, flags: libc::c_int) -> io::Result<OwnedFd> {
    let c = CString::new(path).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let fd = check(unsafe { libc::open(c.as_ptr(), flags | libc::O_CLOEXEC) })?;
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

pub fn grab(fd: RawFd, on: bool) -> io::Result<()> {
    check(unsafe { libc::ioctl(fd, EVIOCGRAB, if on { 1 } else { 0 } as libc::c_long) }).map(|_| ())
}

/// Timestamp events with CLOCK_MONOTONIC so they match `now()`.
pub fn set_monotonic_clock(fd: RawFd) -> io::Result<()> {
    let id: libc::c_int = libc::CLOCK_MONOTONIC;
    check(unsafe { libc::ioctl(fd, EVIOCSCLOCKID, &id as *const libc::c_int) }).map(|_| ())
}

/// Seconds on CLOCK_MONOTONIC.
pub fn now() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
}

pub fn device_name(fd: RawFd) -> io::Result<String> {
    let mut buf = [0u8; 256];
    check(unsafe { libc::ioctl(fd, eviocgname(buf.len()), buf.as_mut_ptr()) })?;
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

pub fn device_id(fd: RawFd) -> io::Result<InputId> {
    let mut id = InputId::default();
    check(unsafe { libc::ioctl(fd, EVIOCGID, &mut id as *mut InputId) })?;
    Ok(id)
}

/// Bit numbers set in an EVIOCGBIT bitmap (ev = 0 returns the event types).
pub fn bits(fd: RawFd, ev: u16, nbytes: usize) -> io::Result<Vec<u16>> {
    let mut buf = vec![0u8; nbytes];
    check(unsafe { libc::ioctl(fd, eviocgbit(ev, nbytes), buf.as_mut_ptr()) })?;
    Ok(bitmap_to_vec(&buf))
}

pub fn props(fd: RawFd) -> io::Result<Vec<u16>> {
    let mut buf = [0u8; 4];
    check(unsafe { libc::ioctl(fd, eviocgprop(buf.len()), buf.as_mut_ptr()) })?;
    Ok(bitmap_to_vec(&buf))
}

fn bitmap_to_vec(buf: &[u8]) -> Vec<u16> {
    (0..buf.len() * 8).filter(|i| buf[i / 8] >> (i % 8) & 1 == 1).map(|i| i as u16).collect()
}

pub fn absinfo(fd: RawFd, code: u16) -> io::Result<AbsInfo> {
    let mut a = AbsInfo::default();
    check(unsafe { libc::ioctl(fd, eviocgabs(code), &mut a as *mut AbsInfo) })?;
    Ok(a)
}

/// Read as many input events as are available (fd may be non-blocking).
pub fn read_events(fd: RawFd, out: &mut Vec<InputEvent>) -> io::Result<usize> {
    let mut buf = [InputEvent::default(); 64];
    let n = unsafe {
        libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, std::mem::size_of_val(&buf))
    };
    if n < 0 {
        let e = io::Error::last_os_error();
        return if e.kind() == io::ErrorKind::WouldBlock { Ok(0) } else { Err(e) };
    }
    let count = n as usize / std::mem::size_of::<InputEvent>();
    out.extend_from_slice(&buf[..count]);
    Ok(count)
}

pub fn write_events(fd: RawFd, evs: &[InputEvent]) -> io::Result<()> {
    if evs.is_empty() {
        return Ok(());
    }
    let bytes = std::mem::size_of_val(evs);
    let n = unsafe { libc::write(fd, evs.as_ptr() as *const libc::c_void, bytes) };
    if n < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

pub fn event(type_: u16, code: u16, value: i32) -> InputEvent {
    InputEvent { sec: 0, usec: 0, type_, code, value }
}

impl InputEvent {
    pub fn time(&self) -> f64 {
        self.sec as f64 + self.usec as f64 * 1e-6
    }
}

/// Builder for a uinput device.
pub struct UinputBuilder {
    fd: OwnedFd,
}

impl UinputBuilder {
    pub fn new() -> io::Result<Self> {
        Ok(Self { fd: open("/dev/uinput", libc::O_RDWR | libc::O_NONBLOCK)? })
    }
    fn set(&self, req: libc::c_ulong, v: u32) -> io::Result<()> {
        check(unsafe { libc::ioctl(self.fd.as_raw_fd(), req, v as libc::c_long) }).map(|_| ())
    }
    pub fn evbit(&self, ev: u16) -> io::Result<()> {
        self.set(UI_SET_EVBIT, ev as u32)
    }
    pub fn keybit(&self, k: u16) -> io::Result<()> {
        self.set(UI_SET_KEYBIT, k as u32)
    }
    pub fn mscbit(&self, m: u16) -> io::Result<()> {
        self.set(UI_SET_MSCBIT, m as u32)
    }
    pub fn ledbit(&self, l: u16) -> io::Result<()> {
        self.set(UI_SET_LEDBIT, l as u32)
    }
    pub fn propbit(&self, p: u16) -> io::Result<()> {
        self.set(UI_SET_PROPBIT, p as u32)
    }
    pub fn abs(&self, code: u16, info: AbsInfo) -> io::Result<()> {
        self.set(UI_SET_ABSBIT, code as u32)?;
        let s = UinputAbsSetup { code, absinfo: info };
        check(unsafe { libc::ioctl(self.fd.as_raw_fd(), UI_ABS_SETUP, &s as *const UinputAbsSetup) })
            .map(|_| ())
    }
    pub fn create(self, id: InputId, name: &str) -> io::Result<OwnedFd> {
        let mut setup = UinputSetup { id, name: [0; 80], ff_effects_max: 0 };
        let n = name.len().min(79);
        setup.name[..n].copy_from_slice(&name.as_bytes()[..n]);
        check(unsafe { libc::ioctl(self.fd.as_raw_fd(), UI_DEV_SETUP, &setup as *const UinputSetup) })?;
        check(unsafe { libc::ioctl(self.fd.as_raw_fd(), UI_DEV_CREATE) })?;
        Ok(self.fd)
    }
}

pub fn uinput_destroy(fd: RawFd) {
    unsafe {
        libc::ioctl(fd, UI_DEV_DESTROY);
    }
}

pub fn hid_set_feature(path: &str, report: &[u8]) -> io::Result<()> {
    let fd = open(path, libc::O_RDWR)?;
    let mut buf = report.to_vec();
    check(unsafe { libc::ioctl(fd.as_raw_fd(), hidiocsfeature(buf.len()), buf.as_mut_ptr()) }).map(|_| ())
}

/// Wait until one of the fds is readable or the timeout expires. Returns readiness per fd.
pub fn poll(fds: &[RawFd], timeout_ms: i32) -> io::Result<Vec<bool>> {
    let mut pfds: Vec<libc::pollfd> =
        fds.iter().map(|&fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 }).collect();
    let r = unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, timeout_ms) };
    if r < 0 {
        let e = io::Error::last_os_error();
        if e.kind() == io::ErrorKind::Interrupted {
            return Ok(vec![false; fds.len()]);
        }
        return Err(e);
    }
    Ok(pfds.iter().map(|p| p.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0).collect())
}
