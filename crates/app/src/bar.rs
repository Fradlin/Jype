//! The floating Jype bar.
//!
//! A small rounded card: the Jype badge (click = switch Bangla/English), the
//! name and current mode, and a chevron that slides the bar open to a row of
//! icon buttons (layout, words, font, settings, more). Drag it anywhere; its
//! position and open/closed state are remembered.

use std::{
    cell::RefCell,
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::DwmSetWindowAttribute, Gdi::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT},
        WindowsAndMessaging::*,
    },
};

use crate::{
    app,
    gfx::{self, ease_out, palette, rect, scale, Canvas},
    studio::{self, Tab},
    tray,
    util::wide,
};

const CLASS: &str = "JypeBar";
const DRAG_THRESHOLD: i32 = 4;
const TIMER_ANIM: usize = 1;
// Not exported by the bindings feature set we use.
const WM_MOUSELEAVE: u32 = 0x02A3;

/// (icon glyph in Segoe MDL2 Assets, label)
const BUTTONS: [(&str, &str); 5] = [
    ("\u{E765}", "Layout"),
    ("\u{E82D}", "Words"),
    ("\u{E8D2}", "Font"),
    ("\u{E713}", "Settings"),
    ("\u{E712}", "More"),
];
const CHEVRON_RIGHT: &str = "\u{E76C}";
const CHEVRON_LEFT: &str = "\u{E76B}";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hit {
    None,
    Badge,
    Chevron,
    Button(usize),
}

struct Drag {
    cursor: POINT,
    origin: POINT,
    moved: bool,
    pressed: Hit,
}

struct Bar {
    hwnd: HWND,
    dpi: u32,
    expanded: bool,
    /// 0 = closed .. 1 = fully open (animated)
    t: f32,
    hover: Hit,
    drag: Option<Drag>,
}

thread_local! {
    static BAR: RefCell<Bar> = RefCell::new(Bar {
        hwnd: null_mut(),
        dpi: 96,
        expanded: false,
        t: 0.0,
        hover: Hit::None,
        drag: None,
    });
}

/// Layout numbers for one frame (all in device pixels).
struct Geo {
    w: i32,
    h: i32,
    badge: RECT,
    name: RECT,
    buttons: [RECT; 5],
    chevron: RECT,
}

fn width_closed(d: u32) -> i32 {
    scale(138, d)
}
fn width_open(d: u32) -> i32 {
    scale(350, d)
}

fn geo(d: u32, t: f32) -> Geo {
    let e = ease_out(t);
    let h = scale(46, d);
    let w = width_closed(d) + ((width_open(d) - width_closed(d)) as f32 * e) as i32;
    let m = scale(7, d);
    let bd = scale(32, d);
    let badge = rect(m, (h - bd) / 2, m + bd, (h - bd) / 2 + bd);
    let cw = scale(28, d);
    let chevron = rect(w - m - cw, (h - cw) / 2, w - m, (h - cw) / 2 + cw);
    let bw = scale(34, d);
    let mut buttons = [rect(0, 0, 0, 0); 5];
    let mut x = badge.right + scale(8, d);
    for b in buttons.iter_mut() {
        *b = rect(x, (h - bw) / 2, x + bw, (h - bw) / 2 + bw);
        x += bw + scale(2, d);
    }
    // name block: right after the badge when closed, after the buttons when open
    let name_x = badge.right + scale(10, d) + ((x - badge.right - scale(8, d)) as f32 * e) as i32;
    let name = rect(name_x, 0, (chevron.left - scale(4, d)).max(name_x + 10), h);
    Geo { w, h, badge, name, buttons, chevron }
}

fn hit_test(g: &Geo, x: i32, y: i32, open: bool) -> Hit {
    let inside = |r: &RECT| x >= r.left && x < r.right && y >= r.top && y < r.bottom;
    if inside(&g.chevron) {
        return Hit::Chevron;
    }
    if inside(&g.badge) {
        return Hit::Badge;
    }
    if open {
        for (i, b) in g.buttons.iter().enumerate() {
            if inside(b) {
                return Hit::Button(i);
            }
        }
    }
    Hit::None
}

fn default_position(dpi: u32) -> (i32, i32) {
    let h = scale(46, dpi);
    let mut work = RECT { left: 0, top: 0, right: 1280, bottom: 720 };
    unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut RECT as *mut c_void, 0);
    }
    (work.right - width_closed(dpi) - scale(24, dpi), work.bottom - h - scale(24, dpi))
}

