//! ASUS NumberPad module. Only active on touchpads
//! with a known profile; profiles live in data/*.json.

use serde_json::Value;

use crate::device::TouchpadInfo;
use crate::keys;

const PROFILES: &[&str] = &[include_str!("../data/asus_ASUP1415.json")];

#[derive(Clone, Debug)]
pub struct PadKey {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub codes: Vec<u16>,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub model: String,
    pub keys: Vec<PadKey>,
    pub area: (i32, i32, i32, i32),
    pub led_on: u8,
    pub led_off: u8,
    pub levels: Vec<u8>,
    pub init: Vec<u8>,
}

fn label(name: &str) -> String {
    match name {
        "KEY_KPSLASH" => "/".into(),
        "KEY_KPASTERISK" => "*".into(),
        "KEY_KPMINUS" => "-".into(),
        "KEY_KPPLUS" => "+".into(),
        "KEY_KPDOT" => ".".into(),
        "KEY_KPENTER" => "↵".into(),
        "KEY_BACKSPACE" => "⌫".into(),
        "KEY_EQUAL" => "=".into(),
        "KEY_LEFTSHIFT+KEY_5" => "%".into(),
        n => n.trim_start_matches("KEY_KP").to_string(),
    }
}

fn parse(text: &str) -> Option<(String, Profile)> {
    let v: Value = serde_json::from_str(text).ok()?;
    let dev = &v["device"];
    let hid = dev["hid"].as_str()?.to_ascii_lowercase();
    let i = |x: &Value| x.as_i64().unwrap_or(0) as i32;
    let keys = v["keys"]
        .as_array()?
        .iter()
        .filter_map(|k| {
            let name = k["linux"].as_str()?;
            Some(PadKey {
                x0: i(&k["x0"]),
                y0: i(&k["y0"]),
                x1: i(&k["x1"]),
                y1: i(&k["y1"]),
                codes: keys::parse_key_combo(name).ok()?,
                label: label(name),
            })
        })
        .collect();
    let ka = &v["key_area"];
    let led = &v["led"]["values"];
    let u = |x: &Value| x.as_u64().unwrap_or(0) as u8;
    Some((
        hid,
        Profile {
            model: dev["model_name"].as_str().unwrap_or("NumberPad").into(),
            keys,
            area: (i(&ka["x0"]), i(&ka["y0"]), i(&ka["x1"]), i(&ka["y1"])),
            led_on: u(&led["numpad_led_on"]),
            led_off: u(&led["numpad_led_off"]),
            levels: vec![u(&led["brightness_level_1"]), u(&led["brightness_level_2"])],
            init: vec![u(&led["rdx_mode_off"])],
        },
    ))
}

pub fn profile_for(info: &TouchpadInfo) -> Option<Profile> {
    let id = info.hid_id();
    PROFILES.iter().filter_map(|t| parse(t)).find(|(hid, _)| *hid == id).map(|(_, p)| p)
}

#[cfg(test)]
pub fn test_profile() -> Profile {
    parse(PROFILES[0]).expect("built-in profile").1
}

/// HID feature report that sets a NumberPad LED value (report 0x0d).
pub fn led_report(value: u8) -> [u8; 5] {
    [0x0d, 0x14, 0x03, value, 0xad]
}

impl Profile {
    pub fn in_area(&self, x: i32, y: i32) -> bool {
        let (x0, y0, x1, y1) = self.area;
        x >= x0 && x < x1 && y >= y0 && y < y1
    }

    pub fn key_at(&self, x: i32, y: i32) -> Option<&PadKey> {
        self.keys.iter().find(|k| x >= k.x0 && x < k.x1 && y >= k.y0 && y < k.y1)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asus_profile_parses() {
        let (hid, p) = parse(PROFILES[0]).unwrap();
        assert_eq!(hid, "093a:300c");
        assert_eq!(p.keys.len(), 20);
        assert_eq!(p.key_at(1200, 1000).unwrap().label, "5");
        assert_eq!(p.key_at(3400, 2000).unwrap().label, "=");
        assert_eq!(p.key_at(3400, 1500).unwrap().codes, vec![42, 6]);
        assert_eq!(p.levels, vec![0x40, 0x41]);
        assert!(p.key_at(100, 100).is_none());
    }
}
