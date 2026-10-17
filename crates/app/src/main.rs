//! Jype - Bangla typing for Windows 10/11.
//!
//! Architecture (Phase 1): a tray application. A low-level keyboard hook feeds
//! key presses to the Riti-based engine (`bangla-engine`); the word being typed
//! is shown in a candidate popup next to the text cursor, and the chosen word is
//! inserted into whatever app has focus with `SendInput`.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod bar;
mod gfx;
mod hook;
mod inject;
mod popup;
mod settings;
mod studio;
mod tray;
mod util;

fn main() {
    app::run();
}
