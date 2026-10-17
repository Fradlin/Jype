//! Jype Studio: the settings window (Dictionary, Look, Typing, About).
//!
//! Drawn by hand with the same smooth shapes as the bar and popup, so it looks
//! like one modern app instead of a classic Windows dialog. The only real
//! Windows controls are the two text boxes of the dictionary page.
//!
//! The UI is "immediate mode": `build()` returns a list of widgets (rectangle +
//! kind + action) for the current page. Painting and mouse hit-testing both walk
//! that same list, so they can never disagree.

use std::{
    cell::RefCell,
    ffi::c_void,
    mem::zeroed,
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::DwmSetWindowAttribute, Gdi::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT},
        WindowsAndMessaging::*,
    },
};

use bangla_engine::Layout;

use crate::{
    app,
    gfx::{self, palette, rect, scale, Canvas, Palette, ACCENTS, FONTS},
    settings::Hotkey,
    tray,
    util::{rgb, wide},
};

const CLASS: &str = "JypeStudio";
const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_CTLCOLOREDIT_MSG: u32 = 0x0133;
const EN_SETFOCUS_N: u32 = 0x0100;
const EN_KILLFOCUS_N: u32 = 0x0200;
const EM_SETCUEBANNER_MSG: u32 = 0x1501;
const ID_EDIT_EN: isize = 5001;
const ID_EDIT_BN: isize = 5002;
const ROW_H: i32 = 42;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Dictionary,
    Look,
    Typing,
    About,
}

const TABS: [(Tab, &str, &str); 4] = [
    (Tab::Dictionary, "\u{E82D}", "Dictionary"),
    (Tab::Look, "\u{E790}", "Look"),
    (Tab::Typing, "\u{E765}", "Typing"),
    (Tab::About, "\u{E946}", "About"),
];

#[derive(Clone, PartialEq, Debug)]
enum Act {
    Tab(Tab),
    Font(usize),
    Size(u8),
    Theme(u8),
    Accent(u8),
    Layout(usize),
    Hotkey(usize),
    ToggleEnglish,
    ToggleAutostart,
    ToggleBar,
    AddWord,
    DelWord(String),
    PickWord(String, String),
    OpenFolder,
    ExportLearned,
    ForgetLearned,
}

#[derive(Clone, Debug)]
enum Kind {
    Card,
    Heading,
    Body,
    Muted,
    Choice { selected: bool },
    Toggle { on: bool },
    FontRow { index: usize, selected: bool },
    Swatch { color: (u8, u8, u8), selected: bool },
    DictRow { english: String, bangla: String },
    Primary,
    LinkButton,
    Tab { tab: Tab, selected: bool },
    Hero,
}

#[derive(Clone)]
struct W {
    r: RECT,
    kind: Kind,
    text: String,
    act: Option<Act>,
}

struct Studio {
    hwnd: HWND,
    edit_en: HWND,
    edit_bn: HWND,
    tab: Tab,
    dpi: u32,
    hover: Option<usize>,
    scroll: usize,
    words: Vec<(String, String)>,
    message: String,
    focus_edit: i32, // 0 none, 1 english, 2 bangla
    backdrop: HBRUSH,
    backdrop_for: u32,
    prev_proc: WNDPROC,
    anim: f32, // 0..1 page-change fade
}

thread_local! {
    static STUDIO: RefCell<Studio> = RefCell::new(Studio {
        hwnd: null_mut(),
        edit_en: null_mut(),
        edit_bn: null_mut(),
        tab: Tab::Dictionary,
        dpi: 96,
        hover: None,
        scroll: 0,
        words: Vec::new(),
        message: String::new(),
        focus_edit: 0,
        backdrop: null_mut(),
        backdrop_for: 0,
        prev_proc: None,
        anim: 1.0,
    });
}

// ----------------------------------------------------------------- layout --

struct Frame {
    side_w: i32,
    pad: i32,
}

fn frame(d: u32) -> Frame {
    Frame { side_w: scale(188, d), pad: scale(28, d) }
}

fn win_size(d: u32) -> (i32, i32) {
    (scale(820, d), scale(580, d))
}

fn w(r: RECT, kind: Kind, text: &str, act: Option<Act>) -> W {
    W { r, kind, text: text.to_string(), act }
}

