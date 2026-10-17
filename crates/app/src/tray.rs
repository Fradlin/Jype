//! System-tray icon and menu. This is the app's only "window" for settings.

use std::{
    cell::{Cell, RefCell},
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};

use bangla_engine::Layout;
use windows_sys::Win32::{
    Foundation::*,
    System::{
        LibraryLoader::GetModuleHandleW,
        Registry::*,
    },
    UI::{Shell::*, WindowsAndMessaging::*},
};

use crate::{
    app, gfx,
    settings::{self, Hotkey},
    studio::{self, Tab},
    util::{fill_wide, message_box, wide},
};

const CLASS: &str = "JypeTray";
const WM_TRAY: u32 = WM_APP + 1;
const TRAY_UID: u32 = 1;

const ID_TOGGLE: usize = 1;
const ID_LAYOUT: usize = 100; // + index in Layout::ALL
const ID_HOTKEY: usize = 200; // + index in Hotkey::ALL
const ID_ENGLISH: usize = 300;
const ID_AUTOSTART: usize = 301;
const ID_FOLDER: usize = 302;
const ID_GUIDE: usize = 303;
const ID_ABOUT: usize = 304;
const ID_BAR: usize = 305;
const ID_STUDIO: usize = 306;
const ID_WORDS: usize = 307;
const ID_FONT: usize = 400; // + index in gfx::FONTS
const ID_EXIT: usize = 399;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_NAME: &str = "Jype";

thread_local! {
    static HWND_TRAY: Cell<HWND> = const { Cell::new(null_mut()) };
    static ICON: Cell<HICON> = const { Cell::new(null_mut()) };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    static _KEEP: RefCell<()> = const { RefCell::new(()) };
}

pub fn init(first_run: bool) -> bool {
    unsafe {
        let hinst = GetModuleHandleW(null());
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        // A normal (hidden) top-level window so we also receive "TaskbarCreated".
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Jype").as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        if hwnd.is_null() {
            return false;
        }
        HWND_TRAY.with(|h| h.set(hwnd));
        TASKBAR_CREATED.with(|t| t.set(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr())));
        add_icon(app::settings().bangla_on, first_run)
    }
}

pub fn remove() {
    unsafe {
        let mut nid = base_nid();
        Shell_NotifyIconW(NIM_DELETE, &mut nid);
        let icon = ICON.with(|i| i.replace(null_mut()));
        if !icon.is_null() {
            DestroyIcon(icon);
        }
    }
}

unsafe fn base_nid() -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = HWND_TRAY.with(|h| h.get());
    nid.uID = TRAY_UID;
    nid
}

fn tooltip(on: bool) -> String {
    let hk = app::settings().hotkey.label();
    if on {
        format!("Jype: Bangla typing ON ({hk} to switch)")
    } else {
        format!("Jype: English (typing off) ({hk} to switch)")
    }
}

unsafe fn set_icon(on: bool) -> HICON {
    let new = make_icon(on);
    let old = ICON.with(|i| i.replace(new));
    if !old.is_null() {
        // The shell copies the icon on NIM_MODIFY, so the old one can go after.
        DestroyIcon(old);
    }
    new
}

unsafe fn add_icon(on: bool, welcome: bool) -> bool {
    let mut nid = base_nid();
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = set_icon(on);
    fill_wide(&mut nid.szTip, &tooltip(on));
    let ok = Shell_NotifyIconW(NIM_ADD, &mut nid) != 0;
    if ok && welcome {
        let mut n = base_nid();
        n.uFlags = NIF_INFO;
        n.dwInfoFlags = NIIF_INFO;
        fill_wide(&mut n.szInfoTitle, "Jype is ready");
        fill_wide(
            &mut n.szInfo,
            &format!(
                "Click in any text box and type: ami\nPress Space to accept. {} switches Bangla/English.",
                app::settings().hotkey.label()
            ),
        );
        Shell_NotifyIconW(NIM_MODIFY, &mut n);
    }
    ok
}

/// Refresh the tray icon + tooltip after the mode changed.
pub fn update_icon(on: bool) {
    unsafe {
        let mut nid = base_nid();
        nid.uFlags = NIF_ICON | NIF_TIP;
        nid.hIcon = set_icon(on);
        fill_wide(&mut nid.szTip, &tooltip(on));
        Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    }
    crate::bar::refresh();
}

/// The tray icon: the Jype logo (maroon when Bangla is on, grey when off).
/// Both icons are compiled into the exe (see build.rs).
unsafe fn make_icon(on: bool) -> HICON {
    let hinst = GetModuleHandleW(null());
    let size = GetSystemMetrics(SM_CXSMICON).max(16);
    let id: usize = if on { 1 } else { 2 };
    LoadImageW(hinst, id as *const u16, 1, size, size, 0) as HICON // 1 = IMAGE_ICON
}

// ------------------------------------------------------------------- menu --

