use std::{fs, path::PathBuf};

use bangla_engine::Layout;

/// Key combination that switches between Bangla and English typing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hotkey {
    CtrlSpace,
    CtrlAltSpace,
    CtrlAltB,
}

impl Hotkey {
    pub const ALL: [Hotkey; 3] = [Hotkey::CtrlSpace, Hotkey::CtrlAltSpace, Hotkey::CtrlAltB];

    pub fn id(self) -> &'static str {
        match self {
            Hotkey::CtrlSpace => "ctrl+space",
            Hotkey::CtrlAltSpace => "ctrl+alt+space",
            Hotkey::CtrlAltB => "ctrl+alt+b",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Hotkey::CtrlSpace => "Ctrl + Space",
            Hotkey::CtrlAltSpace => "Ctrl + Alt + Space",
            Hotkey::CtrlAltB => "Ctrl + Alt + B",
        }
    }
    pub fn from_id(id: &str) -> Option<Hotkey> {
        Hotkey::ALL.into_iter().find(|h| h.id() == id)
    }

    pub fn matches(self, vk: u32, ctrl: bool, alt: bool, shift: bool) -> bool {
        const SPACE: u32 = 0x20;
        const B: u32 = 0x42;
        match self {
            Hotkey::CtrlSpace => vk == SPACE && ctrl && !alt && !shift,
            Hotkey::CtrlAltSpace => vk == SPACE && ctrl && alt,
            Hotkey::CtrlAltB => vk == B && ctrl && alt,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub layout: Layout,
    pub include_english: bool,
    pub hotkey: Hotkey,
    pub bangla_on: bool,
    /// How many more words to show the "Space = accept" hint line for.
    pub hints_left: u32,
    /// True until the welcome balloon has been shown once.
    pub first_run: bool,
    /// Show the small floating Jype bar.
    pub bar_visible: bool,
    /// Last position of the bar (screen pixels), if the user moved it.
    pub bar_pos: Option<(i32, i32)>,
    /// Id of the Bangla font (see `gfx::FONTS`).
    pub font: String,
    /// Bangla text size: 0 small, 1 medium, 2 large.
    pub font_size: u8,
    /// 0 follow Windows, 1 light, 2 dark.
    pub theme: u8,
    /// Index into `gfx::ACCENTS`.
    pub accent: u8,
    /// The floating bar shows its icon buttons.
    pub bar_expanded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            layout: Layout::AvroPhonetic,
            include_english: false,
            hotkey: Hotkey::CtrlSpace,
            bangla_on: true,
            hints_left: 25,
            first_run: true,
            bar_visible: true,
            bar_pos: None,
            font: "kohinoor".to_string(),
            font_size: 1,
            theme: 0,
            accent: 0,
            bar_expanded: false,
        }
    }
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Jype")
}

/// Early builds stored settings in %APPDATA%\BanglaType; move them over once.
pub fn migrate_old_data() {
    let new = data_dir();
    if new.exists() {
        return;
    }
    if let Some(old) = new.parent().map(|p| p.join("BanglaType")) {
        if old.exists() {
            let _ = fs::rename(old, new);
        }
    }
}

fn file() -> PathBuf {
    data_dir().join("settings.ini")
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Ok(text) = fs::read_to_string(file()) else {
            return s;
        };
        s.first_run = false;
        let (mut bar_x, mut bar_y): (Option<i32>, Option<i32>) = (None, None);
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "layout" => {
                    if let Some(l) = Layout::from_id(v) {
                        s.layout = l;
                    }
                }
                "include_english" => s.include_english = v == "1",
                "hotkey" => {
                    if let Some(h) = Hotkey::from_id(v) {
                        s.hotkey = h;
                    }
                }
                "bangla_on" => s.bangla_on = v != "0",
                "hints_left" => s.hints_left = v.parse().unwrap_or(0),
                "bar_visible" => s.bar_visible = v != "0",
                "font" => s.font = v.to_string(),
                "font_size" => s.font_size = v.parse::<u8>().unwrap_or(1).min(2),
                "theme" => s.theme = v.parse::<u8>().unwrap_or(0).min(2),
                "accent" => s.accent = v.parse::<u8>().unwrap_or(0).min(3),
                "bar_expanded" => s.bar_expanded = v == "1",
                "bar_x" => bar_x = v.parse().ok(),
                "bar_y" => bar_y = v.parse().ok(),
                _ => {}
            }
        }
        if let (Some(x), Some(y)) = (bar_x, bar_y) {
            s.bar_pos = Some((x, y));
        }
        s
    }

    pub fn save(&self) {
        let _ = fs::create_dir_all(data_dir());
        let mut text = format!(
            "layout={}\ninclude_english={}\nhotkey={}\nbangla_on={}\nhints_left={}\nbar_visible={}\nfont={}\nfont_size={}\ntheme={}\naccent={}\nbar_expanded={}\n",
            self.layout.id(),
            self.include_english as u8,
            self.hotkey.id(),
            self.bangla_on as u8,
            self.hints_left,
            self.bar_visible as u8,
            self.font,
            self.font_size,
            self.theme,
            self.accent,
            self.bar_expanded as u8,
        );
        if let Some((x, y)) = self.bar_pos {
            text.push_str(&format!("bar_x={x}\nbar_y={y}\n"));
        }
        let _ = fs::write(file(), text);
    }
}
