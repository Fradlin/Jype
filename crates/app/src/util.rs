use std::{mem::size_of, ptr::null_mut};

use windows_sys::Win32::{
    Foundation::ERROR_SUCCESS,
    System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
    UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK},
};

/// UTF-16, NUL terminated.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Win32 COLORREF (0x00BBGGRR).
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// Copy `s` into a fixed-size NUL-terminated UTF-16 buffer (for NOTIFYICONDATA etc).
pub fn fill_wide<const N: usize>(dst: &mut [u16; N], s: &str) {
    let mut i = 0;
    for u in s.encode_utf16() {
        if i >= N - 1 {
            break;
        }
        dst[i] = u;
        i += 1;
    }
    dst[i] = 0;
}

/// True when Windows is set to dark app mode.
pub fn system_uses_dark() -> bool {
    unsafe {
        let key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let name = wide("AppsUseLightTheme");
        let mut data: u32 = 1;
        let mut size = size_of::<u32>() as u32;
        let rc = RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            &mut data as *mut u32 as *mut _,
            &mut size,
        );
        rc == ERROR_SUCCESS && data == 0
    }
}

pub fn message_box(title: &str, text: &str, error: bool) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(text).as_ptr(),
            wide(title).as_ptr(),
            MB_OK | if error { MB_ICONERROR } else { MB_ICONINFORMATION },
        );
    }
}