/// Everything that is drawn / clickable on the current page.
fn build(st: &Studio, client_w: i32, client_h: i32) -> Vec<W> {
    let d = st.dpi;
    let f = frame(d);
    let s = app::settings();
    let mut v: Vec<W> = Vec::new();

    // sidebar tabs
    let mut y = scale(96, d);
    for (tab, _, name) in TABS {
        let r = rect(scale(12, d), y, f.side_w - scale(12, d), y + scale(42, d));
        v.push(w(r, Kind::Tab { tab, selected: st.tab == tab }, name, Some(Act::Tab(tab))));
        y += scale(46, d);
    }

    let cx = f.side_w + f.pad;
    let cr = client_w - f.pad;
    let cw = cr - cx;
    let mut y = f.pad;

    match st.tab {
        Tab::Dictionary => {
            v.push(w(rect(cx, y, cr, y + scale(34, d)), Kind::Heading, "My words", None));
            y += scale(36, d);
            v.push(w(
                rect(cx, y, cr, y + scale(22, d)),
                Kind::Muted,
                "Teach Jype how you spell English words in Bangla. Your words always come first.",
                None,
            ));
            y += scale(36, d);
            // input card (the two edit boxes sit inside it, see place_edits)
            let card = rect(cx, y, cr, y + scale(64, d));
            v.push(w(card, Kind::Card, "", None));
            let btn_w = scale(96, d);
            let btn = rect(cr - scale(10, d) - btn_w, y + scale(12, d), cr - scale(10, d), y + scale(52, d));
            v.push(w(btn, Kind::Primary, "Add word", Some(Act::AddWord)));
            y += scale(64, d) + scale(10, d);
            if !st.message.is_empty() {
                v.push(w(rect(cx, y, cr, y + scale(22, d)), Kind::Muted, &st.message, None));
            }
            y += scale(26, d);

            // learned choices: on-device only, nothing leaves the PC unless exported
            if app::has_learned_words() {
                let row = rect(cx, y, cr, y + scale(30, d));
                v.push(w(row, Kind::Muted, "Jype has learned some of your picks on this PC.", None));
                let bw = scale(132, d);
                let r2 = rect(cr - bw, y - scale(2, d), cr, y + scale(30, d));
                v.push(w(r2, Kind::LinkButton, "Export word data", Some(Act::ExportLearned)));
                let r1 = rect(cr - bw - scale(118, d) - scale(8, d), y - scale(2, d), cr - bw - scale(8, d), y + scale(30, d));
                v.push(w(r1, Kind::LinkButton, "Reset learned", Some(Act::ForgetLearned)));
                y += scale(38, d);
            }

            // the word list
            let list_top = y;
            let list_bottom = client_h - f.pad;
            let rows_fit = (((list_bottom - list_top) / scale(ROW_H, d)) as usize).max(1);
            v.push(w(rect(cx, list_top, cr, list_bottom), Kind::Card, "", None));
            if st.words.is_empty() {
                v.push(w(
                    rect(cx + scale(18, d), list_top + scale(16, d), cr - scale(18, d), list_top + scale(48, d)),
                    Kind::Muted,
                    "Nothing here yet. Add a word above, for example: bkash \u{2192} \u{09AC}\u{09BF}\u{0995}\u{09BE}\u{09B6}",
                    None,
                ));
            }
            let start = st.scroll.min(st.words.len().saturating_sub(rows_fit));
            for (i, (en, bn)) in st.words.iter().enumerate().skip(start).take(rows_fit) {
                let ry = list_top + scale(4, d) + (i - start) as i32 * scale(ROW_H, d);
                let r = rect(cx + scale(6, d), ry, cr - scale(6, d), ry + scale(ROW_H, d));
                v.push(w(r, Kind::DictRow { english: en.clone(), bangla: bn.clone() }, "", Some(Act::PickWord(en.clone(), bn.clone()))));
                let x = rect(r.right - scale(40, d), r.top + scale(5, d), r.right - scale(6, d), r.bottom - scale(5, d));
                v.push(w(x, Kind::Muted, "\u{E711}", Some(Act::DelWord(en.clone()))));
            }
        }
        Tab::Look => {
            v.push(w(rect(cx, y, cr, y + scale(34, d)), Kind::Heading, "Look", None));
            y += scale(46, d);
            v.push(w(rect(cx, y, cr, y + scale(22, d)), Kind::Body, "Bangla font", None));
            y += scale(28, d);
            // font list in two columns
            let col_w = (cw - scale(10, d)) / 2;
            let rh = scale(54, d);
            let current = gfx::font_index(&s.font);
            for (i, _) in FONTS.iter().enumerate() {
                let col = (i % 2) as i32;
                let row = (i / 2) as i32;
                let x0 = cx + col * (col_w + scale(10, d));
                let y0 = y + row * (rh + scale(8, d));
                v.push(w(
                    rect(x0, y0, x0 + col_w, y0 + rh),
                    Kind::FontRow { index: i, selected: i == current },
                    FONTS[i].name,
                    Some(Act::Font(i)),
                ));
            }
            y += ((FONTS.len() as i32 + 1) / 2) * (rh + scale(8, d)) + scale(10, d);

            // size + theme on one line each
            v.push(w(rect(cx, y, cx + scale(110, d), y + scale(36, d)), Kind::Body, "Text size", None));
            let mut x = cx + scale(120, d);
            for (i, name) in ["Small", "Medium", "Large"].iter().enumerate() {
                let r = rect(x, y, x + scale(92, d), y + scale(36, d));
                v.push(w(r, Kind::Choice { selected: s.font_size as usize == i }, name, Some(Act::Size(i as u8))));
                x += scale(100, d);
            }
            y += scale(48, d);
            v.push(w(rect(cx, y, cx + scale(110, d), y + scale(36, d)), Kind::Body, "Theme", None));
            let mut x = cx + scale(120, d);
            for (i, name) in ["Auto", "Light", "Dark"].iter().enumerate() {
                let r = rect(x, y, x + scale(92, d), y + scale(36, d));
                v.push(w(r, Kind::Choice { selected: s.theme as usize == i }, name, Some(Act::Theme(i as u8))));
                x += scale(100, d);
            }
            y += scale(48, d);
            v.push(w(rect(cx, y, cx + scale(110, d), y + scale(36, d)), Kind::Body, "Accent", None));
            let mut x = cx + scale(120, d);
            for (i, (name, c)) in ACCENTS.iter().enumerate() {
                let r = rect(x, y, x + scale(36, d), y + scale(36, d));
                v.push(w(r, Kind::Swatch { color: *c, selected: s.accent as usize == i }, name, Some(Act::Accent(i as u8))));
                x += scale(46, d);
            }
        }
        Tab::Typing => {
            v.push(w(rect(cx, y, cr, y + scale(34, d)), Kind::Heading, "Typing", None));
            y += scale(46, d);
            v.push(w(rect(cx, y, cr, y + scale(22, d)), Kind::Body, "Keyboard layout", None));
            y += scale(28, d);
            let col_w = (cw - scale(10, d)) / 2;
            for (i, l) in Layout::ALL.iter().enumerate() {
                let col = (i % 2) as i32;
                let row = (i / 2) as i32;
                let x0 = cx + col * (col_w + scale(10, d));
                let y0 = y + row * scale(46, d);
                v.push(w(
                    rect(x0, y0, x0 + col_w, y0 + scale(38, d)),
                    Kind::Choice { selected: *l == s.layout },
                    l.label(),
                    Some(Act::Layout(i)),
                ));
            }
            y += ((Layout::ALL.len() as i32 + 1) / 2) * scale(46, d) + scale(12, d);
            v.push(w(rect(cx, y, cr, y + scale(22, d)), Kind::Body, "Switch shortcut", None));
            y += scale(28, d);
            let hk_gap = scale(10, d);
            let hk_w = (cw - hk_gap * (Hotkey::ALL.len() as i32 - 1)) / Hotkey::ALL.len() as i32;
            let mut x = cx;
            for (i, h) in Hotkey::ALL.iter().enumerate() {
                let r = rect(x, y, x + hk_w, y + scale(36, d));
                v.push(w(r, Kind::Choice { selected: *h == s.hotkey }, h.label(), Some(Act::Hotkey(i))));
                x += hk_w + hk_gap;
            }
            y += scale(48, d);
            for (label, on, act) in [
                ("Also suggest English words", s.include_english, Act::ToggleEnglish),
                ("Show the floating Jype bar", s.bar_visible, Act::ToggleBar),
                ("Start Jype with Windows", tray::autostart_enabled(), Act::ToggleAutostart),
            ] {
                v.push(w(rect(cx, y, cr, y + scale(40, d)), Kind::Toggle { on }, label, Some(act)));
                y += scale(46, d);
            }
        }
        Tab::About => {
            v.push(w(rect(cx, y, cr, y + scale(190, d)), Kind::Hero, "", None));
            y += scale(210, d);
            v.push(w(rect(cx, y, cr, y + scale(26, d)), Kind::Body, "For Jim by Frad", None));
            y += scale(30, d);
            v.push(w(rect(cx, y, cr, y + scale(24, d)), Kind::Muted, "Author: @fradlin   \u{00B7}   Publisher: Frad", None));
            y += scale(40, d);
            v.push(w(
                rect(cx, y, cr, y + scale(24, d)),
                Kind::Muted,
                "Phonetic engine: Riti from OpenBangla Keyboard (MPL-2.0).",
                None,
            ));
            y += scale(26, d);
            v.push(w(
                rect(cx, y, cr, y + scale(24, d)),
                Kind::Muted,
                "English readings: CMU Pronouncing Dictionary. Jype never uses the internet.",
                None,
            ));
            y += scale(44, d);
            v.push(w(rect(cx, y, cx + scale(190, d), y + scale(40, d)), Kind::Primary, "Open settings folder", Some(Act::OpenFolder)));
        }
    }
    v
}

