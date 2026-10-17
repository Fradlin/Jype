//! English words typed in English spelling -> Bangla transliteration.
//!
//! Riti spells unknown words letter by letter, so "customized" becomes
//! চুস্তমিযেদ. This module knows how English words are *pronounced* (table
//! generated from the CMU Pronouncing Dictionary by `tools/gen_loanwords.py`)
//! and gives কাস্টমাইজড instead. Users can add their own words in
//! `loanwords.txt` in the Jype settings folder.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

static BUILTIN: &str = include_str!("../data/loanwords.tsv");
static TABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
// Hand-checked everyday words, brands and Bangladeshi services (same file the
// table generator uses). These lead the suggestion list.
static CURATED_SRC: &str = include_str!("../../../tools/curated.tsv");
static CURATED: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

const USER_FILE_HELP: &str = "# Jype - your own English -> Bangla words\n\
# One word per line:  english-word  \u{09AC}\u{09BE}\u{0982}\u{09B2}\u{09BE}\n\
# Example:            customized  \u{0995}\u{09BE}\u{09B8}\u{09CD}\u{099F}\u{09AE}\u{09BE}\u{0987}\u{099C}\u{09A1}\n\
# Lines starting with # are ignored. Your words always come first in the list.\n";

fn table() -> &'static HashMap<&'static str, &'static str> {
    TABLE.get_or_init(|| BUILTIN.lines().filter_map(|l| l.split_once('\t')).collect())
}

fn curated() -> &'static HashMap<&'static str, &'static str> {
    CURATED.get_or_init(|| {
        CURATED_SRC
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once('\t'))
            .map(|(w, b)| (w.trim(), b.trim()))
            .collect()
    })
}

/// Build the lookup table now, so the first key press doesn't pay for it.
pub fn warm_up() {
    let _ = table();
    let _ = curated();
}

pub struct Loanwords {
    user: HashMap<String, String>,
    file: Option<PathBuf>,
}

/// Where a reading came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    User,
    /// Hand-checked list shipped with Jype.
    Curated,
    Builtin,
}

