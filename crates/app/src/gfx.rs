//! Drawing helpers shared by the bar, the popup and the Jype Studio window.
//!
//! * `Canvas` - an off-screen 32-bit bitmap with smooth (anti-aliased) rounded
//!   shapes and the Jype logo, plus GDI text on top. Windows' plain GDI has no
//!   anti-aliased shapes, so those are computed here.
//! * Fonts - the Bangla fonts are compiled into the .exe and registered for this
//!   process only (nothing is installed on the PC). Extra `.ttf` files dropped in
//!   `%APPDATA%\Jype\fonts` are loaded too (e.g. Noto Sans Bengali).
//! * Palette - colours for light/dark and the four accent colours.

use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};

use windows_sys::Win32::{Foundation::RECT, Graphics::Gdi::*};

use crate::{
    settings::{self, Settings},
    util::{rgb, system_uses_dark, wide},
};

// ------------------------------------------------------------------ fonts --

static EMBEDDED: &[&[u8]] = &[
    include_bytes!("../assets/fonts/KohinoorBangla-Regular.ttf"),
    include_bytes!("../assets/fonts/KohinoorBangla-Medium.ttf"),
    include_bytes!("../assets/fonts/KohinoorBangla-Semibold.ttf"),
    include_bytes!("../assets/fonts/HindSiliguri-Regular.ttf"),
    include_bytes!("../assets/fonts/HindSiliguri-Medium.ttf"),
    include_bytes!("../assets/fonts/HindSiliguri-SemiBold.ttf"),
    include_bytes!("../assets/fonts/AnekBangla-VariableFont_wdth_wght.ttf"),
    include_bytes!("../assets/fonts/TiroBangla-Regular.ttf"),
    include_bytes!("../assets/fonts/BalooDa2-Regular.ttf"),
    include_bytes!("../assets/fonts/BalooDa2-Medium.ttf"),
    include_bytes!("../assets/fonts/BalooDa2-Bold.ttf"),
    include_bytes!("../assets/fonts/Lato-Regular.ttf"),
    include_bytes!("../assets/fonts/Lato-Bold.ttf"),
    include_bytes!("../assets/fonts/Lato-Light.ttf"),
];

/// A Bangla font the user can pick.
pub struct FontChoice {
    pub id: &'static str,
    pub name: &'static str,
    /// GDI family name for normal text.
    pub regular: &'static str,
    /// GDI family name for slightly bolder text.
    pub strong: &'static str,
}

pub const FONTS: &[FontChoice] = &[
    FontChoice { id: "kohinoor", name: "Kohinoor Bangla", regular: "Kohinoor Bangla", strong: "Kohinoor Bangla Semibold" },
    FontChoice { id: "hind", name: "Hind Siliguri", regular: "Hind Siliguri", strong: "Hind Siliguri SemiBold" },
    FontChoice { id: "anek", name: "Anek Bangla", regular: "Anek Bangla Medium", strong: "Anek Bangla Medium" },
    FontChoice { id: "tiro", name: "Tiro Bangla", regular: "Tiro Bangla", strong: "Tiro Bangla" },
    FontChoice { id: "baloo", name: "Baloo Da 2", regular: "Baloo Da 2", strong: "Baloo Da 2 Medium" },
    FontChoice { id: "noto_sans", name: "Noto Sans Bengali", regular: "Noto Sans Bengali", strong: "Noto Sans Bengali" },
    FontChoice { id: "noto_serif", name: "Noto Serif Bengali", regular: "Noto Serif Bengali", strong: "Noto Serif Bengali" },
    FontChoice { id: "nirmala", name: "Windows default (Nirmala UI)", regular: "Nirmala UI", strong: "Nirmala UI" },
];

pub fn font_index(id: &str) -> usize {
    FONTS.iter().position(|f| f.id == id).unwrap_or(0)
}

/// Register the bundled fonts (and any in `%APPDATA%\Jype\fonts`) for this process.
pub fn register_fonts() {
    unsafe {
        for data in EMBEDDED {
            let count: u32 = 0;
            AddFontMemResourceEx(data.as_ptr() as *const c_void, data.len() as u32, null(), &count);
        }
        let dir = settings::data_dir().join("fonts");
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                if ext == "ttf" || ext == "otf" {
                    // 0x10 = FR_PRIVATE: visible to Jype only
                    AddFontResourceExW(wide(&p.to_string_lossy()).as_ptr(), 0x10, null());
                }
            }
        }
    }
}

pub unsafe fn make_font(face: &str, px: i32, weight: i32) -> HFONT {
    CreateFontW(
        -px,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        (DEFAULT_PITCH | FF_DONTCARE) as u32,
        wide(face).as_ptr(),
    )
}

/// The user's chosen Bangla font at `px` pixels (`strong` = bolder).
pub unsafe fn bangla_font(s: &Settings, px: i32, strong: bool) -> HFONT {
    let f = &FONTS[font_index(&s.font)];
    make_font(if strong { f.strong } else { f.regular }, px, 400)
}

/// A specific entry of the font list (for the font preview rows).
pub unsafe fn font_by_index(i: usize, px: i32) -> HFONT {
    make_font(FONTS[i.min(FONTS.len() - 1)].regular, px, 400)
}

