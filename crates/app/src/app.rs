//! Application state and the glue between keyboard hook, engine, popup and tray.
//!
//! Everything runs on one thread (the one with the message loop), so state lives
//! in thread-locals. Rule: never hold a `RefCell` borrow while calling into
//! Win32 - Win32 may call back into our window procedures.

use std::{
    cell::{Cell, RefCell},
    ptr::{null, null_mut},
};

use bangla_engine::{Engine, KeyInput, Layout, Reply};
use windows_sys::Win32::{
    Foundation::{GetLastError, ERROR_ALREADY_EXISTS},
    System::Threading::CreateMutexW,
    UI::{
        HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
        Input::KeyboardAndMouse::{GetAsyncKeyState, GetKeyState},
        WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage, MSG},
    },
};

use crate::{
    bar, gfx, hook, inject, popup,
    settings::{self, Hotkey, Settings},
    studio, tray,
    util::{message_box, wide},
};

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
    static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default());
    /// Mirror of "is a word being composed", cheap to read from the mouse hook.
    static COMPOSING: Cell<bool> = const { Cell::new(false) };
    /// While one of Jype's own text boxes has focus: Some(true) = always type Bangla,
    /// Some(false) = always plain English, None = follow the normal mode.
    static FORCE_MODE: Cell<Option<bool>> = const { Cell::new(None) };
}

pub fn run() {
    gfx::register_fonts();
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let name = wide("Local\\Jype.SingleInstance");
        let mutex = CreateMutexW(null(), 0, name.as_ptr());
        if !mutex.is_null() && GetLastError() == ERROR_ALREADY_EXISTS {
            message_box(
                "Jype",
                "Jype is already running.\n\nLook for the maroon Jype J icon near the clock (click the ^ arrow if it is hidden).",
                false,
            );
            return;
        }
    }

    settings::migrate_old_data();
    let s = Settings::load();
    let engine = match Engine::new(&settings::data_dir(), s.layout, s.include_english) {
        Ok(e) => e,
        Err(err) => {
            message_box("Jype", &format!("Could not start the Bangla engine:\n\n{err}"), true);
            return;
        }
    };
    let first_run = s.first_run;
    SETTINGS.with(|c| *c.borrow_mut() = s);
    ENGINE.with(|c| *c.borrow_mut() = Some(engine));

    popup::set_click_handler(on_candidate_click);
    if !tray::init(first_run) {
        message_box("Jype", "Could not create the tray icon.", true);
        return;
    }
    if first_run {
        SETTINGS.with(|c| c.borrow().save());
    }
    bar::init();
    hook::install();

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    hook::uninstall();
    tray::remove();
}

// ---------------------------------------------------------------- settings --

pub fn settings() -> Settings {
    SETTINGS.with(|s| s.borrow().clone())
}

fn update_settings(f: impl FnOnce(&mut Settings)) {
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        f(&mut s);
        s.save();
    });
}

pub fn is_composing() -> bool {
    COMPOSING.with(|c| c.get())
}

// ------------------------------------------------------------------ typing --

fn is_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

/// Called from the keyboard hook for every key press. Returns true to swallow it.
pub fn on_key_down(vk: u32) -> bool {
    // Pure modifier / lock keys are never typing; leave them alone.
    if matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0..=0xA5 | 0x5B | 0x5C | 0x14 | 0x90 | 0x91) {
        return false;
    }

    const VK_SHIFT: u16 = 0x10;
    const VK_CONTROL: u16 = 0x11;
    const VK_MENU: u16 = 0x12;
    const VK_RMENU: u16 = 0xA5;
    const VK_LWIN: u16 = 0x5B;
    const VK_RWIN: u16 = 0x5C;
    const VK_CAPITAL: i32 = 0x14;

    let (hotkey, on, phonetic) = SETTINGS.with(|s| {
        let s = s.borrow();
        (s.hotkey, s.bangla_on, s.layout.is_phonetic())
    });
    let on = FORCE_MODE.with(|f| f.get()).unwrap_or(on);

    let shift = is_down(VK_SHIFT);
    // Right Alt acts as AltGr only for the fixed layouts that use it.
    let altgr = is_down(VK_RMENU) && !phonetic;
    let ctrl = is_down(VK_CONTROL) && !altgr;
    let alt = is_down(VK_MENU) && !altgr;
    let win = is_down(VK_LWIN) || is_down(VK_RWIN);

    if hotkey.matches(vk, ctrl, alt, shift) {
        if alt {
            inject::cancel_menu_activation();
        }
        toggle_mode(true);
        return true;
    }
    if !on {
        return false;
    }

    let caps = unsafe { (GetKeyState(VK_CAPITAL) & 1) != 0 };
    let key = KeyInput { vk, shift, caps, ctrl, alt, altgr, win };

    let reply = ENGINE.with(|e| e.borrow_mut().as_mut().map(|e| e.handle_key(key)));
    match reply {
        Some(r) => present(r),
        None => false,
    }
}

/// Act on the engine's answer. Returns true if the key should be swallowed.
fn present(reply: Reply) -> bool {
    match reply {
        Reply::Pass => {
            sync_popup();
            false
        }
        Reply::Consume => {
            sync_popup();
            true
        }
        Reply::Commit { text, then } => {
            popup::hide();
            COMPOSING.with(|c| c.set(false));
            inject::commit(&text, then);
            count_hint();
            true
        }
    }
}

