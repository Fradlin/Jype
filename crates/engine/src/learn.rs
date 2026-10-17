//! Remembers, on this PC only, which suggestion was picked for each typed
//! word, so the same choice is offered first next time.
//!
//! Nothing here ever leaves the PC by itself. [`Learn::export`] writes a
//! plain-text copy of this file for the *person using Jype* to send
//! somewhere themselves, if they choose to - Jype has no network code at all.

use std::{collections::HashMap, fs, path::PathBuf};

const HEADER: &str = "# Jype: words you've typed and the suggestion you picked, with how many times.\n\
     # Stored only on this PC. Nothing here is ever sent anywhere by Jype itself.\n\
     # word<TAB>suggestion<TAB>times\n";

pub struct Learn {
    // typed word (lowercase) -> { suggestion -> times picked }
    counts: HashMap<String, HashMap<String, u32>>,
    file: Option<PathBuf>,
}

/// Words we don't learn from: anything that isn't a plain short word, since
/// that is more likely to be sensitive (an email, a code, a number) than a
/// genuine spelling choice.
fn eligible(word: &str) -> bool {
    let n = word.chars().count();
    (2..=24).contains(&n) && word.chars().all(|c| c.is_ascii_alphabetic())
}

impl Learn {
    pub fn load(file: &std::path::Path) -> Learn {
        let mut counts: HashMap<String, HashMap<String, u32>> = HashMap::new();
        if let Ok(text) = fs::read_to_string(file) {
            for line in text.lines() {
                if line.starts_with('#') || line.trim().is_empty() {
                    continue;
                }
                let mut parts = line.splitn(3, '\t');
                if let (Some(w), Some(s), Some(n)) = (parts.next(), parts.next(), parts.next()) {
                    if let Ok(n) = n.trim().parse::<u32>() {
                        counts.entry(w.to_string()).or_default().insert(s.to_string(), n);
                    }
                }
            }
        }
        Learn { counts, file: Some(file.to_path_buf()) }
    }

    pub fn empty() -> Learn {
        Learn { counts: HashMap::new(), file: None }
    }

    /// Record that `suggestion` was chosen for `word` (saves at once). The
    /// word is kept exactly as typed: in Avro phonetic, case can change the
    /// result ("bhalobashi" vs "bhalObashi" are different words), so
    /// lower-casing it here would make Jype re-apply the wrong one.
    pub fn record(&mut self, word: &str, suggestion: &str) {
        if !eligible(word) || suggestion.is_empty() {
            return;
        }
        let word = word.to_string();
        *self.counts.entry(word).or_default().entry(suggestion.to_string()).or_insert(0) += 1;
        let _ = self.save();
    }

    /// The suggestion picked most often for `word`, if any.
    pub fn best(&self, word: &str) -> Option<String> {
        self.counts.get(word)?.iter().max_by_key(|(_, n)| **n).map(|(s, _)| s.clone())
    }

    /// Everything learned, for display in Studio: (word, [(suggestion, times), ..]),
    /// sorted by word, each word's suggestions sorted by times (most first).
    pub fn entries(&self) -> Vec<(String, Vec<(String, u32)>)> {
        let mut words: Vec<(String, Vec<(String, u32)>)> = self
            .counts
            .iter()
            .map(|(w, m)| {
                let mut v: Vec<(String, u32)> = m.iter().map(|(s, n)| (s.clone(), *n)).collect();
                v.sort_by(|a, b| b.1.cmp(&a.1));
                (w.clone(), v)
            })
            .collect();
        words.sort_by(|a, b| a.0.cmp(&b.0));
        words
    }

    pub fn remove(&mut self, word: &str) -> Result<(), String> {
        self.counts.remove(word);
        self.save()
    }

    pub fn clear(&mut self) -> Result<(), String> {
        self.counts.clear();
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.file else { return Ok(()) };
        let mut text = String::from(HEADER);
        for (w, suggestions) in self.entries() {
            for (s, n) in suggestions {
                text.push_str(&format!("{w}\t{s}\t{n}\n"));
            }
        }
        fs::write(path, text).map_err(|e| format!("Could not save your learned words: {e}"))
    }

    /// Write a copy of the learned-word file to `dest`, for the person using
    /// Jype to send themselves - Jype never sends this anywhere on its own.
    pub fn export(&self, dest: &std::path::Path) -> Result<(), String> {
        let mut text = String::from(
            "# Jype word data - exported by you, for you to share however you like.\n\
             # This file never leaves your PC unless you send it yourself.\n\
             # word<TAB>suggestion<TAB>times\n",
        );
        for (w, suggestions) in self.entries() {
            for (s, n) in suggestions {
                text.push_str(&format!("{w}\t{s}\t{n}\n"));
            }
        }
        fs::write(dest, text).map_err(|e| format!("Could not write the export file: {e}"))
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_the_most_picked_suggestion() {
        let dir = std::env::temp_dir().join(format!("jype-learn-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let file = dir.join("learned.tsv");
        let _ = fs::remove_file(&file);

        let mut l = Learn::load(&file);
        assert_eq!(l.best("ami"), None);
        l.record("ami", "আমি");
        l.record("ami", "আমি");
        l.record("ami", "এমআই");
        assert_eq!(l.best("ami"), Some("আমি".to_string()));

        // persists across a reload
        let l2 = Learn::load(&file);
        assert_eq!(l2.best("ami"), Some("আমি".to_string()));
        assert_eq!(l2.entries(), vec![("ami".to_string(), vec![("আমি".to_string(), 2), ("এমআই".to_string(), 1)])]);

        let mut l3 = Learn::load(&file);
        l3.remove("ami").unwrap();
        assert!(Learn::load(&file).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_things_that_are_not_plain_short_words() {
        let mut l = Learn::empty();
        l.record("a@b.com", "x");
        l.record("12345678", "x");
        l.record("a", "x");
        assert!(l.is_empty());
    }
}