/// Latin UI text (Lato, bundled; falls back to Segoe UI if somehow missing).
pub unsafe fn ui_font(px: i32, bold: bool) -> HFONT {
    make_font("Lato", px, if bold { 700 } else { 400 })
}

/// A slightly lighter weight for secondary UI text.
pub unsafe fn ui_font_light(px: i32) -> HFONT {
    make_font("Lato Light", px, 300)
}

/// Small icon font (Segoe Fluent Icons on Windows 11, MDL2 on Windows 10).
pub unsafe fn icon_font(px: i32) -> HFONT {
    make_font("Segoe MDL2 Assets", px, 400)
}

pub fn scale(v: i32, dpi: u32) -> i32 {
    (v as i64 * dpi as i64 / 96) as i32
}

/// Bangla text size choice (Small / Medium / Large) -> pixels at 96 dpi.
pub fn size_px(s: &Settings) -> i32 {
    match s.font_size {
        0 => 19,
        2 => 26,
        _ => 22,
    }
}

// ---------------------------------------------------------------- palette --

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: u32,
    pub surface: u32,
    pub surface2: u32,
    pub text: u32,
    pub muted: u32,
    pub border: u32,
    pub accent: u32,
    pub accent_text: u32,
    pub accent_soft: u32,
}

pub const ACCENTS: &[(&str, (u8, u8, u8))] = &[
    ("Maroon", (110, 8, 30)),
    ("Emerald", (0, 120, 88)),
    ("Ocean", (0, 100, 190)),
    ("Violet", (110, 64, 196)),
];

pub fn is_dark(s: &Settings) -> bool {
    match s.theme {
        1 => false,
        2 => true,
        _ => system_uses_dark(),
    }
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, sh: u32| ((c >> sh) & 0xFF) as f32;
    let m = |sh: u32| (ch(a, sh) * (1.0 - t) + ch(b, sh) * t).round() as u32;
    m(0) | (m(8) << 8) | (m(16) << 16)
}

pub fn palette(s: &Settings) -> Palette {
    let dark = is_dark(s);
    let (r, g, b) = ACCENTS[(s.accent as usize).min(ACCENTS.len() - 1)].1;
    let base = rgb(r, g, b);
    if dark {
        // lighten the accent so it reads on dark surfaces
        let accent = mix(base, rgb(255, 255, 255), 0.22);
        let surface = rgb(36, 34, 36);
        Palette {
            dark,
            bg: rgb(26, 24, 26),
            surface,
            surface2: rgb(46, 43, 46),
            text: rgb(244, 240, 238),
            muted: rgb(164, 158, 160),
            border: rgb(66, 62, 66),
            accent,
            accent_text: rgb(255, 255, 255),
            accent_soft: mix(surface, accent, 0.28),
        }
    } else {
        let surface = rgb(255, 253, 251);
        Palette {
            dark,
            bg: rgb(247, 242, 238),
            surface,
            surface2: rgb(242, 235, 231),
            text: rgb(38, 28, 30),
            muted: rgb(128, 116, 118),
            border: rgb(226, 216, 212),
            accent: base,
            accent_text: rgb(255, 255, 255),
            accent_soft: mix(surface, base, 0.12),
        }
    }
}

// ------------------------------------------------------------------ canvas --

static LOGO: &[u8] = include_bytes!("../assets/logo_mask.bin"); // 128 x 128 alpha mask
const LOGO_N: i32 = 128;

/// COLORREF (0x00BBGGRR) -> DIB pixel (0x00RRGGBB).
fn to_px(c: u32) -> u32 {
    ((c & 0xFF) << 16) | (c & 0xFF00) | ((c >> 16) & 0xFF)
}

pub struct Canvas {
    pub dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
    pub w: i32,
    pub h: i32,
}

