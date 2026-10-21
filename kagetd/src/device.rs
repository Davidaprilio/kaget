//! Touchpad discovery and capability probing.

use std::io;
use std::os::fd::{AsRawFd, OwnedFd};

use crate::sys::{self, AbsInfo, InputId};

#[derive(Clone, Debug)]
pub struct TouchpadInfo {
    pub path: String,
    pub sysfs: String,
    pub name: String,
    pub id: InputId,
    pub keys: Vec<u16>,
    pub abs: Vec<(u16, AbsInfo)>,
    pub msc: Vec<u16>,
    pub props: Vec<u16>,
    pub x: AbsInfo,
    pub y: AbsInfo,
    pub slots: u32,
    pub max_fingers: u32,
    pub semi_mt: bool,
}

impl TouchpadInfo {
    /// Width/height in mm; falls back to the given size when the device reports no resolution.
    pub fn size_mm(&self, fallback: (f64, f64)) -> (f64, f64) {
        let w = span(&self.x);
        let h = span(&self.y);
        if self.x.resolution > 0 && self.y.resolution > 0 {
            (w / self.x.resolution as f64, h / self.y.resolution as f64)
        } else {
            fallback
        }
    }

    /// Hardware vendor:product as "093a:300c".
    pub fn hid_id(&self) -> String {
        format!("{:04x}:{:04x}", self.id.vendor, self.id.product)
    }
}

fn span(a: &AbsInfo) -> f64 {
    (a.maximum - a.minimum).max(1) as f64
}

/// Is this event node backed by a virtual (uinput) device? Our own clone has the same name.
fn is_virtual(event_node: &str) -> (bool, String) {
    let link = format!("/sys/class/input/{event_node}/device");
    let real = std::fs::canonicalize(&link).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    (real.contains("/devices/virtual/"), real)
}

pub fn probe(path: &str) -> io::Result<TouchpadInfo> {
    let fd: OwnedFd = sys::open(path, libc::O_RDONLY | libc::O_NONBLOCK)?;
    let raw = fd.as_raw_fd();
    let evs = sys::bits(raw, 0, 4)?;
    if !evs.contains(&sys::EV_ABS) {
        return Err(io::Error::other("no EV_ABS"));
    }
    let absc = sys::bits(raw, sys::EV_ABS, 8)?;
    if !absc.contains(&sys::ABS_MT_POSITION_X) || !absc.contains(&sys::ABS_MT_POSITION_Y) {
        return Err(io::Error::other("not multitouch"));
    }
    let props = sys::props(raw)?;
    if !props.contains(&(sys::INPUT_PROP_POINTER as u16)) {
        return Err(io::Error::other("not a touchpad (direct touch device)"));
    }
    let keys = sys::bits(raw, sys::EV_KEY, 96)?;
    let msc = if evs.contains(&sys::EV_MSC) { sys::bits(raw, sys::EV_MSC, 1)? } else { vec![] };
    let mut abs = Vec::new();
    for &c in &absc {
        abs.push((c, sys::absinfo(raw, c)?));
    }
    let get = |c: u16| abs.iter().find(|(k, _)| *k == c).map(|(_, a)| *a).unwrap_or_default();
    let x = get(sys::ABS_MT_POSITION_X);
    let y = get(sys::ABS_MT_POSITION_Y);
    // Tracked contacts = MT slots (BTN_TOOL_* only counts fingers, it does not track positions).
    let slots = if absc.contains(&sys::ABS_MT_SLOT) { (get(sys::ABS_MT_SLOT).maximum + 1).max(1) as u32 } else { 1 };
    let node = path.rsplit('/').next().unwrap_or_default();
    let (_, sysfs) = is_virtual(node);
    Ok(TouchpadInfo {
        path: path.to_string(),
        sysfs,
        name: sys::device_name(raw)?,
        id: sys::device_id(raw)?,
        keys,
        abs,
        msc,
        props: props.clone(),
        x,
        y,
        slots,
        max_fingers: slots,
        semi_mt: props.contains(&(sys::INPUT_PROP_SEMI_MT as u16)),
    })
}

/// Find the first real (non-virtual) multitouch touchpad.
pub fn find_touchpad() -> io::Result<TouchpadInfo> {
    let mut nodes: Vec<String> = std::fs::read_dir("/dev/input")?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("event"))
        .collect();
    nodes.sort_by_key(|n| n[5..].parse::<u32>().unwrap_or(u32::MAX));
    for n in nodes {
        if is_virtual(&n).0 {
            continue;
        }
        if let Ok(info) = probe(&format!("/dev/input/{n}")) {
            return Ok(info);
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "no multitouch touchpad found"))
}

/// Find the hidraw node of the touchpad's HID device ("0018:093A:300C.*").
pub fn find_hidraw(info: &TouchpadInfo) -> Option<String> {
    let needle = format!(":{:04X}:{:04X}.", info.id.vendor, info.id.product);
    for e in std::fs::read_dir("/sys/class/hidraw").ok()?.flatten() {
        let dev = std::fs::canonicalize(e.path().join("device")).ok()?;
        if dev.to_string_lossy().to_uppercase().contains(&needle) {
            return Some(format!("/dev/{}", e.file_name().to_string_lossy()));
        }
    }
    None
}
