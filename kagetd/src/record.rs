//! Recording format (`kagetd --record`): JSON lines. The first line describes the device, every
//! further line is one frame `{t, events: [[type, code, value], ...]}`.

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::device::TouchpadInfo;
use crate::engine::Engine;
use crate::engine::outputs::{FakeOutputs, Fired, Outputs};
use crate::geometry::Geometry;
use crate::reader::Frame;
use crate::sys;
use crate::tracker::Tracker;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DeviceHeader {
    pub name: String,
    pub vendor: u16,
    pub product: u16,
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
    pub res_x: i32,
    pub res_y: i32,
    pub slots: u32,
    pub semi_mt: bool,
}

#[derive(Serialize, Deserialize)]
struct Header {
    device: DeviceHeader,
}

#[derive(Serialize, Deserialize)]
struct Line {
    t: f64,
    events: Vec<(u16, u16, i32)>,
}

impl DeviceHeader {
    pub fn from_info(i: &TouchpadInfo) -> Self {
        Self {
            name: i.name.clone(),
            vendor: i.id.vendor,
            product: i.id.product,
            x_min: i.x.minimum,
            x_max: i.x.maximum,
            y_min: i.y.minimum,
            y_max: i.y.maximum,
            res_x: i.x.resolution,
            res_y: i.y.resolution,
            slots: i.max_fingers,
            semi_mt: i.semi_mt,
        }
    }

    pub fn geometry(&self, cfg: &Config) -> Geometry {
        let size = if self.res_x > 0 && self.res_y > 0 {
            (
                (self.x_max - self.x_min).max(1) as f64 / self.res_x as f64,
                (self.y_max - self.y_min).max(1) as f64 / self.res_y as f64,
            )
        } else {
            (cfg.device.width_mm, cfg.device.height_mm)
        };
        Geometry::new(self.x_min, self.x_max, self.y_min, self.y_max, size, cfg)
    }
}

pub struct Recorder<W: Write> {
    w: W,
}

impl<W: Write> Recorder<W> {
    pub fn new(mut w: W, dev: &DeviceHeader) -> io::Result<Self> {
        serde_json::to_writer(&mut w, &Header { device: dev.clone() })?;
        w.write_all(b"\n")?;
        Ok(Self { w })
    }

    pub fn frame(&mut self, f: &Frame) -> io::Result<()> {
        let line = Line { t: f.t, events: f.events.iter().map(|e| (e.type_, e.code, e.value)).collect() };
        serde_json::to_writer(&mut self.w, &line)?;
        self.w.write_all(b"\n")?;
        self.w.flush()
    }
}

/// Outputs that remember when each binding fired.
#[derive(Default)]
pub struct Timed {
    pub now: f64,
    pub fired: Vec<(f64, Fired)>,
    pub rest: FakeOutputs,
}

impl Outputs for Timed {
    fn fired(&mut self, f: &Fired) {
        self.fired.push((self.now, f.clone()));
    }
    fn key(&mut self, codes: &[u16], down: bool) {
        self.rest.key(codes, down);
    }
    fn led(&mut self, value: u8) {
        self.rest.led(value);
    }
    fn pointer(&mut self, enabled: bool) {
        self.rest.pointer(enabled);
    }
    fn state_changed(&mut self) {}
}

/// Feed a recording through a fresh engine, with 10 ms timer ticks between frames like the main loop.
pub fn replay(input: impl BufRead, cfg: &Config) -> Result<Timed, String> {
    let mut lines = input.lines();
    let first = lines.next().ok_or("empty recording")?.map_err(|e| e.to_string())?;
    let header: Header = serde_json::from_str(&first).map_err(|e| format!("bad header: {e}"))?;
    let dev = header.device;
    let geo = dev.geometry(cfg);
    let mut engine = Engine::new(cfg.clone(), geo, dev.slots, dev.semi_mt, None);
    let mut tracker = Tracker::default();
    let mut out = Timed::default();
    let mut last: Option<f64> = None;
    for (n, line) in lines.enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let l: Line = serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", n + 2))?;
        if let Some(mut t) = last {
            while t + 0.01 < l.t {
                t += 0.01;
                out.now = t;
                engine.tick(&tracker, t, &mut out);
            }
        }
        last = Some(l.t);
        let evs: Vec<_> = l.events.iter().map(|&(ty, c, v)| sys::event(ty, c, v)).collect();
        out.now = l.t;
        tracker.apply(&evs, l.t);
        engine.frame(&tracker, l.t, &mut out);
        tracker.remove_lifted();
    }
    if let Some(t) = last {
        // let pending timers (double tap windows) run out
        out.now = t + 1.0;
        engine.tick(&tracker, t + 1.0, &mut out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Binding;
    use crate::sys::event;

    fn dev() -> DeviceHeader {
        DeviceHeader {
            name: "test".into(),
            vendor: 0x093a,
            product: 0x300c,
            x_min: 0,
            x_max: 3996,
            y_min: 0,
            y_max: 2242,
            res_x: 31,
            res_y: 31,
            slots: 5,
            semi_mt: false,
        }
    }

    #[test]
    fn record_then_replay_hold() {
        let mut buf = Vec::new();
        {
            let mut r = Recorder::new(&mut buf, &dev()).unwrap();
            let abs = |c, v| event(sys::EV_ABS, c, v);
            r.frame(&Frame { t: 10.0, events: vec![abs(sys::ABS_MT_SLOT, 0), abs(sys::ABS_MT_TRACKING_ID, 7),
                abs(sys::ABS_MT_POSITION_X, 3900), abs(sys::ABS_MT_POSITION_Y, 100)], resync: false }).unwrap();
            r.frame(&Frame { t: 10.5, events: vec![abs(sys::ABS_MT_TRACKING_ID, -1)], resync: false }).unwrap();
        }
        let cfg = Config {
            bindings: vec![Binding { trigger: "corner.top_right.hold".into(), action: "cmd:true".into(), when_running: None }],
            ..Default::default()
        };
        let out = replay(&buf[..], &cfg).unwrap();
        assert_eq!(out.fired.len(), 1);
        let (t, f) = &out.fired[0];
        assert_eq!(f.trigger, "corner.top_right.hold");
        assert!((t - 10.32).abs() < 0.011, "{t}");
    }
}