// ------------------------------------------------------------------ window --

/// Open (or bring forward) the Studio on the given page.
pub fn open(tab: Tab) {
    unsafe {
        let existing = STUDIO.with(|s| {
            let mut s = s.borrow_mut();
            s.tab = tab;
            s.hwnd
        });
        if !existing.is_null() {
            refresh_words();
            update_edit_visibility();
            ShowWindow(existing, SW_SHOWNORMAL);
            SetForegroundWindow(existing);
            InvalidateRect(existing, null(), 0);
            return;
        }
        create(tab);
    }
}

unsafe fn create(tab: Tab) {
    let hinst = GetModuleHandleW(null());
    let class = wide(CLASS);
    let icon = LoadIconW(hinst, 1usize as *const u16);
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: icon,
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class.as_ptr(),
    };
    RegisterClassW(&wc);

    // size from the DPI of the monitor under the mouse
    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut dpi = 96u32;
    {
        // GetDpiForMonitor lives in another feature set; the system DPI is close enough
        // for a first size, WM_DPICHANGED fixes it up.
        let screen = GetDC(null_mut());
        let sd = GetDeviceCaps(screen, LOGPIXELSX as i32);
        ReleaseDC(null_mut(), screen);
        if sd > 0 {
            dpi = sd as u32;
        }
        let _ = mon;
    }
    let (cw, ch) = win_size(dpi);
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
    let mut wr = RECT { left: 0, top: 0, right: cw, bottom: ch };
    AdjustWindowRect(&mut wr, style, 0);
    let (ww, wh) = (wr.right - wr.left, wr.bottom - wr.top);
    let sx = GetSystemMetrics(SM_CXSCREEN);
    let sy = GetSystemMetrics(SM_CYSCREEN);
    let hwnd = CreateWindowExW(
        0,
        class.as_ptr(),
        wide("Jype Studio").as_ptr(),
        style,
        ((sx - ww) / 2).max(0),
        ((sy - wh) / 2).max(0),
        ww,
        wh,
        null_mut(),
        null_mut(),
        hinst,
        null(),
    );
    if hwnd.is_null() {
        return;
    }
    let real = GetDpiForWindow(hwnd);
    STUDIO.with(|s| {
        let mut s = s.borrow_mut();
        s.hwnd = hwnd;
        s.tab = tab;
        s.dpi = if real == 0 { dpi } else { real };
    });
    // edit boxes
    let edit_class = wide("EDIT");
    let mk = |id: isize| {
        CreateWindowExW(
            0,
            edit_class.as_ptr(),
            wide("").as_ptr(),
            WS_CHILD | WS_VISIBLE | (ES_AUTOHSCROLL as u32),
            0,
            0,
            10,
            10,
            hwnd,
            id as HMENU,
            hinst,
            null(),
        )
    };
    let en = mk(ID_EDIT_EN);
    let bn = mk(ID_EDIT_BN);
    STUDIO.with(|s| {
        let mut s = s.borrow_mut();
        s.edit_en = en;
        s.edit_bn = bn;
    });
    SendMessageW(en, EM_SETCUEBANNER_MSG, 1, wide("English word").as_ptr() as LPARAM);
    SendMessageW(bn, EM_SETCUEBANNER_MSG, 1, wide("Bangla spelling").as_ptr() as LPARAM);
    // Enter / Tab handling in the edit boxes
    let prev = SetWindowLongPtrW(en, GWLP_WNDPROC, edit_proc as usize as isize);
    SetWindowLongPtrW(bn, GWLP_WNDPROC, edit_proc as usize as isize);
    STUDIO.with(|s| {
        s.borrow_mut().prev_proc = std::mem::transmute::<isize, WNDPROC>(prev);
    });

    apply_theme_to_frame(hwnd);
    refresh_words();
    relayout(hwnd);
    ShowWindow(hwnd, SW_SHOWNORMAL);
    SetForegroundWindow(hwnd);
}