unsafe fn append(menu: HMENU, id: usize, text: &str, checked: bool) {
    let flags = MF_STRING | if checked { MF_CHECKED } else { 0 };
    AppendMenuW(menu, flags, id, wide(text).as_ptr());
}

/// Open the full menu at the mouse pointer (used by the floating bar too).
pub fn open_menu() {
    open_menu_at_point(None);
}

/// Open the full menu at a given screen position.
pub fn open_menu_at(x: i32, y: i32) {
    open_menu_at_point(Some((x, y)));
}

fn open_menu_at_point(at: Option<(i32, i32)>) {
    let hwnd = HWND_TRAY.with(|h| h.get());
    if !hwnd.is_null() {
        unsafe { show_menu(hwnd, at, MenuKind::Full) };
    }
}

/// Just the layout list (bar button).
pub fn open_layout_menu(x: i32, y: i32) {
    let hwnd = HWND_TRAY.with(|h| h.get());
    if !hwnd.is_null() {
        unsafe { show_menu(hwnd, Some((x, y)), MenuKind::Layouts) };
    }
}

/// Just the font list (bar button).
pub fn open_font_menu(x: i32, y: i32) {
    let hwnd = HWND_TRAY.with(|h| h.get());
    if !hwnd.is_null() {
        unsafe { show_menu(hwnd, Some((x, y)), MenuKind::Fonts) };
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuKind {
    Full,
    Layouts,
    Fonts,
}

unsafe fn fill_layouts(menu: HMENU, current: Layout) {
    for (i, l) in Layout::ALL.iter().enumerate() {
        append(menu, ID_LAYOUT + i, l.label(), *l == current);
    }
}

unsafe fn fill_fonts(menu: HMENU, current: usize) {
    for (i, f) in gfx::FONTS.iter().enumerate() {
        append(menu, ID_FONT + i, f.name, i == current);
    }
}

unsafe fn show_menu(hwnd: HWND, at: Option<(i32, i32)>, kind: MenuKind) {
    let s = app::settings();
    let menu = CreatePopupMenu();

    match kind {
        MenuKind::Layouts => fill_layouts(menu, s.layout),
        MenuKind::Fonts => fill_fonts(menu, gfx::font_index(&s.font)),
        MenuKind::Full => {
            append(menu, ID_TOGGLE, &format!("Bangla typing   ({})", s.hotkey.label()), s.bangla_on);
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            append(menu, ID_STUDIO, "Open Jype Studio", false);
            append(menu, ID_WORDS, "My words (dictionary)", false);
            AppendMenuW(menu, MF_SEPARATOR, 0, null());

            let layouts = CreatePopupMenu();
            fill_layouts(layouts, s.layout);
            AppendMenuW(menu, MF_POPUP | MF_STRING, layouts as usize, wide("Keyboard layout").as_ptr());

            let fonts = CreatePopupMenu();
            fill_fonts(fonts, gfx::font_index(&s.font));
            AppendMenuW(menu, MF_POPUP | MF_STRING, fonts as usize, wide("Bangla font").as_ptr());

            let hotkeys = CreatePopupMenu();
            for (i, h) in Hotkey::ALL.iter().enumerate() {
                append(hotkeys, ID_HOTKEY + i, h.label(), *h == s.hotkey);
            }
            AppendMenuW(menu, MF_POPUP | MF_STRING, hotkeys as usize, wide("Switch shortcut").as_ptr());

            append(menu, ID_BAR, "Show floating bar", s.bar_visible);
            append(menu, ID_ENGLISH, "Also suggest English words", s.include_english);
            append(menu, ID_AUTOSTART, "Start with Windows", autostart_enabled());
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            append(menu, ID_GUIDE, "How to type (quick guide)", false);
            append(menu, ID_FOLDER, "Open settings folder", false);
            append(menu, ID_ABOUT, "About Jype", false);
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            append(menu, ID_EXIT, "Exit", false);
        }
    }

    let mut pt = POINT { x: 0, y: 0 };
    match at {
        Some((x, y)) => {
            pt.x = x;
            pt.y = y;
        }
        None => {
            GetCursorPos(&mut pt);
        }
    }
    // Required so the menu closes when the user clicks elsewhere.
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, null());
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu); // also destroys the submenus

    if cmd > 0 {
        handle_command(cmd as usize);
    }
}

