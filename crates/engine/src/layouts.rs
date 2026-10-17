//! The typing layouts Jype ships with.

use std::{fs, io, path::{Path, PathBuf}};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Layout {
    AvroPhonetic,
    Khipro,
    Probhat,
    NationalJatiya,
    MunirOptima,
    Borno,
    AvroEasy,
}

impl Layout {
    /// Layouts offered in the menu. Probhat comes from Riti's own (MPL-2.0) data.
    /// The other fixed layouts are taken from the GPLv3 OpenBangla Keyboard
    /// project and are only included with the `extra-layouts` feature.
    #[cfg(feature = "extra-layouts")]
    pub const ALL: &'static [Layout] = &[
        Layout::AvroPhonetic,
        Layout::Khipro,
        Layout::Probhat,
        Layout::NationalJatiya,
        Layout::MunirOptima,
        Layout::Borno,
        Layout::AvroEasy,
    ];
    #[cfg(not(feature = "extra-layouts"))]
    pub const ALL: &'static [Layout] = &[Layout::AvroPhonetic, Layout::Khipro, Layout::Probhat];

    /// Stable identifier stored in the config file.
    pub fn id(self) -> &'static str {
        match self {
            Layout::AvroPhonetic => "avro_phonetic",
            Layout::Khipro => "khipro",
            Layout::Probhat => "probhat",
            Layout::NationalJatiya => "national_jatiya",
            Layout::MunirOptima => "munir_optima",
            Layout::Borno => "borno",
            Layout::AvroEasy => "avro_easy",
        }
    }

    pub fn from_id(id: &str) -> Option<Layout> {
        Layout::ALL.iter().copied().find(|l| l.id() == id)
    }

    /// Human readable name shown in menus.
    pub fn label(self) -> &'static str {
        match self {
            Layout::AvroPhonetic => "Phonetic (Avro style) - recommended",
            Layout::Khipro => "Khipro (phonetic)",
            Layout::Probhat => "Probhat",
            Layout::NationalJatiya => "National (Jatiya)",
            Layout::MunirOptima => "Munir Optima",
            Layout::Borno => "Borno",
            Layout::AvroEasy => "Avro Easy",
        }
    }

    /// Phonetic layouts type words by sound ("ami" -> আমি) and offer suggestions.
    pub fn is_phonetic(self) -> bool {
        matches!(self, Layout::AvroPhonetic | Layout::Khipro)
    }

    /// Embedded layout file contents for fixed layouts.
    fn file(self) -> Option<(&'static str, &'static str)> {
        match self {
            Layout::AvroPhonetic | Layout::Khipro => None,
            Layout::Probhat => Some((
                "Probhat.json",
                include_str!("../../../third_party/riti/data/Probhat.json"),
            )),
            #[cfg(feature = "extra-layouts")]
            Layout::NationalJatiya => Some((
                "National_Jatiya.json",
                include_str!("../../../layouts/National_Jatiya.json"),
            )),
            #[cfg(feature = "extra-layouts")]
            Layout::MunirOptima => Some((
                "Munir_Optima.json",
                include_str!("../../../layouts/Munir_Optima.json"),
            )),
            #[cfg(feature = "extra-layouts")]
            Layout::Borno => Some(("Borno.json", include_str!("../../../layouts/Borno.json"))),
            #[cfg(feature = "extra-layouts")]
            Layout::AvroEasy => Some((
                "Avro_Easy.json",
                include_str!("../../../layouts/Avro_Easy.json"),
            )),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// The string Riti wants as its "layout file path".
    /// Phonetic layouts use special names; fixed layouts use a real path, which
    /// we create from the embedded copy so the app is a single self-contained exe.
    pub fn riti_path(self, data_dir: &Path) -> io::Result<String> {
        match self {
            Layout::AvroPhonetic => Ok("avro_phonetic".into()),
            Layout::Khipro => Ok("khipro".into()),
            _ => {
                let (name, contents) = self.file().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "layout not included in this build")
                })?;
                let dir: PathBuf = data_dir.join("layouts");
                fs::create_dir_all(&dir)?;
                let path = dir.join(name);
                let needs_write = fs::read_to_string(&path).map(|c| c != contents).unwrap_or(true);
                if needs_write {
                    fs::write(&path, contents)?;
                }
                Ok(path.to_string_lossy().into_owned())
            }
        }
    }
}