/// Match the title bar to the app colours (Windows 11; ignored by Windows 10).
unsafe fn apply_theme_to_frame(hwnd: HWND) {
    let pal = palette(&app::settings());
    let dark: i32 = pal.dark as i32;
    DwmSetWindowAttribute(hwnd, 20, &dark as *const i32 as *const c_void, 4);
    DwmSetWindowAttribute(hwnd, 35, &pal.bg as *const u32 as *const c_void, 4);
    DwmSetWindowAttribute(hwnd, 36, &pal.text as *const u32 as *const c_void, 4);
}

fn refresh_words() {
    let words = app::dict_words();
    STUDIO.with(|s| {
        let mut s = s.borrow_mut();
        s.words = words;
    });
}

fn client_size(hwnd: HWND) -> (i32, i32) {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(hwnd, &mut r) };
    (r.right, r.bottom)
}

fn update_edit_visibility() {
    let (hwnd, en, bn, tab) = STUDIO.with(|s| {
        let s = s.borrow();
        (s.hwnd, s.edit_en, s.edit_bn, s.tab)
    });
    unsafe {
        let show = if tab == Tab::Dictionary { SW_SHOW } else { SW_HIDE };
        ShowWindow(en, show);
        ShowWindow(bn, show);
    }
    relayout(hwnd);
}

/// Rectangles of the two text fields (the edit boxes) inside the input card.
fn input_rects(d: u32, card: RECT, add: RECT) -> (RECT, RECT) {
    let pad = scale(16, d);
    // Gap between the two field pills: wide enough that their backgrounds
    // (each padded by `bg_pad` in the drawing code below) never touch.
    let gap = scale(22, d);
    let total = add.left - scale(12, d) - card.left - pad * 2 - gap;
    let half = total / 2;
    let fh = scale(26, d);
    let y = card.top + (card.bottom - card.top - fh) / 2;
    let a = rect(card.left + pad, y, card.left + pad + half, y + fh);
    let b = rect(card.left + pad + half + gap, y, card.left + pad + half * 2 + gap, y + fh);
    (a, b)
}

/// Position the two edit boxes inside the input card and give them fonts.
fn relayout(hwnd: HWND) {
    let (cw, ch) = client_size(hwnd);
    let (en, bn, dpi, tab, widgets) = STUDIO.with(|s| {
        let s = s.borrow();
        let widgets = if s.tab == Tab::Dictionary { build(&s, cw, ch) } else { Vec::new() };
        (s.edit_en, s.edit_bn, s.dpi, s.tab, widgets)
    });
    if tab != Tab::Dictionary || en.is_null() {
        return;
    }
    let settings = app::settings();
    unsafe {
        // the first Card is the input card; the Add button is the Primary widget
        let card = widgets.iter().find(|x| matches!(x.kind, Kind::Card)).map(|x| x.r);
        let add = widgets.iter().find(|x| matches!(x.kind, Kind::Primary)).map(|x| x.r);
        if let (Some(card), Some(add)) = (card, add) {
            let (a, b) = input_rects(dpi, card, add);
            MoveWindow(en, a.left, a.top, a.right - a.left, a.bottom - a.top, 1);
            MoveWindow(bn, b.left, b.top, b.right - b.left, b.bottom - b.top, 1);
        }
        let fe = gfx::ui_font(scale(16, dpi), false);
        let fb = gfx::bangla_font(&settings, scale(20, dpi), false);
        SendMessageW(en, WM_SETFONT, fe as WPARAM, 1);
        SendMessageW(bn, WM_SETFONT, fb as WPARAM, 1);
        // (fonts leak a few handles per relayout; Studio is opened rarely)
        InvalidateRect(hwnd, null(), 0);
    }
}

