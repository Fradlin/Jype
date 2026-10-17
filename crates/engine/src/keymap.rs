//! Translation from Windows virtual-key codes to Riti key codes.
//!
//! The numeric values are the documented, stable Win32 `VK_*` constants, so this
//! module is plain Rust and can be unit-tested on any OS.

use riti::keycodes::*;

pub const VK_BACK: u32 = 0x08;
pub const VK_TAB: u32 = 0x09;
pub const VK_RETURN: u32 = 0x0D;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_SPACE: u32 = 0x20;
pub const VK_PRIOR: u32 = 0x21; // Page Up
pub const VK_NEXT: u32 = 0x22; // Page Down
pub const VK_END: u32 = 0x23;
pub const VK_HOME: u32 = 0x24;
pub const VK_LEFT: u32 = 0x25;
pub const VK_UP: u32 = 0x26;
pub const VK_RIGHT: u32 = 0x27;
pub const VK_DOWN: u32 = 0x28;
pub const VK_DELETE: u32 = 0x2E;

const LETTERS: u32 = 26;

/// Map a physical key (Windows VK code) to the Riti key code.
///
/// `shift` is the real Shift state; `caps` is Caps Lock. Caps Lock only affects
/// letters (it inverts Shift for them), exactly like a normal keyboard.
pub fn vk_to_vc(vk: u32, shift: bool, caps: bool) -> Option<u16> {
    // Letters A-Z
    if (0x41..0x41 + LETTERS).contains(&vk) {
        let offset = (vk - 0x41) as u16;
        let upper = shift ^ caps;
        return Some(if upper { VC_A_SHIFT + offset } else { VC_A + offset });
    }

    // Number row 0-9
    if (0x30..=0x39).contains(&vk) {
        let d = (vk - 0x30) as usize;
        const PLAIN: [u16; 10] = [
            VC_0, VC_1, VC_2, VC_3, VC_4, VC_5, VC_6, VC_7, VC_8, VC_9,
        ];
        const SHIFTED: [u16; 10] = [
            VC_PAREN_RIGHT,
            VC_EXCLAIM,
            VC_AT,
            VC_HASH,
            VC_DOLLAR,
            VC_PERCENT,
            VC_CIRCUM,
            VC_AMPERSAND,
            VC_ASTERISK,
            VC_PAREN_LEFT,
        ];
        return Some(if shift { SHIFTED[d] } else { PLAIN[d] });
    }

    // Numeric keypad
    if (0x60..=0x69).contains(&vk) {
        const KP: [u16; 10] = [
            VC_KP_0, VC_KP_1, VC_KP_2, VC_KP_3, VC_KP_4, VC_KP_5, VC_KP_6, VC_KP_7, VC_KP_8,
            VC_KP_9,
        ];
        return Some(KP[(vk - 0x60) as usize]);
    }
    match vk {
        0x6A => return Some(VC_KP_MULTIPLY),
        0x6B => return Some(VC_KP_ADD),
        0x6D => return Some(VC_KP_SUBTRACT),
        0x6E => return Some(VC_KP_DECIMAL),
        0x6F => return Some(VC_KP_DIVIDE),
        _ => {}
    }

    // US-layout punctuation keys (VK_OEM_*)
    let pair = |plain: u16, shifted: u16| Some(if shift { shifted } else { plain });
    match vk {
        0xBA => pair(VC_SEMICOLON, VC_COLON),       // ; :
        0xBB => pair(VC_EQUALS, VC_PLUS),           // = +
        0xBC => pair(VC_COMMA, VC_LESS),            // , <
        0xBD => pair(VC_MINUS, VC_UNDERSCORE),      // - _
        0xBE => pair(VC_PERIOD, VC_GREATER),        // . >
        0xBF => pair(VC_SLASH, VC_QUESTION),        // / ?
        0xC0 => pair(VC_GRAVE, VC_TILDE),           // ` ~
        0xDB => pair(VC_BRACKET_LEFT, VC_BRACE_LEFT), // [ {
        0xDC => pair(VC_BACK_SLASH, VC_BAR),        // \ |
        0xDD => pair(VC_BRACKET_RIGHT, VC_BRACE_RIGHT), // ] }
        0xDE => pair(VC_APOSTROPHE, VC_QUOTE),      // ' "
        _ => None,
    }
}

/// Reverse helper used by tests and the on-screen hint: Latin char -> (vk, shift).
pub fn char_to_vk(c: char) -> Option<(u32, bool)> {
    match c {
        'a'..='z' => Some((c as u32 - 'a' as u32 + 0x41, false)),
        'A'..='Z' => Some((c as u32 - 'A' as u32 + 0x41, true)),
        '0'..='9' => Some((c as u32 - '0' as u32 + 0x30, false)),
        ')' => Some((0x30, true)),
        '!' => Some((0x31, true)),
        '@' => Some((0x32, true)),
        '#' => Some((0x33, true)),
        '$' => Some((0x34, true)),
        '%' => Some((0x35, true)),
        '^' => Some((0x36, true)),
        '&' => Some((0x37, true)),
        '*' => Some((0x38, true)),
        '(' => Some((0x39, true)),
        ';' => Some((0xBA, false)),
        ':' => Some((0xBA, true)),
        '=' => Some((0xBB, false)),
        '+' => Some((0xBB, true)),
        ',' => Some((0xBC, false)),
        '<' => Some((0xBC, true)),
        '-' => Some((0xBD, false)),
        '_' => Some((0xBD, true)),
        '.' => Some((0xBE, false)),
        '>' => Some((0xBE, true)),
        '/' => Some((0xBF, false)),
        '?' => Some((0xBF, true)),
        '`' => Some((0xC0, false)),
        '~' => Some((0xC0, true)),
        '[' => Some((0xDB, false)),
        '{' => Some((0xDB, true)),
        '\\' => Some((0xDC, false)),
        '|' => Some((0xDC, true)),
        ']' => Some((0xDD, false)),
        '}' => Some((0xDD, true)),
        '\'' => Some((0xDE, false)),
        '"' => Some((0xDE, true)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_ranges_are_contiguous() {
        assert_eq!(VC_Z - VC_A, 25);
        assert_eq!(VC_Z_SHIFT - VC_A_SHIFT, 25);
    }

    #[test]
    fn letters_and_caps() {
        assert_eq!(vk_to_vc(0x41, false, false), Some(VC_A));
        assert_eq!(vk_to_vc(0x41, true, false), Some(VC_A_SHIFT));
        assert_eq!(vk_to_vc(0x41, false, true), Some(VC_A_SHIFT));
        assert_eq!(vk_to_vc(0x41, true, true), Some(VC_A));
        assert_eq!(vk_to_vc(0x5A, false, false), Some(VC_Z));
    }

    #[test]
    fn digits_and_punct() {
        assert_eq!(vk_to_vc(0x31, false, false), Some(VC_1));
        assert_eq!(vk_to_vc(0x31, true, false), Some(VC_EXCLAIM));
        assert_eq!(vk_to_vc(0xBE, false, false), Some(VC_PERIOD));
        assert_eq!(vk_to_vc(0xDE, true, false), Some(VC_QUOTE));
        assert_eq!(vk_to_vc(0x70, false, false), None); // F1
    }

    #[test]
    fn every_char_roundtrips() {
        for c in "abcxyzABCXYZ0123456789!@#$%^&*()-_=+[]{}\\|;:'\",.<>/?`~".chars() {
            let (vk, shift) = char_to_vk(c).unwrap_or_else(|| panic!("no vk for {c}"));
            assert!(vk_to_vc(vk, shift, false).is_some(), "no vc for {c}");
        }
    }
}
