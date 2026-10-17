//! The suggestion card shown next to the text cursor while a word is typed.
//!
//! A top-most, non-activating window (it never steals focus from the app you are
//! typing in). It fades in, shows what you typed so far in a chip at the top,
//! and the highlight glides between suggestions.

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};

use bangla_engine::View;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::DwmSetWindowAttribute, Gdi::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
};

use crate::{
    app,
    gfx::{self, palette, rect, scale, Canvas, Palette},
    settings::Settings,
    util::wide,
};

const CLASS: &str = "JypePopup";
const MAX_ROWS: usize = 9;
const TIMER_FLASH: usize = 1;
const TIMER_ANIM: usize = 2;
const LWA_ALPHA_FLAG: u32 = 2;

struct Popup {
    hwnd: HWND,
    view: View,
    first: usize,
    settings: Settings,
    dpi: u32,
    show_hint: bool,
    /// Brief mode indicator ("বাংলা" / "English") instead of candidates.
    flash: Option<String>,
    visible: bool,
    /// (x, caret top, caret bottom) in screen pixels.
    anchor: (i32, i32, i32),
    size: (i32, i32),
    /// 0..1 fade-in progress.
    fade: f32,
    /// Highlight position (px from the first row) and where it is heading.
    hl_y: f32,
    hl_target: f32,
}

thread_local! {
    static POPUP: RefCell<Popup> = RefCell::new(Popup {
        hwnd: null_mut(),
        view: View::default(),
        first: 0,
        settings: Settings::default(),
        dpi: 96,
        show_hint: false,
        flash: None,
        visible: false,
        anchor: (0, 0, 0),
        size: (0, 0),
        fade: 1.0,
        hl_y: 0.0,
        hl_target: 0.0,
    });
    static ON_CLICK: Cell<Option<fn(usize)>> = const { Cell::new(None) };
}

pub fn set_click_handler(f: fn(usize)) {
    ON_CLICK.with(|c| c.set(Some(f)));
}

struct M {
    pad: i32,
    head: i32,
    row: i32,
    badge: i32,
    foot: i32,
    radius: i32,
}

fn metrics(dpi: u32) -> M {
    M {
        pad: scale(10, dpi),
        head: scale(38, dpi),
        row: scale(40, dpi),
        badge: scale(22, dpi),
        foot: scale(26, dpi),
        radius: scale(8, dpi),
    }
}

const HINT: &str = "Space accept    \u{2191}\u{2193} choose    Esc cancel";

impl Popup {
    unsafe fn ensure_window(&mut self) {
        if !self.hwnd.is_null() {
            return;
        }
        let hinst = GetModuleHandleW(null());
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            style: CS_DROPSHADOW | CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        self.hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class.as_ptr(),
            wide("Jype").as_ptr(),
            WS_POPUP,
            0,
            0,
            10,
            10,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        SetLayeredWindowAttributes(self.hwnd, 0, 255, LWA_ALPHA_FLAG);
    }

    fn range(&self) -> (usize, usize) {
        let n = self.view.candidates.len();
        let first = if self.view.selected >= MAX_ROWS { self.view.selected + 1 - MAX_ROWS } else { 0 };
        (first, (first + MAX_ROWS).min(n))
    }