// Subclass for the edit boxes: Enter / Tab move on or add the word.
unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if (msg == WM_KEYDOWN || msg == WM_CHAR) && (wp == 0x0D || wp == 0x09) {
        if msg == WM_KEYDOWN {
            let (en, bn) = STUDIO.with(|s| {
                let s = s.borrow();
                (s.edit_en, s.edit_bn)
            });
            if hwnd == en {
                SetFocus(bn);
            } else if wp == 0x0D {
                add_word_from_edits();
            } else {
                SetFocus(en);
            }
        }
        return 0;
    }
    let prev = STUDIO.with(|s| s.borrow().prev_proc);
    CallWindowProcW(prev, hwnd, msg, wp, lp)
}

unsafe fn edit_text(h: HWND) -> String {
    let n = GetWindowTextLengthW(h);
    if n <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; n as usize + 1];
    let got = GetWindowTextW(h, buf.as_mut_ptr(), n + 1);
    String::from_utf16_lossy(&buf[..got.max(0) as usize])
}

unsafe fn add_word_from_edits() {
    let (hwnd, en, bn) = STUDIO.with(|s| {
        let s = s.borrow();
        (s.hwnd, s.edit_en, s.edit_bn)
    });
    let (e, b) = (edit_text(en), edit_text(bn));
    let result = app::dict_add(&e, &b);
    match result {
        Ok(()) => {
            SetWindowTextW(en, wide("").as_ptr());
            SetWindowTextW(bn, wide("").as_ptr());
            SetFocus(en);
            refresh_words();
            STUDIO.with(|s| {
                let mut s = s.borrow_mut();
                s.message = format!("Saved: {} \u{2192} {}", e.trim().to_lowercase(), b.trim());
                s.scroll = 0;
            });
        }
        Err(msg) => STUDIO.with(|s| s.borrow_mut().message = msg),
    }
    InvalidateRect(hwnd, null(), 0);
}

fn run(act: Act) {
    let hwnd = STUDIO.with(|s| s.borrow().hwnd);
    unsafe {
        match act {
            Act::Tab(t) => {
                STUDIO.with(|s| {
                    let mut s = s.borrow_mut();
                    s.tab = t;
                    s.message.clear();
                    s.anim = 0.0;
                });
                update_edit_visibility();
                SetTimer(hwnd, 1, 15, None);
            }
            Act::Font(i) => {
                app::change_settings(|s| s.font = FONTS[i].id.to_string());
                relayout(hwnd);
            }
            Act::Size(i) => app::change_settings(|s| s.font_size = i),
            Act::Theme(i) => {
                app::change_settings(|s| s.theme = i);
                apply_theme_to_frame(hwnd);
            }
            Act::Accent(i) => app::change_settings(|s| s.accent = i),
            Act::Layout(i) => app::set_layout(Layout::ALL[i]),
            Act::Hotkey(i) => {
                app::set_hotkey(Hotkey::ALL[i]);
                tray::update_icon(app::settings().bangla_on);
            }
            Act::ToggleEnglish => app::set_include_english(!app::settings().include_english),
            Act::ToggleAutostart => tray::set_autostart(!tray::autostart_enabled()),
            Act::ToggleBar => app::set_bar_visible(!app::settings().bar_visible),
            Act::AddWord => add_word_from_edits(),
            Act::DelWord(e) => {
                let _ = app::dict_remove(&e);
                refresh_words();
            }
            Act::PickWord(e, b) => {
                let (en, bn) = STUDIO.with(|s| {
                    let s = s.borrow();
                    (s.edit_en, s.edit_bn)
                });
                SetWindowTextW(en, wide(&e).as_ptr());
                SetWindowTextW(bn, wide(&b).as_ptr());
                SetFocus(bn);
            }
            Act::OpenFolder => tray::open_data_folder(),
            Act::ExportLearned => {
                STUDIO.with(|s| {
                    let mut s = s.borrow_mut();
                    s.message = match app::export_learned() {
                        Ok(path) => {
                            tray::reveal_file(&path);
                            format!("Saved: {}", path.display())
                        }
                        Err(e) => e,
                    };
                });
            }
            Act::ForgetLearned => {
                let _ = app::forget_all_learned();
                STUDIO.with(|s| s.borrow_mut().message = "Learned choices cleared.".to_string());
            }
        }
        app::refresh_ui();
        InvalidateRect(hwnd, null(), 0);
    }
}