pub fn open_data_folder() {
    unsafe {
        let dir = settings::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        ShellExecuteW(
            null_mut(),
            wide("open").as_ptr(),
            wide(&dir.to_string_lossy()).as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Open Explorer with one file pre-selected (used after exporting word data).
pub fn reveal_file(path: &std::path::Path) {
    unsafe {
        let arg = format!("/select,\"{}\"", path.display());
        ShellExecuteW(null_mut(), wide("open").as_ptr(), wide("explorer.exe").as_ptr(), wide(&arg).as_ptr(), null(), SW_SHOWNORMAL);
    }
}

fn handle_command(cmd: usize) {
    match cmd {
        ID_TOGGLE => app::toggle_mode(false),
        ID_STUDIO => studio::open(Tab::Look),
        ID_WORDS => studio::open(Tab::Dictionary),
        c if (ID_FONT..ID_FONT + gfx::FONTS.len()).contains(&c) => {
            app::change_settings(|st| st.font = gfx::FONTS[c - ID_FONT].id.to_string());
        }
        c if (ID_LAYOUT..ID_LAYOUT + Layout::ALL.len()).contains(&c) => {
            app::set_layout(Layout::ALL[c - ID_LAYOUT]);
        }
        c if (ID_HOTKEY..ID_HOTKEY + Hotkey::ALL.len()).contains(&c) => {
            app::set_hotkey(Hotkey::ALL[c - ID_HOTKEY]);
            update_icon(app::settings().bangla_on); // refresh tooltip
        }
        ID_BAR => app::set_bar_visible(!app::settings().bar_visible),
        ID_ENGLISH => app::set_include_english(!app::settings().include_english),
        ID_AUTOSTART => set_autostart(!autostart_enabled()),
        ID_GUIDE => message_box("Jype - quick guide", GUIDE, false),
        ID_FOLDER => open_data_folder(),
        ID_ABOUT => studio::open(Tab::About),
        ID_EXIT => unsafe { PostQuitMessage(0) },
        _ => {}
    }
}

const GUIDE: &str = "Type Bangla with English letters. A small box shows suggestions:\n\
\n\
    ami        \u{2192}  \u{0986}\u{09AE}\u{09BF}\n\
    tumi       \u{2192}  \u{09A4}\u{09C1}\u{09AE}\u{09BF}\n\
    bangla     \u{2192}  \u{09AC}\u{09BE}\u{0982}\u{09B2}\u{09BE}\n\
    kShoma     \u{2192}  \u{0995}\u{09CD}\u{09B7}\u{09AE}\u{09BE}\n\
\n\
English words work too - type them in English spelling:\n\
    customized  \u{2192}  \u{0995}\u{09BE}\u{09B8}\u{09CD}\u{099F}\u{09AE}\u{09BE}\u{0987}\u{099C}\u{09A1}\n\
    (add your own words: bar > Words, or Studio > Dictionary)\n\
\n\
While a word is in the box:\n\
    Space           accept the highlighted word\n\
    Up / Down / Tab choose another suggestion (or click one)\n\
    Backspace       fix what you typed\n\
    Esc             cancel the word\n\
\n\
Handy letters:\n\
    t th d dh   =  \u{09A4} \u{09A5} \u{09A6} \u{09A7}      T Th D Dh  =  \u{099F} \u{09A0} \u{09A1} \u{09A2}\n\
    n = \u{09A8}   N = \u{09A3}   ng = \u{0999}   NG = \u{099E}\n\
    s = \u{09B8}   sh = \u{09B6}   Sh = \u{09B7}\n\
    R = \u{09DC}   Rh = \u{09DD}   Y = \u{09DF}\n\
    rri = \u{098B}   $ = \u{09F3}\n\
    Capital O = \u{09CB} / \u{0993}  (small o is the silent \u{0985})\n\
\n\
Switch Bangla / English: press the shortcut shown in the tray menu\n\
(default Ctrl+Space), or click the tray icon.\n\
\n\
Tip: keep Windows' own keyboard set to English (US).";

// -------------------------------------------------------------- autostart --

fn exe_path() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.to_string_lossy().into_owned())
}

pub fn autostart_enabled() -> bool {
    unsafe {
        let mut key: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_QUERY_VALUE, &mut key) != 0 {
            return false;
        }
        let rc = RegQueryValueExW(key, wide(RUN_NAME).as_ptr(), null(), null_mut(), null_mut(), null_mut());
        RegCloseKey(key);
        rc == 0
    }
}

pub fn set_autostart(enable: bool) {
    unsafe {
        let mut key: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut key) != 0 {
            message_box("Jype", "Could not change the startup setting.", true);
            return;
        }
        if enable {
            if let Some(exe) = exe_path() {
                let value = wide(&format!("\"{exe}\""));
                RegSetValueExW(
                    key,
                    wide(RUN_NAME).as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr() as *const u8,
                    (value.len() * 2) as u32,
                );
            }
        } else {
            RegDeleteValueW(key, wide(RUN_NAME).as_ptr());
        }
        RegCloseKey(key);
    }
}

// ----------------------------------------------------------------- window --

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_TRAY {
        match (lp as u32) & 0xFFFF {
            WM_LBUTTONUP => app::toggle_mode(false),
            WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd, None, MenuKind::Full),
            _ => {}
        }
        return 0;
    }
    if msg != 0 && msg == TASKBAR_CREATED.with(|t| t.get()) {
        // Explorer restarted: put the icon back.
        add_icon(app::settings().bangla_on, false);
        return 0;
    }
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
