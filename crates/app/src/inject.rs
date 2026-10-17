//! Putting text into the focused application.

use std::mem::{size_of, zeroed};

use bangla_engine::{keymap::*, Then};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC,
};

/// Marks events we generated, so our own keyboard hook ignores them.
pub const MAGIC: usize = 0x4247_5459; // "BGTY"

fn key_event(vk: u16, scan: u16, flags: u32) -> INPUT {
    let mut i: INPUT = unsafe { zeroed() };
    i.r#type = INPUT_KEYBOARD;
    i.Anonymous.ki = KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: MAGIC };
    i
}

fn push_text(out: &mut Vec<INPUT>, s: &str) {
    for unit in s.encode_utf16() {
        out.push(key_event(0, unit, KEYEVENTF_UNICODE));
        out.push(key_event(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
}

fn is_extended(vk: u32) -> bool {
    matches!(
        vk,
        VK_LEFT | VK_UP | VK_RIGHT | VK_DOWN | VK_PRIOR | VK_NEXT | VK_END | VK_HOME | VK_DELETE | 0x2D
    )
}

fn push_key(out: &mut Vec<INPUT>, vk: u32) {
    let scan = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) } as u16;
    let ext = if is_extended(vk) { KEYEVENTF_EXTENDEDKEY } else { 0 };
    out.push(key_event(vk as u16, scan, ext));
    out.push(key_event(vk as u16, scan, ext | KEYEVENTF_KEYUP));
}

fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32);
    }
}

/// Type `text` into the focused app, then do the follow-up.
/// Everything goes in one `SendInput` call so the order is guaranteed.
pub fn commit(text: &str, then: Then) {
    let mut v = Vec::with_capacity(text.len() * 2 + 4);
    push_text(&mut v, text);
    match then {
        Then::Nothing => {}
        Then::Space => push_text(&mut v, " "),
        Then::Replay(vk) => push_key(&mut v, vk),
    }
    send(&v);
}

/// Just the text, no follow-up.
pub fn text(text: &str) {
    commit(text, Then::Nothing);
}

/// After a shortcut that contains Alt (e.g. Ctrl+Alt+B) we swallow the letter, so
/// the app would see "Alt down, Alt up" alone and open its menu bar. Tapping an
/// unassigned virtual key in between tells Windows the Alt press was a combo.
pub fn cancel_menu_activation() {
    const VK_UNASSIGNED: u16 = 0xE8;
    send(&[key_event(VK_UNASSIGNED, 0, 0), key_event(VK_UNASSIGNED, 0, KEYEVENTF_KEYUP)]);
}