impl Loanwords {
    /// Load the user's list; creates an explanatory template on first run.
    pub fn load(user_file: &Path) -> Loanwords {
        let mut user = HashMap::new();
        match fs::read_to_string(user_file) {
            Ok(text) => {
                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    let split = line
                        .split_once('\t')
                        .or_else(|| line.split_once('='))
                        .or_else(|| line.split_once(' '));
                    if let Some((w, b)) = split {
                        let (w, b) = (w.trim().to_lowercase(), b.trim());
                        if !w.is_empty() && !b.is_empty() {
                            user.insert(w, b.to_string());
                        }
                    }
                }
            }
            Err(_) => {
                let _ = fs::write(user_file, USER_FILE_HELP);
            }
        }
        Loanwords { user, file: Some(user_file.to_path_buf()) }
    }

    /// A list with no user file (used by tests).
    pub fn empty() -> Loanwords {
        Loanwords { user: HashMap::new(), file: None }
    }

    /// The user's own words, sorted by English spelling.
    pub fn user_entries(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self.user.iter().map(|(a, b)| (a.clone(), b.clone())).collect();
        v.sort();
        v
    }

    /// Add or replace one of the user's words and save the file.
    pub fn set_user(&mut self, english: &str, bangla: &str) -> Result<(), String> {
        let w = english.trim().to_lowercase();
        let b = bangla.trim().to_string();
        if w.len() < 2 || !w.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err("The English word must be letters a-z only (at least 2).".into());
        }
        if b.is_empty() || b.chars().any(|c| c.is_whitespace()) {
            return Err("Type the Bangla spelling as one word.".into());
        }
        self.user.insert(w, b);
        self.save()
    }

    /// Remove one of the user's words and save the file.
    pub fn remove_user(&mut self, english: &str) -> Result<(), String> {
        self.user.remove(&english.trim().to_lowercase());
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.file else { return Ok(()) };
        let mut text = String::from(USER_FILE_HELP);
        for (w, b) in self.user_entries() {
            text.push_str(&format!("{w}\t{b}\n"));
        }
        fs::write(path, text).map_err(|e| format!("Could not save your words: {e}"))
    }

    /// Bangla reading of a plain lowercase English word.
    pub fn lookup(&self, word: &str) -> Option<(String, Source)> {
        let key = word.to_lowercase();
        if let Some(b) = self.user.get(&key) {
            return Some((b.clone(), Source::User));
        }
        if let Some(b) = curated().get(key.as_str()) {
            return Some((b.to_string(), Source::Curated));
        }
        table().get(key.as_str()).map(|b| (b.to_string(), Source::Builtin))
    }

    /// Reading for what the user typed so far, e.g. `customized.` -> `কাস্টমাইজড।`.
    /// Returns None unless the text is one plain English word (lowercase or
    /// Capitalised) with optional punctuation around it: Avro-style capitals
    /// such as `kShoma` are phonetic input, never English.
    pub fn reading_for_typed(&self, typed: &str) -> Option<(String, Source, usize)> {
        if !typed.is_ascii() {
            return None;
        }
        let start = typed.find(|c: char| c.is_ascii_alphabetic())?;
        let end = typed.rfind(|c: char| c.is_ascii_alphabetic())? + 1;
        let word = &typed[start..end];
        if word.len() < 2 || !word.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let mut cs = word.chars();
        let first = cs.next()?;
        if !cs.all(|c| c.is_ascii_lowercase()) {
            return None; // mixed capitals = Avro phonetic input
        }
        let _ = first; // lowercase or Capitalised are both fine
        let (bn, src) = self.lookup(word)?;
        let map_punct = |s: &str| -> String {
            s.chars().map(|c| if c == '.' { '\u{0964}' } else { c }).collect()
        };
        Some((format!("{}{}{}", map_punct(&typed[..start]), bn, map_punct(&typed[end..])), src, word.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_table_has_the_reported_words() {
        let l = Loanwords::empty();
        assert_eq!(l.lookup("customize").unwrap().0, "কাস্টমাইজ");
        assert_eq!(l.lookup("customized").unwrap().0, "কাস্টমাইজড");
        assert_eq!(l.lookup("Computer").unwrap().0, "কম্পিউটার");
        assert!(table().len() > 100_000);
    }

    #[test]
    fn fixed_common_words() {
        let l = Loanwords::empty();
        for (w, bn) in [
            ("order", "অর্ডার"),
            ("bkash", "বিকাশ"),
            ("market", "মার্কেট"),
            ("payment", "পেমেন্ট"),
            ("rocket", "রকেট"),
            ("nagad", "নগদ"),
        ] {
            assert_eq!(l.lookup(w).unwrap().0, bn, "{w}");
        }
    }

    #[test]
    fn personal_dictionary_roundtrip() {
        let dir = std::env::temp_dir().join(format!("jype-dict-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let file = dir.join("loanwords.txt");
        let _ = fs::remove_file(&file);
        let mut l = Loanwords::load(&file);
        assert!(l.set_user("x", "এক্স").is_err());
        assert!(l.set_user("Zoom", "জুম").is_ok());
        assert_eq!(l.lookup("zoom").unwrap(), ("জুম".to_string(), Source::User));
        let again = Loanwords::load(&file);
        assert_eq!(again.user_entries(), vec![("zoom".to_string(), "জুম".to_string())]);
        let mut again = again;
        again.remove_user("zoom").unwrap();
        assert!(Loanwords::load(&file).user_entries().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mixed_capitals_are_not_english() {
        let l = Loanwords::empty();
        assert!(l.reading_for_typed("kShoma").is_none());
        assert!(l.reading_for_typed("customized").is_some());
        assert!(l.reading_for_typed("Customized").is_some());
        assert_eq!(l.reading_for_typed("customized.").unwrap().0, "কাস্টমাইজড।");
    }
}