fn on_a_screen(x: i32, y: i32, dpi: u32) -> bool {
    let (w, h) = (width_closed(dpi), scale(46, dpi));
    unsafe { !MonitorFromPoint(POINT { x: x + w / 2, y: y + h / 2 }, MONITOR_DEFAULTTONULL).is_null() }
}

/// Resize/redraw for the current animation state (keeps the left edge in place).
unsafe fn apply_shape(hwnd: HWND, dpi: u32, t: f32, x: Option<i32>, y: Option<i32>) {
    let g = geo(dpi, t);
    let (mut flags, px, py) = (SWP_NOACTIVATE | SWP_NOZORDER, x.unwrap_or(0), y.unwrap_or(0));
    if x.is_none() || y.is_none() {
        flags |= SWP_NOMOVE;
    }
    SetWindowPos(hwnd, null_mut(), px, py, g.w, g.h, flags);
    // Windows 11 rounds popup windows itself; Windows 10 gets a clipping region.
    let round: i32 = 2; // DWMWCP_ROUND
    if DwmSetWindowAttribute(hwnd, 33, &round as *const i32 as *const c_void, 4) != 0 {
        let r = scale(8, dpi) * 2;
        SetWindowRgn(hwnd, CreateRoundRectRgn(0, 0, g.w + 1, g.h + 1, r, r), 1);
    }
    InvalidateRect(hwnd, null(), 0);
}

/// Create the bar (and show it if the user wants it).
pub fn init() {
    unsafe {
        let hinst = GetModuleHandleW(null());
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_HAND),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
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
        if hwnd.is_null() {
            return;
        }
        let settings = app::settings();
        let d = GetDpiForWindow(hwnd);
        let dpi = if d == 0 { 96 } else { d };
        let t = if settings.bar_expanded { 1.0 } else { 0.0 };
        BAR.with(|b| {
            let mut b = b.borrow_mut();
            b.hwnd = hwnd;
            b.dpi = dpi;
            b.expanded = settings.bar_expanded;
            b.t = t;
        });
        let (x, y) = match settings.bar_pos {
            Some((x, y)) if on_a_screen(x, y, dpi) => (x, y),
            _ => default_position(dpi),
        };
        apply_shape(hwnd, dpi, t, Some(x), Some(y));
        if settings.bar_visible {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
    }
}

