//! Linux key codes (input-event-codes.h subset) and shortcut parsing.

pub const KEYS: &[(&str, u16)] = &[
    ("KEY_ESC", 1), ("KEY_1", 2), ("KEY_2", 3), ("KEY_3", 4), ("KEY_4", 5), ("KEY_5", 6), ("KEY_6", 7),
    ("KEY_7", 8), ("KEY_8", 9), ("KEY_9", 10), ("KEY_0", 11), ("KEY_MINUS", 12), ("KEY_EQUAL", 13),
    ("KEY_BACKSPACE", 14), ("KEY_TAB", 15), ("KEY_Q", 16), ("KEY_W", 17), ("KEY_E", 18), ("KEY_R", 19),
    ("KEY_T", 20), ("KEY_Y", 21), ("KEY_U", 22), ("KEY_I", 23), ("KEY_O", 24), ("KEY_P", 25),
    ("KEY_LEFTBRACE", 26), ("KEY_RIGHTBRACE", 27), ("KEY_ENTER", 28), ("KEY_LEFTCTRL", 29), ("KEY_A", 30),
    ("KEY_S", 31), ("KEY_D", 32), ("KEY_F", 33), ("KEY_G", 34), ("KEY_H", 35), ("KEY_J", 36), ("KEY_K", 37),
    ("KEY_L", 38), ("KEY_SEMICOLON", 39), ("KEY_APOSTROPHE", 40), ("KEY_GRAVE", 41), ("KEY_LEFTSHIFT", 42),
    ("KEY_BACKSLASH", 43), ("KEY_Z", 44), ("KEY_X", 45), ("KEY_C", 46), ("KEY_V", 47), ("KEY_B", 48),
    ("KEY_N", 49), ("KEY_M", 50), ("KEY_COMMA", 51), ("KEY_DOT", 52), ("KEY_SLASH", 53),
    ("KEY_RIGHTSHIFT", 54), ("KEY_KPASTERISK", 55), ("KEY_LEFTALT", 56), ("KEY_SPACE", 57),
    ("KEY_CAPSLOCK", 58), ("KEY_F1", 59), ("KEY_F2", 60), ("KEY_F3", 61), ("KEY_F4", 62), ("KEY_F5", 63),
    ("KEY_F6", 64), ("KEY_F7", 65), ("KEY_F8", 66), ("KEY_F9", 67), ("KEY_F10", 68), ("KEY_NUMLOCK", 69),
    ("KEY_SCROLLLOCK", 70), ("KEY_KP7", 71), ("KEY_KP8", 72), ("KEY_KP9", 73), ("KEY_KPMINUS", 74),
    ("KEY_KP4", 75), ("KEY_KP5", 76), ("KEY_KP6", 77), ("KEY_KPPLUS", 78), ("KEY_KP1", 79), ("KEY_KP2", 80),
    ("KEY_KP3", 81), ("KEY_KP0", 82), ("KEY_KPDOT", 83), ("KEY_F11", 87), ("KEY_F12", 88),
    ("KEY_KPENTER", 96), ("KEY_RIGHTCTRL", 97), ("KEY_KPSLASH", 98), ("KEY_SYSRQ", 99), ("KEY_RIGHTALT", 100),
    ("KEY_HOME", 102), ("KEY_UP", 103), ("KEY_PAGEUP", 104), ("KEY_LEFT", 105), ("KEY_RIGHT", 106),
    ("KEY_END", 107), ("KEY_DOWN", 108), ("KEY_PAGEDOWN", 109), ("KEY_INSERT", 110), ("KEY_DELETE", 111),
    ("KEY_MUTE", 113), ("KEY_VOLUMEDOWN", 114), ("KEY_VOLUMEUP", 115), ("KEY_POWER", 116),
    ("KEY_KPEQUAL", 117), ("KEY_PAUSE", 119), ("KEY_LEFTMETA", 125), ("KEY_RIGHTMETA", 126),
    ("KEY_COMPOSE", 127), ("KEY_STOP", 128), ("KEY_AGAIN", 129), ("KEY_UNDO", 131), ("KEY_COPY", 133),
    ("KEY_PASTE", 135), ("KEY_FIND", 136), ("KEY_CUT", 137), ("KEY_HELP", 138), ("KEY_MENU", 139),
    ("KEY_CALC", 140), ("KEY_SLEEP", 142), ("KEY_WWW", 150), ("KEY_MAIL", 155), ("KEY_BOOKMARKS", 156),
    ("KEY_COMPUTER", 157), ("KEY_BACK", 158), ("KEY_FORWARD", 159), ("KEY_NEXTSONG", 163),
    ("KEY_PLAYPAUSE", 164), ("KEY_PREVIOUSSONG", 165), ("KEY_STOPCD", 166), ("KEY_REFRESH", 173),
    ("KEY_F13", 183), ("KEY_F14", 184), ("KEY_F15", 185), ("KEY_F16", 186), ("KEY_F17", 187),
    ("KEY_F18", 188), ("KEY_F19", 189), ("KEY_F20", 190), ("KEY_F21", 191), ("KEY_F22", 192),
    ("KEY_F23", 193), ("KEY_F24", 194), ("KEY_PRINT", 210), ("KEY_SEARCH", 217),
    ("KEY_BRIGHTNESSDOWN", 224), ("KEY_BRIGHTNESSUP", 225), ("KEY_MEDIA", 226),
    ("KEY_KBDILLUMTOGGLE", 228), ("KEY_KBDILLUMDOWN", 229), ("KEY_KBDILLUMUP", 230),
    ("KEY_MICMUTE", 248), ("KEY_ZOOMIN", 418), ("KEY_ZOOMOUT", 419),
];