fn sync_popup() {
    let view = ENGINE.with(|e| e.borrow().as_ref().and_then(|e| e.view().cloned()));
    COMPOSING.with(|c| c.set(view.is_some()));
    match view {
        Some(v) => {
            let hint = SETTINGS.with(|s| s.borrow().hints_left > 0);
            popup::show(&v, hint);
        }
        None => popup::hide(),
    }
}

fn count_hint() {
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        if s.hints_left > 0 {
            s.hints_left -= 1;
            if s.hints_left % 5 == 0 {
                s.save();
            }
        }
    });
}

fn on_candidate_click(index: usize) {
    let reply = ENGINE.with(|e| e.borrow_mut().as_mut().map(|e| e.commit_candidate(index)));
    if let Some(r) = reply {
        present(r);
    }
}

/// Drop the word in progress without typing it (focus moved, mouse clicked...).
pub fn reset_composition() {
    ENGINE.with(|e| {
        if let Some(e) = e.borrow_mut().as_mut() {
            e.reset();
        }
    });
    COMPOSING.with(|c| c.set(false));
    popup::hide();
}

// -------------------------------------------------------------- user actions --

/// Switch between Bangla and English typing. `flash` shows a small indicator
/// next to the text cursor.
pub fn toggle_mode(flash: bool) {
    // Don't lose a half-typed word: commit it first.
    let finished = ENGINE.with(|e| e.borrow_mut().as_mut().and_then(|e| e.finish()));
    popup::hide();
    COMPOSING.with(|c| c.set(false));
    if let Some(t) = finished {
        inject::text(&t);
    }
    let mut on = true;
    update_settings(|s| {
        s.bangla_on = !s.bangla_on;
        on = s.bangla_on;
    });
    tray::update_icon(on);
    if flash {
        popup::flash(if on { "\u{09AC}\u{09BE}\u{0982}\u{09B2}\u{09BE}" } else { "English" });
    }
}

pub fn set_layout(layout: Layout) {
    reset_composition();
    let ok = ENGINE.with(|e| {
        e.borrow_mut().as_mut().map(|e| e.set_layout(layout).is_ok()).unwrap_or(false)
    });
    if ok {
        update_settings(|s| s.layout = layout);
    } else {
        message_box("Jype", "Could not switch layout.", true);
    }
}

pub fn set_include_english(include: bool) {
    reset_composition();
    let ok = ENGINE.with(|e| {
        e.borrow_mut().as_mut().map(|e| e.set_include_english(include).is_ok()).unwrap_or(false)
    });
    if ok {
        update_settings(|s| s.include_english = include);
    }
}

pub fn set_hotkey(h: Hotkey) {
    update_settings(|s| s.hotkey = h);
}

pub fn set_bar_visible(visible: bool) {
    update_settings(|s| s.bar_visible = visible);
    bar::set_visible(visible);
}

pub fn save_bar_pos(x: i32, y: i32) {
    update_settings(|s| s.bar_pos = Some((x, y)));
}

/// Change settings from anywhere (Studio, tray menu), save them and repaint.
pub fn change_settings(f: impl FnOnce(&mut Settings)) {
    update_settings(f);
    refresh_ui();
}

/// Repaint everything that shows settings (bar, Studio).
pub fn refresh_ui() {
    bar::refresh();
    studio::refresh();
}

pub fn set_force_mode(mode: Option<bool>) {
    FORCE_MODE.with(|f| f.set(mode));
}

pub fn save_bar_expanded(expanded: bool) {
    update_settings(|s| s.bar_expanded = expanded);
}

// ------------------------------------------------------- personal dictionary --

pub fn dict_words() -> Vec<(String, String)> {
    ENGINE.with(|e| e.borrow().as_ref().map(|e| e.user_words()).unwrap_or_default())
}

pub fn dict_add(english: &str, bangla: &str) -> Result<(), String> {
    ENGINE.with(|e| match e.borrow_mut().as_mut() {
        Some(e) => e.add_user_word(english, bangla),
        None => Err("The engine is not running.".to_string()),
    })
}

pub fn dict_remove(english: &str) -> Result<(), String> {
    ENGINE.with(|e| match e.borrow_mut().as_mut() {
        Some(e) => e.remove_user_word(english),
        None => Err("The engine is not running.".to_string()),
    })
}

// ------------------------------------------------------- learned choices --

pub fn learned_words() -> Vec<(String, Vec<(String, u32)>)> {
    ENGINE.with(|e| e.borrow().as_ref().map(|e| e.learned_words()).unwrap_or_default())
}

pub fn has_learned_words() -> bool {
    ENGINE.with(|e| e.borrow().as_ref().map(|e| e.has_learned_words()).unwrap_or(false))
}

pub fn forget_learned_word(word: &str) -> Result<(), String> {
    ENGINE.with(|e| match e.borrow_mut().as_mut() {
        Some(e) => e.forget_word(word),
        None => Err("The engine is not running.".to_string()),
    })
}

pub fn forget_all_learned() -> Result<(), String> {
    ENGINE.with(|e| match e.borrow_mut().as_mut() {
        Some(e) => e.forget_all(),
        None => Err("The engine is not running.".to_string()),
    })
}

/// Write a copy of the learned-word file into the settings folder for the
/// person to send themselves; returns its path. Jype never sends it anywhere
/// on its own.
pub fn export_learned() -> Result<std::path::PathBuf, String> {
    let dest = settings::data_dir().join("Jype-word-data.txt");
    ENGINE.with(|e| match e.borrow().as_ref() {
        Some(e) => e.export_learned(&dest).map(|_| dest.clone()),
        None => Err("The engine is not running.".to_string()),
    })
}
