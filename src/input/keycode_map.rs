use evdev::Key;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Map of key name strings to evdev Key codes.
/// Key names are lowercase; lookup should be case-insensitive.
static KEY_MAP: LazyLock<HashMap<&'static str, Key>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Letters
    m.insert("a", Key::KEY_A);
    m.insert("b", Key::KEY_B);
    m.insert("c", Key::KEY_C);
    m.insert("d", Key::KEY_D);
    m.insert("e", Key::KEY_E);
    m.insert("f", Key::KEY_F);
    m.insert("g", Key::KEY_G);
    m.insert("h", Key::KEY_H);
    m.insert("i", Key::KEY_I);
    m.insert("j", Key::KEY_J);
    m.insert("k", Key::KEY_K);
    m.insert("l", Key::KEY_L);
    m.insert("m", Key::KEY_M);
    m.insert("n", Key::KEY_N);
    m.insert("o", Key::KEY_O);
    m.insert("p", Key::KEY_P);
    m.insert("q", Key::KEY_Q);
    m.insert("r", Key::KEY_R);
    m.insert("s", Key::KEY_S);
    m.insert("t", Key::KEY_T);
    m.insert("u", Key::KEY_U);
    m.insert("v", Key::KEY_V);
    m.insert("w", Key::KEY_W);
    m.insert("x", Key::KEY_X);
    m.insert("y", Key::KEY_Y);
    m.insert("z", Key::KEY_Z);

    // Numbers
    m.insert("0", Key::KEY_0);
    m.insert("1", Key::KEY_1);
    m.insert("2", Key::KEY_2);
    m.insert("3", Key::KEY_3);
    m.insert("4", Key::KEY_4);
    m.insert("5", Key::KEY_5);
    m.insert("6", Key::KEY_6);
    m.insert("7", Key::KEY_7);
    m.insert("8", Key::KEY_8);
    m.insert("9", Key::KEY_9);

    // Function keys
    m.insert("f1", Key::KEY_F1);
    m.insert("f2", Key::KEY_F2);
    m.insert("f3", Key::KEY_F3);
    m.insert("f4", Key::KEY_F4);
    m.insert("f5", Key::KEY_F5);
    m.insert("f6", Key::KEY_F6);
    m.insert("f7", Key::KEY_F7);
    m.insert("f8", Key::KEY_F8);
    m.insert("f9", Key::KEY_F9);
    m.insert("f10", Key::KEY_F10);
    m.insert("f11", Key::KEY_F11);
    m.insert("f12", Key::KEY_F12);

    // Special keys
    m.insert("return", Key::KEY_ENTER);
    m.insert("enter", Key::KEY_ENTER);
    m.insert("tab", Key::KEY_TAB);
    m.insert("space", Key::KEY_SPACE);
    m.insert("backspace", Key::KEY_BACKSPACE);
    m.insert("delete", Key::KEY_BACKSPACE);
    m.insert("forwarddelete", Key::KEY_DELETE);
    m.insert("escape", Key::KEY_ESC);
    m.insert("esc", Key::KEY_ESC);

    // Arrow keys
    m.insert("left", Key::KEY_LEFT);
    m.insert("leftarrow", Key::KEY_LEFT);
    m.insert("right", Key::KEY_RIGHT);
    m.insert("rightarrow", Key::KEY_RIGHT);
    m.insert("up", Key::KEY_UP);
    m.insert("uparrow", Key::KEY_UP);
    m.insert("down", Key::KEY_DOWN);
    m.insert("downarrow", Key::KEY_DOWN);

    // Navigation
    m.insert("home", Key::KEY_HOME);
    m.insert("end", Key::KEY_END);
    m.insert("pageup", Key::KEY_PAGEUP);
    m.insert("pagedown", Key::KEY_PAGEDOWN);

    // Modifiers
    m.insert("shift", Key::KEY_LEFTSHIFT);
    m.insert("control", Key::KEY_LEFTCTRL);
    m.insert("ctrl", Key::KEY_LEFTCTRL);
    m.insert("alt", Key::KEY_LEFTALT);
    m.insert("option", Key::KEY_LEFTALT);
    m.insert("super", Key::KEY_LEFTMETA);
    m.insert("command", Key::KEY_LEFTMETA);
    m.insert("cmd", Key::KEY_LEFTMETA);
    m.insert("meta", Key::KEY_LEFTMETA);
    m.insert("capslock", Key::KEY_CAPSLOCK);

    // Punctuation
    m.insert("minus", Key::KEY_MINUS);
    m.insert("-", Key::KEY_MINUS);
    m.insert("equal", Key::KEY_EQUAL);
    m.insert("=", Key::KEY_EQUAL);
    m.insert("leftbracket", Key::KEY_LEFTBRACE);
    m.insert("[", Key::KEY_LEFTBRACE);
    m.insert("rightbracket", Key::KEY_RIGHTBRACE);
    m.insert("]", Key::KEY_RIGHTBRACE);
    m.insert("semicolon", Key::KEY_SEMICOLON);
    m.insert(";", Key::KEY_SEMICOLON);
    m.insert("quote", Key::KEY_APOSTROPHE);
    m.insert("'", Key::KEY_APOSTROPHE);
    m.insert("backslash", Key::KEY_BACKSLASH);
    m.insert("\\", Key::KEY_BACKSLASH);
    m.insert("comma", Key::KEY_COMMA);
    m.insert(",", Key::KEY_COMMA);
    m.insert("period", Key::KEY_DOT);
    m.insert(".", Key::KEY_DOT);
    m.insert("slash", Key::KEY_SLASH);
    m.insert("/", Key::KEY_SLASH);
    m.insert("grave", Key::KEY_GRAVE);
    m.insert("`", Key::KEY_GRAVE);

    // Media
    m.insert("volumeup", Key::KEY_VOLUMEUP);
    m.insert("volumedown", Key::KEY_VOLUMEDOWN);
    m.insert("mute", Key::KEY_MUTE);

    m
});