pub fn code(name: &str) -> Option<u16> {
    let n = name.trim().to_ascii_uppercase();
    let n = if n.starts_with("KEY_") { n } else { format!("KEY_{n}") };
    KEYS.iter().find(|(k, _)| *k == n).map(|&(_, c)| c)
}

pub fn all_codes() -> Vec<u16> {
    KEYS.iter().map(|&(_, c)| c).collect()
}

/// `KEY_LEFTCTRL+KEY_T` -> codes.
pub fn parse_key_combo(s: &str) -> Result<Vec<u16>, String> {
    s.split('+').map(|p| code(p).ok_or_else(|| format!("unknown key '{p}'"))).collect()
}

/// Human shortcut `Ctrl+Alt+T`, `Meta+Left`, `Ctrl+Shift+Tab` -> codes (US layout).
pub fn parse_shortcut(s: &str) -> Result<Vec<u16>, String> {
    let mut out = Vec::new();
    for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let lower = part.to_ascii_lowercase();
        let name = match lower.as_str() {
            "ctrl" | "control" => "KEY_LEFTCTRL",
            "alt" => "KEY_LEFTALT",
            "altgr" => "KEY_RIGHTALT",
            "shift" => "KEY_LEFTSHIFT",
            "meta" | "super" | "win" => "KEY_LEFTMETA",
            "esc" | "escape" => "KEY_ESC",
            "return" | "enter" => "KEY_ENTER",
            "del" => "KEY_DELETE",
            "ins" => "KEY_INSERT",
            "pgup" => "KEY_PAGEUP",
            "pgdown" | "pgdn" => "KEY_PAGEDOWN",
            "print" | "printscreen" => "KEY_SYSRQ",
            "-" => "KEY_MINUS",
            "=" => "KEY_EQUAL",
            "[" => "KEY_LEFTBRACE",
            "]" => "KEY_RIGHTBRACE",
            ";" => "KEY_SEMICOLON",
            "'" => "KEY_APOSTROPHE",
            "`" => "KEY_GRAVE",
            "\\" => "KEY_BACKSLASH",
            "," => "KEY_COMMA",
            "." => "KEY_DOT",
            "/" => "KEY_SLASH",
            _ => "",
        };
        let c = if name.is_empty() { code(part) } else { code(name) };
        out.push(c.ok_or_else(|| format!("unknown key '{part}'"))?);
    }
    if out.is_empty() {
        return Err("empty shortcut".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts() {
        assert_eq!(parse_shortcut("Ctrl+Alt+T").unwrap(), vec![29, 56, 20]);
        assert_eq!(parse_shortcut("Meta+Left").unwrap(), vec![125, 105]);
        assert_eq!(parse_shortcut("ctrl+shift+tab").unwrap(), vec![29, 42, 15]);
        assert!(parse_shortcut("Ctrl+Nope").is_err());
        assert_eq!(parse_key_combo("KEY_LEFTSHIFT+KEY_5").unwrap(), vec![42, 6]);
    }
}