pub fn set_visible(visible: bool) {
    let hwnd = BAR.with(|b| b.borrow().hwnd);
    if hwnd.is_null() {
        return;
    }
    unsafe {
        ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
}

/// Repaint after the mode, theme, font or accent changed.
pub fn refresh() {
    let hwnd = BAR.with(|b| b.borrow().hwnd);
    if !hwnd.is_null() {
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
}

fn toggle_expanded() {
    let (hwnd, now) = BAR.with(|b| {
        let mut b = b.borrow_mut();
        b.expanded = !b.expanded;
        (b.hwnd, b.expanded)
    });
    app::save_bar_expanded(now);
    unsafe { SetTimer(hwnd, TIMER_ANIM, 15, None) };
}

/// Where a menu opened from a bar button should appear (just under the button).
fn button_anchor(hwnd: HWND, r: &RECT) -> (i32, i32) {
    let mut p = POINT { x: r.left, y: r.bottom };
    unsafe { ClientToScreen(hwnd, &mut p) };
    (p.x, p.y)
}

fn click(hwnd: HWND, hit: Hit) {
    let dpi = BAR.with(|b| b.borrow().dpi);
    let t = BAR.with(|b| b.borrow().t);
    let g = geo(dpi, t);
    match hit {
        Hit::Badge => app::toggle_mode(false),
        Hit::Chevron => toggle_expanded(),
        Hit::Button(0) => {
            let (x, y) = button_anchor(hwnd, &g.buttons[0]);
            tray::open_layout_menu(x, y);
        }
        Hit::Button(1) => studio::open(Tab::Dictionary),
        Hit::Button(2) => {
            let (x, y) = button_anchor(hwnd, &g.buttons[2]);
            tray::open_font_menu(x, y);
        }
        Hit::Button(3) => studio::open(Tab::Look),
        Hit::Button(_) => {
            let (x, y) = button_anchor(hwnd, &g.buttons[4]);
            tray::open_menu_at(x, y);
        }
        Hit::None => {}
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let mouse = |lp: LPARAM| (lp as u16 as i16 as i32, ((lp >> 16) & 0xFFFF) as u16 as i16 as i32);
    match msg {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let settings = app::settings();
            BAR.with(|b| {
                if let Ok(b) = b.try_borrow() {
                    draw(hwnd, hdc, &b, &settings);
                }
            });
            EndPaint(hwnd, &ps);
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = mouse(lp);
            let mut cursor = POINT { x: 0, y: 0 };
            GetCursorPos(&mut cursor);
            let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetWindowRect(hwnd, &mut r);
            BAR.with(|b| {
                if let Ok(mut b) = b.try_borrow_mut() {
                    let g = geo(b.dpi, b.t);
                    let pressed = hit_test(&g, x, y, b.t > 0.5);
                    b.drag = Some(Drag { cursor, origin: POINT { x: r.left, y: r.top }, moved: false, pressed });
                }
            });
            SetCapture(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = mouse(lp);
            // hover highlight
            let (changed, newly_inside) = BAR.with(|b| {
                let Ok(mut b) = b.try_borrow_mut() else { return (false, false) };
                let g = geo(b.dpi, b.t);
                let h = hit_test(&g, x, y, b.t > 0.5);
                let changed = h != b.hover;
                b.hover = h;
                (changed, changed)
            });
            if newly_inside {
                let mut t = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut t);
            }
            if changed {
                InvalidateRect(hwnd, null(), 0);
            }
            // dragging
            let target = BAR.with(|b| {
                let Ok(mut b) = b.try_borrow_mut() else { return None };
                let Some(d) = b.drag.as_mut() else { return None };
                let mut c = POINT { x: 0, y: 0 };
                GetCursorPos(&mut c);
                let (dx, dy) = (c.x - d.cursor.x, c.y - d.cursor.y);
                if !d.moved && dx.abs() < DRAG_THRESHOLD && dy.abs() < DRAG_THRESHOLD {
                    return None;
                }
                d.moved = true;
                Some((d.origin.x + dx, d.origin.y + dy))
            });
            if let Some((tx, ty)) = target {
                SetWindowPos(hwnd, null_mut(), tx, ty, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
            }
            0
        }
        WM_MOUSELEAVE => {
            BAR.with(|b| {
                if let Ok(mut b) = b.try_borrow_mut() {
                    b.hover = Hit::None;
                }
            });
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            let drag = BAR.with(|b| b.try_borrow_mut().ok().and_then(|mut b| b.drag.take()));
            let (x, y) = mouse(lp);
            match drag {
                Some(d) if d.moved => {
                    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    GetWindowRect(hwnd, &mut r);
                    app::save_bar_pos(r.left, r.top);
                }
                Some(d) => {
                    // a plain click: act only if it ended on the same thing it started on
                    let now = BAR.with(|b| {
                        let b = b.borrow();
                        hit_test(&geo(b.dpi, b.t), x, y, b.t > 0.5)
                    });
                    if now == d.pressed {
                        click(hwnd, now);
                    }
                }
                None => {}
            }
            0
        }
        WM_RBUTTONUP => {
            tray::open_menu();
            0
        }
        WM_TIMER => {
            if wp == TIMER_ANIM {
                let (dpi, t, done) = BAR.with(|b| {
                    let Ok(mut b) = b.try_borrow_mut() else { return (96, 0.0, false) };
                    let goal = if b.expanded { 1.0 } else { 0.0 };
                    let step = 0.09;
                    if b.t < goal {
                        b.t = (b.t + step).min(goal);
                    } else if b.t > goal {
                        b.t = (b.t - step).max(goal);
                    }
                    (b.dpi, b.t, b.t == goal)
                });
                apply_shape(hwnd, dpi, t, None, None);
                if done {
                    KillTimer(hwnd, TIMER_ANIM);
                    keep_on_screen(hwnd, dpi, t);
                }
            }
            0
        }
        WM_DPICHANGED => {
            let new_dpi = (wp & 0xFFFF) as u32;
            let suggested = &*(lp as *const RECT);
            let t = BAR.with(|b| {
                let mut b = b.borrow_mut();
                b.dpi = new_dpi;
                b.t
            });
            apply_shape(hwnd, new_dpi, t, Some(suggested.left), Some(suggested.top));
            0
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            refresh();
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// After the bar opens, pull it back if it now hangs off the right edge of the screen.
unsafe fn keep_on_screen(hwnd: HWND, dpi: u32, t: f32) {
    let g = geo(dpi, t);
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetWindowRect(hwnd, &mut r);
    let mon = MonitorFromPoint(POINT { x: r.left + g.w / 2, y: r.top + g.h / 2 }, MONITOR_DEFAULTTONEAREST);
    let mut mi: MONITORINFO = zeroed();
    mi.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(mon, &mut mi) != 0 && r.left + g.w > mi.rcWork.right {
        let x = mi.rcWork.right - g.w;
        SetWindowPos(hwnd, null_mut(), x, r.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        app::save_bar_pos(x, r.top);
    }
}

unsafe fn draw(hwnd: HWND, screen_dc: HDC, b: &Bar, s: &crate::settings::Settings) {
    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(hwnd, &mut client);
    let (w, h) = (client.right, client.bottom);
    let Some(mut cv) = Canvas::new(screen_dc, w, h) else { return };
    let pal = palette(s);
    let d = b.dpi;
    let g = geo(d, b.t);
    let on = s.bangla_on;
    let e = ease_out(b.t);

    cv.clear(pal.surface);
    cv.shape(0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, scale(8, d) as f32, 1.0, pal.border, 1.0);

    // badge: logo on the accent colour (grey when typing is off)
    let bd = (g.badge.right - g.badge.left) as f32;
    let badge_color = if on { pal.accent } else { pal.muted };
    let hover_badge = b.hover == Hit::Badge;
    let grow = if hover_badge { scale(1, d) as f32 } else { 0.0 };
    cv.shape(
        g.badge.left as f32 - grow,
        g.badge.top as f32 - grow,
        bd + grow * 2.0,
        bd + grow * 2.0,
        scale(8, d) as f32 + grow,
        0.0,
        badge_color,
        1.0,
    );
    let logo = bd * 0.62;
    cv.logo(
        g.badge.left as f32 + (bd - logo) / 2.0,
        g.badge.top as f32 + (bd - logo) / 2.0,
        logo,
        pal.accent_text,
        if on { 1.0 } else { 0.85 },
    );

    // icon buttons (fade in while the bar opens)
    let icons = gfx::icon_font(scale(16, d));
    if e > 0.02 {
        for (i, r) in g.buttons.iter().enumerate() {
            let hovered = b.hover == Hit::Button(i);
            if hovered {
                cv.shape(r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32, scale(8, d) as f32, 0.0, pal.accent_soft, e);
            }
            let col = if hovered { pal.accent } else { pal.text };
            // fade the glyph by blending toward the surface colour
            let col = fade(col, pal.surface, e);
            cv.text(icons, BUTTONS[i].0, *r, col, DT_CENTER | DT_VCENTER);
        }
    }

    // name + mode (or the label of the hovered button)
    let name_font = gfx::ui_font(scale(14, d), true);
    let small = gfx::ui_font(scale(12, d), false);
    let bn_small = gfx::bangla_font(s, scale(13, d), false);
    let mid = h / 2;
    let hover_label = match b.hover {
        Hit::Button(i) if e > 0.5 => Some(BUTTONS[i].1),
        _ => None,
    };
    let top_r = rect(g.name.left, scale(5, d), g.name.right, mid + scale(2, d));
    let bot_r = rect(g.name.left, mid, g.name.right, h - scale(4, d));
    if let Some(label) = hover_label {
        cv.text(name_font, label, g.name, pal.accent, DT_LEFT | DT_VCENTER);
    } else {
        cv.text(name_font, "Jype", top_r, pal.text, DT_LEFT | DT_BOTTOM);
        if on {
            cv.text(bn_small, "\u{09AC}\u{09BE}\u{0982}\u{09B2}\u{09BE}", bot_r, pal.muted, DT_LEFT | DT_TOP);
        } else {
            cv.text(small, "English", bot_r, pal.muted, DT_LEFT | DT_TOP);
        }
    }

    // chevron
    let chev_hover = b.hover == Hit::Chevron;
    if chev_hover {
        let c = g.chevron;
        cv.shape(c.left as f32, c.top as f32, (c.right - c.left) as f32, (c.bottom - c.top) as f32, scale(8, d) as f32, 0.0, pal.accent_soft, 1.0);
    }
    let chev_font = gfx::icon_font(scale(13, d));
    cv.text(
        chev_font,
        if b.expanded { CHEVRON_LEFT } else { CHEVRON_RIGHT },
        g.chevron,
        if chev_hover { pal.accent } else { pal.muted },
        DT_CENTER | DT_VCENTER,
    );

    DeleteObject(icons);
    DeleteObject(name_font);
    DeleteObject(small);
    DeleteObject(bn_small);
    DeleteObject(chev_font);
    cv.present(screen_dc);
}

/// Blend colour `a` toward `b` (`t` = 1 keeps `a`).
fn fade(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, sh: u32| ((c >> sh) & 0xFF) as f32;
    let m = |sh: u32| (ch(a, sh) * t + ch(b, sh) * (1.0 - t)).round() as u32;
    m(0) | (m(8) << 8) | (m(16) << 16)
}