/// Characters that need Shift to type.
static SHIFT_CHARS: LazyLock<HashMap<char, Key>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert('!', Key::KEY_1);
    m.insert('@', Key::KEY_2);
    m.insert('#', Key::KEY_3);
    m.insert('$', Key::KEY_4);
    m.insert('%', Key::KEY_5);
    m.insert('^', Key::KEY_6);
    m.insert('&', Key::KEY_7);
    m.insert('*', Key::KEY_8);
    m.insert('(', Key::KEY_9);
    m.insert(')', Key::KEY_0);
    m.insert('_', Key::KEY_MINUS);
    m.insert('+', Key::KEY_EQUAL);
    m.insert('{', Key::KEY_LEFTBRACE);
    m.insert('}', Key::KEY_RIGHTBRACE);
    m.insert('|', Key::KEY_BACKSLASH);
    m.insert(':', Key::KEY_SEMICOLON);
    m.insert('"', Key::KEY_APOSTROPHE);
    m.insert('<', Key::KEY_COMMA);
    m.insert('>', Key::KEY_DOT);
    m.insert('?', Key::KEY_SLASH);
    m.insert('~', Key::KEY_GRAVE);
    m
});

/// Look up a key name to an evdev Key code.
pub fn key_name_to_code(name: &str) -> Option<Key> {
    KEY_MAP.get(name.to_lowercase().as_str()).copied()
}

/// Get the Key code for a character, and whether Shift is needed.
pub fn char_to_key(ch: char) -> Option<(Key, bool)> {
    if ch.is_ascii_uppercase() {
        let lower = ch.to_ascii_lowercase().to_string();
        KEY_MAP.get(lower.as_str()).map(|&k| (k, true))
    } else if let Some(&key) = SHIFT_CHARS.get(&ch) {
        Some((key, true))
    } else {
        let s = ch.to_string();
        KEY_MAP.get(s.as_str()).map(|&k| (k, false))
    }
}

/// Resolve a KeyModifier to an evdev Key code.
pub fn modifier_to_code(modifier: &crate::models::api::KeyModifier) -> Key {
    use crate::models::api::KeyModifier;
    match modifier {
        KeyModifier::Cmd | KeyModifier::Command | KeyModifier::Super => Key::KEY_LEFTMETA,
        KeyModifier::Ctrl | KeyModifier::Control => Key::KEY_LEFTCTRL,
        KeyModifier::Alt | KeyModifier::Option => Key::KEY_LEFTALT,
        KeyModifier::Shift => Key::KEY_LEFTSHIFT,
        KeyModifier::Fn | KeyModifier::Function => Key::KEY_FN,
    }
}