    /// Work out size, then position the window under the caret and show it.
    unsafe fn layout_and_show(&mut self) {
        self.ensure_window();
        let m = metrics(self.dpi);
        let was_visible = self.visible;
        let hdc = GetDC(self.hwnd);
        let cv = Canvas::new(hdc, 4, 4);
        let (w, h);
        if let Some(cv) = &cv {
            let ui = gfx::ui_font(scale(14, self.dpi), true);
            let small = gfx::ui_font(scale(12, self.dpi), false);
            let bn = gfx::bangla_font(&self.settings, scale(gfx::size_px(&self.settings), self.dpi), false);
            if let Some(text) = &self.flash {
                let tw = cv.text_width(bn, text);
                w = m.pad * 2 + scale(30, self.dpi) + tw + m.pad;
                h = scale(46, self.dpi);
            } else {
                self.first = self.range().0;
                let (first, last) = self.range();
                let mut max_c = 0;
                for c in &self.view.candidates[first..last] {
                    max_c = max_c.max(cv.text_width(bn, c));
                }
                let typed_w = cv.text_width(ui, &self.view.typed) + scale(24, self.dpi);
                let chip_w = m.pad * 2 + typed_w + scale(20, self.dpi) + scale(12, self.dpi);
                let hint_w = if self.show_hint { cv.text_width(small, HINT) + m.pad * 2 } else { 0 };
                let rows_w = m.pad * 2 + m.badge + scale(12, self.dpi) + max_c + m.pad;
                w = rows_w.max(chip_w).max(hint_w).max(scale(190, self.dpi));
                h = m.head + (last - first) as i32 * m.row + m.pad + if self.show_hint { m.foot } else { 0 };
            }
            DeleteObject(ui);
            DeleteObject(small);
            DeleteObject(bn);
        } else {
            w = scale(200, self.dpi);
            h = scale(120, self.dpi);
        }
        drop(cv);
        ReleaseDC(self.hwnd, hdc);
        self.size = (w, h);

        // Place under the caret, flip above it / clamp if it would leave the screen.
        let (ax, atop, abottom) = self.anchor;
        let gap = scale(6, self.dpi);
        let pt = POINT { x: ax, y: abottom };
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = zeroed();
        mi.cbSize = size_of::<MONITORINFO>() as u32;
        let (mut x, mut y) = (ax, abottom + gap);
        if GetMonitorInfoW(mon, &mut mi) != 0 {
            let work = mi.rcWork;
            if y + h > work.bottom {
                y = atop - h - gap;
            }
            if x + w > work.right {
                x = work.right - w;
            }
            x = x.max(work.left);
            y = y.max(work.top);
        }

        // Rounded corners: Windows 11 does it itself; Windows 10 gets a clip region.
        let round: i32 = 2; // DWMWCP_ROUND
        if DwmSetWindowAttribute(self.hwnd, 33, &round as *const i32 as *const c_void, 4) != 0 {
            let r = m.radius * 2;
            SetWindowRgn(self.hwnd, CreateRoundRectRgn(0, 0, w + 1, h + 1, r, r), 1);
        }

        // highlight target
        let sel_row = self.view.selected.saturating_sub(self.first) as f32;
        self.hl_target = sel_row * m.row as f32;
        if !was_visible || self.flash.is_some() {
            self.hl_y = self.hl_target;
            self.fade = 0.0;
            SetLayeredWindowAttributes(self.hwnd, 0, 0, LWA_ALPHA_FLAG);
        }
        SetWindowPos(self.hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        InvalidateRect(self.hwnd, null(), 0);
        self.visible = true;
        SetTimer(self.hwnd, TIMER_ANIM, 15, None);
    }
}

/// Where the text cursor is: (x, top, bottom) in screen pixels.
unsafe fn caret_anchor() -> (i32, i32, i32) {
    let fg = GetForegroundWindow();
    let tid = if fg.is_null() { 0 } else { GetWindowThreadProcessId(fg, null_mut()) };
    let mut gti: GUITHREADINFO = zeroed();
    gti.cbSize = size_of::<GUITHREADINFO>() as u32;
    if GetGUIThreadInfo(tid, &mut gti) != 0 && !gti.hwndCaret.is_null() {
        let r = gti.rcCaret;
        if r.bottom > r.top {
            let mut tl = POINT { x: r.left, y: r.top };
            let mut bl = POINT { x: r.left, y: r.bottom };
            ClientToScreen(gti.hwndCaret, &mut tl);
            ClientToScreen(gti.hwndCaret, &mut bl);
            return (tl.x, tl.y, bl.y);
        }
    }
    // Many modern apps (browsers, Electron, UWP) don't expose a Win32 caret.
    // Fall back to the mouse pointer so the popup is still visible and near.
    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    (pt.x, pt.y, pt.y + 18)
}

unsafe fn foreground_dpi() -> u32 {
    let fg = GetForegroundWindow();
    let d = if fg.is_null() { 0 } else { GetDpiForWindow(fg) };
    if d == 0 { 96 } else { d }
}

/// Show or update the candidate list.
pub fn show(view: &View, show_hint: bool) {
    let settings = app::settings();
    unsafe {
        POPUP.with(|p| {
            let mut p = p.borrow_mut();
            if !p.visible || p.flash.is_some() {
                p.anchor = caret_anchor();
                p.dpi = foreground_dpi();
            }
            p.settings = settings;
            p.flash = None;
            p.view = view.clone();
            p.show_hint = show_hint;
            p.layout_and_show();
        });
    }
}

/// Briefly show a mode label ("বাংলা" / "English") at the text cursor.
pub fn flash(text: &str) {
    let settings = app::settings();
    unsafe {
        POPUP.with(|p| {
            let mut p = p.borrow_mut();
            p.anchor = caret_anchor();
            p.dpi = foreground_dpi();
            p.settings = settings;
            p.flash = Some(text.to_string());
            p.layout_and_show();
            SetTimer(p.hwnd, TIMER_FLASH, 1100, None);
        });
    }
}

pub fn hide() {
    let hwnd = POPUP.with(|p| {
        let mut p = p.borrow_mut();
        if !p.visible {
            return null_mut();
        }
        p.visible = false;
        p.flash = None;
        p.hwnd
    });
    if !hwnd.is_null() {
        unsafe {
            KillTimer(hwnd, TIMER_FLASH);
            KillTimer(hwnd, TIMER_ANIM);
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

/// Screen rectangle of the popup while it is visible (used to ignore clicks on it).
pub fn screen_rect() -> Option<RECT> {
    POPUP.with(|p| {
        let p = p.borrow();
        if !p.visible || p.hwnd.is_null() {
            return None;
        }
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetWindowRect(p.hwnd, &mut r) };
        Some(r)
    })
}

fn row_at(y: i32) -> Option<usize> {
    POPUP.with(|p| {
        let p = p.try_borrow().ok()?;
        if p.flash.is_some() {
            return None;
        }
        let m = metrics(p.dpi);
        if y < m.head {
            return None;
        }
        let (first, last) = p.range();
        let idx = first + ((y - m.head) / m.row) as usize;
        if idx < last { Some(idx) } else { None }
    })
}

/// One animation step. Returns (hwnd, alpha to apply, finished).
fn animate() -> (HWND, u8, bool) {
    POPUP.with(|p| {
        let Ok(mut p) = p.try_borrow_mut() else { return (null_mut(), 255, false) };
        let mut done = true;
        if p.fade < 1.0 {
            p.fade = (p.fade + 0.16).min(1.0);
            done = false;
        }
        let diff = p.hl_target - p.hl_y;
        if diff.abs() > 0.5 {
            p.hl_y += diff * 0.38;
            done = false;
        } else {
            p.hl_y = p.hl_target;
        }
        let alpha = (gfx::ease_out(p.fade) * 255.0) as u8;
        (p.hwnd, alpha, done)
    })
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            POPUP.with(|p| {
                if let Ok(p) = p.try_borrow() {
                    draw(hwnd, hdc, &p);
                }
            });
            EndPaint(hwnd, &ps);
            0
        }
        WM_LBUTTONDOWN => {
            let y = ((lp >> 16) & 0xFFFF) as u16 as i16 as i32;
            if let Some(i) = row_at(y) {
                if let Some(f) = ON_CLICK.with(|c| c.get()) {
                    f(i);
                }
            }
            0
        }
        WM_TIMER => {
            if wp == TIMER_FLASH {
                hide();
            } else if wp == TIMER_ANIM {
                let (h, alpha, done) = animate();
                if !h.is_null() {
                    SetLayeredWindowAttributes(h, 0, alpha, LWA_ALPHA_FLAG);
                    InvalidateRect(h, null(), 0);
                    if done {
                        KillTimer(h, TIMER_ANIM);
                    }
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn draw(hwnd: HWND, screen_dc: HDC, p: &Popup) {
    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(hwnd, &mut client);
    let (w, h) = (client.right, client.bottom);
    let Some(mut cv) = Canvas::new(screen_dc, w, h) else { return };
    let pal: Palette = palette(&p.settings);
    let m = metrics(p.dpi);
    let d = p.dpi;
    let (wf, hf) = (w as f32, h as f32);
    let rad = m.radius as f32;

    if let Some(text) = &p.flash {
        // Mode indicator: accent pill with the logo.
        cv.clear(pal.accent);
        let logo = scale(22, d) as f32;
        cv.logo(m.pad as f32, (hf - logo) / 2.0, logo, pal.accent_text, 1.0);
        let bn = gfx::bangla_font(&p.settings, scale(gfx::size_px(&p.settings), d), true);
        let r = rect(m.pad * 2 + scale(22, d), 0, w - m.pad, h);
        cv.text(bn, text, r, pal.accent_text, DT_VCENTER | DT_LEFT);
        DeleteObject(bn);
        cv.present(screen_dc);
        return;
    }

    cv.clear(pal.surface);
    cv.shape(0.5, 0.5, wf - 1.0, hf - 1.0, rad, 1.0, pal.border, 1.0);

    let ui = gfx::ui_font(scale(14, d), true);
    let small = gfx::ui_font(scale(12, d), false);
    let digit = gfx::ui_font(scale(12, d), true);
    let bn = gfx::bangla_font(&p.settings, scale(gfx::size_px(&p.settings), d), false);

    // Header: what you typed so far, in a chip, and the logo on the right.
    let chip_h = scale(26, d) as f32;
    let chip_y = (m.head as f32 - chip_h) / 2.0 + scale(2, d) as f32;
    if !p.view.typed.is_empty() {
        let tw = cv.text_width(ui, &p.view.typed);
        let chip_w = (tw + scale(24, d)) as f32;
        cv.fill_rrect(m.pad as f32, chip_y, chip_w, chip_h, chip_h / 2.0, pal.accent_soft);
        let r = rect(m.pad, chip_y as i32, m.pad + chip_w as i32, (chip_y + chip_h) as i32);
        cv.text(ui, &p.view.typed, r, pal.accent, DT_CENTER | DT_VCENTER);
    }
    let logo = scale(20, d) as f32;
    cv.logo(wf - m.pad as f32 - logo, chip_y + (chip_h - logo) / 2.0, logo, pal.accent, 0.9);

    // Sliding highlight behind the selected row.
    let (first, last) = p.range();
    let inset = (m.pad as f32) * 0.5;
    if last > first {
        let y = m.head as f32 + p.hl_y;
        cv.fill_rrect(inset, y + 1.0, wf - inset * 2.0, m.row as f32 - 2.0, rad + 1.0, pal.accent);
    }
    for (row, idx) in (first..last).enumerate() {
        let top = m.head + row as i32 * m.row;
        // The highlight is animated, so "selected" means "the highlight is over this row".
        let center = top as f32 + m.row as f32 / 2.0;
        let hl_center = m.head as f32 + p.hl_y + m.row as f32 / 2.0;
        let on_hl = (center - hl_center).abs() < m.row as f32 / 2.0;
        let cx = (m.pad + m.badge / 2) as f32 + 2.0;
        let badge = m.badge as f32;
        if on_hl {
            cv.shape(cx - badge / 2.0, center - badge / 2.0, badge, badge, badge / 2.0, 0.0, pal.accent_text, 0.25);
        } else {
            cv.circle(cx, center, badge / 2.0, pal.surface2);
        }
        let nr = rect((cx - badge / 2.0) as i32, top, (cx + badge / 2.0) as i32, top + m.row);
        cv.text(digit, &(row + 1).to_string(), nr, if on_hl { pal.accent_text } else { pal.muted }, DT_CENTER | DT_VCENTER);
        let tr = rect(m.pad + m.badge + scale(12, d), top, w - m.pad, top + m.row);
        cv.text(bn, &p.view.candidates[idx], tr, if on_hl { pal.accent_text } else { pal.text }, DT_VCENTER | DT_LEFT);
    }

    if p.show_hint {
        let fr = rect(m.pad, h - m.foot, w - m.pad, h);
        cv.text(small, HINT, fr, pal.muted, DT_VCENTER | DT_LEFT);
    }

    DeleteObject(ui);
    DeleteObject(small);
    DeleteObject(digit);
    DeleteObject(bn);
    cv.present(screen_dc);
}
