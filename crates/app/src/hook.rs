//! Global keyboard / mouse hooks.
//!
//! Hook callbacks must be fast (Windows silently removes a hook that takes too
//! long) and must never unwind into Windows, so every call is wrapped in
//! `catch_unwind`.

use std::{
    cell::Cell,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr::null_mut,
};

use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
        WindowsAndMessaging::*,
    },
};

use crate::{app, inject, popup};

const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;

thread_local! {
    static KB: Cell<HHOOK> = const { Cell::new(null_mut()) };
    static MOUSE: Cell<HHOOK> = const { Cell::new(null_mut()) };
    static EVENT: Cell<HWINEVENTHOOK> = const { Cell::new(null_mut()) };
}

pub fn install() {
    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        KB.with(|h| h.set(SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_proc), hinst, 0)));
        MOUSE.with(|h| h.set(SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hinst, 0)));
        EVENT.with(|h| {
            h.set(SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                null_mut(),
                Some(win_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            ))
        });
    }
}

pub fn uninstall() {
    unsafe {
        let kb = KB.with(|h| h.replace(null_mut()));
        if !kb.is_null() {
            UnhookWindowsHookEx(kb);
        }
        let m = MOUSE.with(|h| h.replace(null_mut()));
        if !m.is_null() {
            UnhookWindowsHookEx(m);
        }
        let e = EVENT.with(|h| h.replace(null_mut()));
        if !e.is_null() {
            UnhookWinEvent(e);
        }
    }
}

unsafe extern "system" fn kb_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lp as *const KBDLLHOOKSTRUCT);
        // Ignore the keys we injected ourselves.
        if kb.dwExtraInfo != inject::MAGIC {
            let msg = wp as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let vk = kb.vkCode;
                let swallow = catch_unwind(AssertUnwindSafe(|| app::on_key_down(vk))).unwrap_or(false);
                if swallow {
                    return 1;
                }
            }
        }
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}

unsafe extern "system" fn mouse_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && app::is_composing() {
        let msg = wp as u32;
        if matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN) {
            let ms = &*(lp as *const MSLLHOOKSTRUCT);
            let inside = popup::screen_rect()
                .map(|r| ms.pt.x >= r.left && ms.pt.x < r.right && ms.pt.y >= r.top && ms.pt.y < r.bottom)
                .unwrap_or(false);
            if !inside {
                let _ = catch_unwind(AssertUnwindSafe(app::reset_composition));
            }
        }
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}

unsafe extern "system" fn win_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    // Focus moved to another window: a half-typed word no longer belongs anywhere.
    if app::is_composing() {
        let _ = catch_unwind(AssertUnwindSafe(app::reset_composition));
    }
}