impl Canvas {
    pub unsafe fn new(reference: HDC, w: i32, h: i32) -> Option<Canvas> {
        let (w, h) = (w.max(1), h.max(1));
        let dc = CreateCompatibleDC(reference);
        let mut bmi: BITMAPINFO = zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut c_void = null_mut();
        let bmp = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bmp.is_null() || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, bmp);
        Some(Canvas { dc, bmp, old, bits: bits as *mut u32, w, h })
    }

    fn px(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) }
    }

    pub fn clear(&mut self, color: u32) {
        let p = 0xFF00_0000 | to_px(color);
        self.px().fill(p);
    }

    /// Anti-aliased rounded rectangle. `stroke` > 0 draws only an outline of that
    /// width (inside the shape); 0 fills it. `alpha` is 0..1.
    pub fn shape(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, stroke: f32, color: u32, alpha: f32) {
        if w <= 0.0 || h <= 0.0 || alpha <= 0.0 {
            return;
        }
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let (hx, hy) = (w / 2.0 - r, h / 2.0 - r);
        let x0 = ((x.floor() as i32) - 1).max(0);
        let y0 = ((y.floor() as i32) - 1).max(0);
        let x1 = (((x + w).ceil() as i32) + 1).min(self.w);
        let y1 = (((y + h).ceil() as i32) + 1).min(self.h);
        let src = to_px(color);
        let (sr, sg, sb) = (((src >> 16) & 0xFF) as f32, ((src >> 8) & 0xFF) as f32, (src & 0xFF) as f32);
        let width = self.w;
        let buf = self.px();
        for py in y0..y1 {
            for pxl in x0..x1 {
                let dx = ((pxl as f32 + 0.5) - cx).abs() - hx;
                let dy = ((py as f32 + 0.5) - cy).abs() - hy;
                let (ox, oy) = (dx.max(0.0), dy.max(0.0));
                let d = (ox * ox + oy * oy).sqrt() + dx.max(dy).min(0.0) - r;
                let mut cov = (0.5 - d).clamp(0.0, 1.0);
                if stroke > 0.0 {
                    cov = (cov - (0.5 - (d + stroke)).clamp(0.0, 1.0)).max(0.0);
                }
                cov *= alpha;
                if cov <= 0.0 {
                    continue;
                }
                blend(&mut buf[(py * width + pxl) as usize], sr, sg, sb, cov);
            }
        }
    }

    pub fn fill_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32) {
        self.shape(x, y, w, h, r, 0.0, color, 1.0);
    }

    pub fn circle(&mut self, cx: f32, cy: f32, radius: f32, color: u32) {
        self.shape(cx - radius, cy - radius, radius * 2.0, radius * 2.0, radius, 0.0, color, 1.0);
    }

    /// The Jype "J" mark, `size` pixels square, in one colour.
    pub fn logo(&mut self, x: f32, y: f32, size: f32, color: u32, alpha: f32) {
        let src = to_px(color);
        let (sr, sg, sb) = (((src >> 16) & 0xFF) as f32, ((src >> 8) & 0xFF) as f32, (src & 0xFF) as f32);
        let x0 = (x.floor() as i32).max(0);
        let y0 = (y.floor() as i32).max(0);
        let x1 = ((x + size).ceil() as i32).min(self.w);
        let y1 = ((y + size).ceil() as i32).min(self.h);
        let width = self.w;
        let buf = self.px();
        let k = LOGO_N as f32 / size;
        let at = |ix: i32, iy: i32| -> f32 {
            let ix = ix.clamp(0, LOGO_N - 1);
            let iy = iy.clamp(0, LOGO_N - 1);
            LOGO[(iy * LOGO_N + ix) as usize] as f32 / 255.0
        };
        for py in y0..y1 {
            for pxl in x0..x1 {
                // bilinear sample of the 128x128 mask
                let u = ((pxl as f32 + 0.5 - x) * k - 0.5).max(0.0);
                let v = ((py as f32 + 0.5 - y) * k - 0.5).max(0.0);
                let (iu, iv) = (u.floor() as i32, v.floor() as i32);
                let (fu, fv) = (u - iu as f32, v - iv as f32);
                let a = at(iu, iv) * (1.0 - fu) * (1.0 - fv)
                    + at(iu + 1, iv) * fu * (1.0 - fv)
                    + at(iu, iv + 1) * (1.0 - fu) * fv
                    + at(iu + 1, iv + 1) * fu * fv;
                let cov = a * alpha;
                if cov > 0.003 {
                    blend(&mut buf[(py * width + pxl) as usize], sr, sg, sb, cov);
                }
            }
        }
    }

    pub unsafe fn text(&self, font: HFONT, s: &str, rect: RECT, color: u32, flags: u32) {
        let old = SelectObject(self.dc, font);
        SetBkMode(self.dc, TRANSPARENT as i32);
        SetTextColor(self.dc, color);
        let w: Vec<u16> = s.encode_utf16().collect();
        let mut r = rect;
        DrawTextW(self.dc, w.as_ptr(), w.len() as i32, &mut r, flags | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(self.dc, old);
    }

    pub unsafe fn text_width(&self, font: HFONT, s: &str) -> i32 {
        let old = SelectObject(self.dc, font);
        let w: Vec<u16> = s.encode_utf16().collect();
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        DrawTextW(self.dc, w.as_ptr(), w.len() as i32, &mut r, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(self.dc, old);
        r.right - r.left
    }

    pub unsafe fn present(&self, dst: HDC) {
        BitBlt(dst, 0, 0, self.w, self.h, self.dc, 0, 0, SRCCOPY);
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bmp);
            DeleteDC(self.dc);
        }
    }
}

fn blend(dst: &mut u32, sr: f32, sg: f32, sb: f32, cov: f32) {
    let (dr, dg, db) = (((*dst >> 16) & 0xFF) as f32, ((*dst >> 8) & 0xFF) as f32, (*dst & 0xFF) as f32);
    let inv = 1.0 - cov;
    let r = (sr * cov + dr * inv).round() as u32;
    let g = (sg * cov + dg * inv).round() as u32;
    let b = (sb * cov + db * inv).round() as u32;
    *dst = 0xFF00_0000 | (r << 16) | (g << 8) | b;
}

pub fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
    RECT { left: l, top: t, right: r, bottom: b }
}

/// Smooth 0..1 easing (fast start, soft landing).
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}