/// Repaint if the window is open (called after settings change elsewhere).
pub fn refresh() {
    let hwnd = STUDIO.with(|s| s.borrow().hwnd);
    if !hwnd.is_null() {
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint(hwnd, hdc);
            EndPaint(hwnd, &ps);
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = (lp as u16 as i16 as i32, ((lp >> 16) & 0xFFFF) as u16 as i16 as i32);
            let changed = STUDIO.with(|s| {
                let Ok(mut s) = s.try_borrow_mut() else { return false };
                let (cw, ch) = client_size(hwnd);
                let widgets = build(&s, cw, ch);
                let h = widgets.iter().rposition(|w| {
                    w.act.is_some() && x >= w.r.left && x < w.r.right && y >= w.r.top && y < w.r.bottom
                });
                let changed = h != s.hover;
                s.hover = h;
                changed
            });
            if changed {
                let mut t = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut t);
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_MOUSELEAVE => {
            STUDIO.with(|s| {
                if let Ok(mut s) = s.try_borrow_mut() {
                    s.hover = None;
                }
            });
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = (lp as u16 as i16 as i32, ((lp >> 16) & 0xFFFF) as u16 as i16 as i32);
            let act = STUDIO.with(|s| {
                let s = s.borrow();
                let (cw, ch) = client_size(hwnd);
                build(&s, cw, ch)
                    .into_iter()
                    .rev()
                    .find(|w| w.act.is_some() && x >= w.r.left && x < w.r.right && y >= w.r.top && y < w.r.bottom)
                    .and_then(|w| w.act)
            });
            if let Some(a) = act {
                run(a);
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wp >> 16) & 0xFFFF) as u16 as i16 as i32;
            STUDIO.with(|s| {
                let mut s = s.borrow_mut();
                if s.tab == Tab::Dictionary {
                    if delta > 0 {
                        s.scroll = s.scroll.saturating_sub(2);
                    } else {
                        s.scroll = (s.scroll + 2).min(s.words.len().saturating_sub(1));
                    }
                }
            });
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_TIMER => {
            let done = STUDIO.with(|s| {
                let mut s = s.borrow_mut();
                s.anim = (s.anim + 0.14).min(1.0);
                s.anim >= 1.0
            });
            if done {
                KillTimer(hwnd, 1);
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as isize;
            let code = ((wp >> 16) & 0xFFFF) as u32;
            if id == ID_EDIT_EN || id == ID_EDIT_BN {
                if code == EN_SETFOCUS_N {
                    app::set_force_mode(Some(id == ID_EDIT_BN));
                    STUDIO.with(|s| s.borrow_mut().focus_edit = if id == ID_EDIT_EN { 1 } else { 2 });
                    InvalidateRect(hwnd, null(), 0);
                } else if code == EN_KILLFOCUS_N {
                    app::set_force_mode(None);
                    STUDIO.with(|s| s.borrow_mut().focus_edit = 0);
                    InvalidateRect(hwnd, null(), 0);
                }
            }
            0
        }
        WM_CTLCOLOREDIT_MSG => {
            let pal = palette(&app::settings());
            let hdc = wp as HDC;
            SetTextColor(hdc, pal.text);
            SetBkColor(hdc, pal.surface2);
            let brush = STUDIO.with(|s| {
                let mut s = s.borrow_mut();
                let key = pal.surface2 | 0x0100_0000;
                if s.backdrop.is_null() || s.backdrop_for != key {
                    if !s.backdrop.is_null() {
                        DeleteObject(s.backdrop);
                    }
                    s.backdrop = CreateSolidBrush(pal.surface2);
                    s.backdrop_for = key;
                }
                s.backdrop
            });
            brush as LRESULT
        }
        WM_DPICHANGED => {
            let new_dpi = (wp & 0xFFFF) as u32;
            let r = &*(lp as *const RECT);
            STUDIO.with(|s| s.borrow_mut().dpi = new_dpi);
            SetWindowPos(hwnd, null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
            relayout(hwnd);
            0
        }
        WM_SIZE => {
            relayout(hwnd);
            0
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            apply_theme_to_frame(hwnd);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_CLOSE => {
            // keep the window object, just hide it (cheap to re-open)
            app::set_force_mode(None);
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ------------------------------------------------------------------ paint --

unsafe fn paint(hwnd: HWND, screen_dc: HDC) {
    let (cw, ch) = client_size(hwnd);
    let Some(mut cv) = Canvas::new(screen_dc, cw, ch) else { return };
    let settings = app::settings();
    let pal = palette(&settings);
    STUDIO.with(|s| {
        let Ok(s) = s.try_borrow() else { return };
        draw(&mut cv, &s, &settings, &pal, cw, ch);
    });
    cv.present(screen_dc);
}

fn lerp_color(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, sh: u32| ((c >> sh) & 0xFF) as f32;
    let m = |sh: u32| (ch(a, sh) * (1.0 - t) + ch(b, sh) * t).round() as u32;
    m(0) | (m(8) << 8) | (m(16) << 16)
}

unsafe fn draw(cv: &mut Canvas, st: &Studio, s: &crate::settings::Settings, pal: &Palette, cw: i32, ch: i32) {
    let d = st.dpi;
    let f = frame(d);
    cv.clear(pal.bg);

    // sidebar
    cv.fill_rrect(scale(8, d) as f32, scale(8, d) as f32, (f.side_w - scale(8, d)) as f32, (ch - scale(16, d)) as f32, scale(14, d) as f32, pal.surface);
    let logo = scale(34, d) as f32;
    cv.logo(scale(24, d) as f32, scale(26, d) as f32, logo, pal.accent, 1.0);
    let title = gfx::ui_font(scale(20, d), true);
    cv.text(title, "Jype", rect(scale(24, d) + logo as i32 + scale(10, d), scale(22, d), f.side_w, scale(50, d)), pal.text, DT_LEFT | DT_VCENTER);
    let small = gfx::ui_font(scale(12, d), false);
    cv.text(small, "Studio", rect(scale(24, d) + logo as i32 + scale(10, d), scale(46, d), f.side_w, scale(64, d)), pal.muted, DT_LEFT | DT_TOP);

    let body = gfx::ui_font(scale(15, d), false);
    let body_b = gfx::ui_font(scale(15, d), true);
    let head = gfx::ui_font(scale(26, d), true);
    let icons = gfx::icon_font(scale(16, d));
    let bn = gfx::bangla_font(s, scale(gfx::size_px(s), d), false);
    let arrow = gfx::ui_font(scale(15, d), false);

    let page_fade = gfx::ease_out(st.anim);
    let widgets = build(st, cw, ch);
    for (i, wd) in widgets.iter().enumerate() {
        let r = wd.r;
        let (x, y, wd_w, ht) = (r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32);
        let hovered = st.hover == Some(i);
        let text_col = |c: u32| lerp_color(pal.bg, c, if matches!(wd.kind, Kind::Tab { .. }) { 1.0 } else { page_fade });
        match &wd.kind {
            Kind::Card => {
                cv.fill_rrect(x, y, wd_w, ht, scale(12, d) as f32, pal.surface);
                cv.shape(x + 0.5, y + 0.5, wd_w - 1.0, ht - 1.0, scale(12, d) as f32, 1.0, pal.border, 1.0);
                // field backgrounds behind the two edit boxes (the first Card on this page)
                if st.tab == Tab::Dictionary && ht < scale(80, d) as f32 {
                    if let Some(add) = widgets.iter().find(|x| matches!(x.kind, Kind::Primary)).map(|x| x.r) {
                        let (a, b) = input_rects(d, r, add);
                        for (i, fr) in [a, b].iter().enumerate() {
                            let (px, py) = (scale(6, d), scale(6, d));
                            let on = st.focus_edit == (i as i32 + 1);
                            cv.fill_rrect((fr.left - px) as f32, (fr.top - py) as f32, (fr.right - fr.left + px * 2) as f32, (fr.bottom - fr.top + py * 2) as f32, scale(9, d) as f32, pal.surface2);
                            if on {
                                cv.shape((fr.left - px) as f32 + 0.5, (fr.top - py) as f32 + 0.5, (fr.right - fr.left + px * 2) as f32 - 1.0, (fr.bottom - fr.top + py * 2) as f32 - 1.0, scale(9, d) as f32, 2.0, pal.accent, 1.0);
                            }
                        }
                    }
                }
            }
            Kind::Heading => cv.text(head, &wd.text, r, text_col(pal.text), DT_LEFT | DT_VCENTER),
            Kind::Body => cv.text(body_b, &wd.text, r, text_col(pal.text), DT_LEFT | DT_VCENTER),
            Kind::Muted => {
                if wd.text == "\u{E711}" {
                    // delete "x" button in a dictionary row
                    if hovered {
                        cv.fill_rrect(x, y, wd_w, ht, scale(8, d) as f32, pal.accent_soft);
                    }
                    cv.text(icons, &wd.text, r, if hovered { pal.accent } else { pal.muted }, DT_CENTER | DT_VCENTER);
                } else {
                    cv.text(body, &wd.text, r, text_col(pal.muted), DT_LEFT | DT_VCENTER);
                }
            }
            Kind::Tab { tab, selected } => {
                if *selected {
                    cv.fill_rrect(x, y, wd_w, ht, scale(10, d) as f32, pal.accent_soft);
                    cv.fill_rrect(x, y + ht * 0.25, scale(4, d) as f32, ht * 0.5, scale(2, d) as f32, pal.accent);
                } else if hovered {
                    cv.fill_rrect(x, y, wd_w, ht, scale(10, d) as f32, pal.surface2);
                }
                let glyph = TABS.iter().find(|t| t.0 == *tab).map(|t| t.1).unwrap_or("");
                let col = if *selected { pal.accent } else { pal.text };
                cv.text(icons, glyph, rect(r.left + scale(10, d), r.top, r.left + scale(40, d), r.bottom), col, DT_CENTER | DT_VCENTER);
                cv.text(body_b, &wd.text, rect(r.left + scale(46, d), r.top, r.right, r.bottom), col, DT_LEFT | DT_VCENTER);
            }
            Kind::Choice { selected } => {
                let bg = if *selected { pal.accent } else if hovered { pal.surface2 } else { pal.surface };
                cv.fill_rrect(x, y, wd_w, ht, scale(10, d) as f32, bg);
                if !*selected {
                    cv.shape(x + 0.5, y + 0.5, wd_w - 1.0, ht - 1.0, scale(10, d) as f32, 1.0, pal.border, 1.0);
                }
                let col = if *selected { pal.accent_text } else { pal.text };
                cv.text(body, &wd.text, rect(r.left + scale(12, d), r.top, r.right - scale(8, d), r.bottom), text_col(col), DT_LEFT | DT_VCENTER | DT_END_ELLIPSIS);
            }
            Kind::Toggle { on } => {
                cv.text(body, &wd.text, rect(r.left, r.top, r.right - scale(70, d), r.bottom), text_col(pal.text), DT_LEFT | DT_VCENTER);
                let (tw, th) = (scale(46, d) as f32, scale(26, d) as f32);
                let tx = r.right as f32 - tw - scale(8, d) as f32;
                let ty = y + (ht - th) / 2.0;
                cv.fill_rrect(tx, ty, tw, th, th / 2.0, if *on { pal.accent } else { pal.border });
                let knob = th - 6.0;
                let kx = if *on { tx + tw - knob - 3.0 } else { tx + 3.0 };
                cv.circle(kx + knob / 2.0, ty + th / 2.0, knob / 2.0, rgb(255, 255, 255));
            }
            Kind::FontRow { index, selected } => {
                let bg = if *selected { pal.accent_soft } else if hovered { pal.surface2 } else { pal.surface };
                cv.fill_rrect(x, y, wd_w, ht, scale(12, d) as f32, bg);
                cv.shape(x + 0.5, y + 0.5, wd_w - 1.0, ht - 1.0, scale(12, d) as f32, if *selected { 2.0 } else { 1.0 }, if *selected { pal.accent } else { pal.border }, 1.0);
                // sample in that very font, name below in small text
                let sample = gfx::font_by_index(*index, scale(20, d));
                cv.text(sample, "\u{0986}\u{09AE}\u{09BF} \u{09AC}\u{09BE}\u{0982}\u{09B2}\u{09BE}\u{09DF} \u{0995}\u{09A5}\u{09BE}", rect(r.left + scale(14, d), r.top + scale(2, d), r.right - scale(8, d), r.top + scale(32, d)), text_col(pal.text), DT_LEFT | DT_VCENTER);
                DeleteObject(sample);
                cv.text(small, &wd.text, rect(r.left + scale(14, d), r.top + scale(30, d), r.right - scale(8, d), r.bottom - scale(2, d)), text_col(pal.muted), DT_LEFT | DT_VCENTER);
            }
            Kind::Swatch { color, selected } => {
                let c = rgb(color.0, color.1, color.2);
                let rr = wd_w / 2.0;
                cv.shape(x, y, wd_w, ht, rr, 0.0, c, 1.0);
                if *selected {
                    cv.shape(x - 3.0, y - 3.0, wd_w + 6.0, ht + 6.0, rr + 3.0, 2.0, c, 1.0);
                } else if hovered {
                    cv.shape(x - 2.0, y - 2.0, wd_w + 4.0, ht + 4.0, rr + 2.0, 1.5, c, 0.6);
                }
            }
            Kind::DictRow { english, bangla } => {
                if hovered {
                    cv.fill_rrect(x, y, wd_w, ht, scale(8, d) as f32, pal.surface2);
                }
                cv.text(body_b, english, rect(r.left + scale(12, d), r.top, r.left + (r.right - r.left) / 2 - scale(20, d), r.bottom), text_col(pal.text), DT_LEFT | DT_VCENTER | DT_END_ELLIPSIS);
                cv.text(arrow, "\u{2192}", rect(r.left + (r.right - r.left) / 2 - scale(20, d), r.top, r.left + (r.right - r.left) / 2, r.bottom), pal.muted, DT_CENTER | DT_VCENTER);
                cv.text(bn, bangla, rect(r.left + (r.right - r.left) / 2, r.top, r.right - scale(48, d), r.bottom), text_col(pal.accent), DT_LEFT | DT_VCENTER | DT_END_ELLIPSIS);
            }
            Kind::LinkButton => {
                if hovered {
                    cv.fill_rrect(x, y, wd_w, ht, scale(8, d) as f32, pal.surface2);
                }
                cv.shape(x + 0.5, y + 0.5, wd_w - 1.0, ht - 1.0, scale(8, d) as f32, 1.0, pal.border, 1.0);
                cv.text(small, &wd.text, r, text_col(pal.accent), DT_CENTER | DT_VCENTER);
            }
            Kind::Primary => {
                let bg = if hovered { lerp_color(pal.accent, rgb(255, 255, 255), 0.12) } else { pal.accent };
                cv.fill_rrect(x, y, wd_w, ht, scale(10, d) as f32, bg);
                cv.text(body_b, &wd.text, r, pal.accent_text, DT_CENTER | DT_VCENTER);
            }
            Kind::Hero => {
                cv.fill_rrect(x, y, wd_w, ht, scale(18, d) as f32, pal.accent);
                let lg = ht * 0.62;
                cv.logo(x + 36.0, y + (ht - lg) / 2.0, lg, pal.accent_text, 1.0);
                let big = gfx::ui_font(scale(44, d), true);
                let mid = gfx::ui_font(scale(16, d), false);
                let bn_big = gfx::bangla_font(s, scale(26, d), true);
                let tx = (x + 36.0 + lg + 32.0) as i32;
                cv.text(big, "Jype", rect(tx, r.top + scale(30, d), r.right, r.top + scale(90, d)), pal.accent_text, DT_LEFT | DT_VCENTER);
                cv.text(bn_big, "\u{099C}\u{09BE}\u{0987}\u{09AA}", rect(tx, r.top + scale(88, d), r.right, r.top + scale(128, d)), pal.accent_text, DT_LEFT | DT_VCENTER);
                cv.text(mid, "The key to Bangla typing   \u{00B7}   v0.3", rect(tx, r.top + scale(128, d), r.right, r.top + scale(160, d)), lerp_color(pal.accent, pal.accent_text, 0.8), DT_LEFT | DT_VCENTER);
                DeleteObject(big);
                DeleteObject(mid);
                DeleteObject(bn_big);
            }
        }
    }

    DeleteObject(title);
    DeleteObject(small);
    DeleteObject(body);
    DeleteObject(body_b);
    DeleteObject(head);
    DeleteObject(icons);
    DeleteObject(bn);
    DeleteObject(arrow);
}
