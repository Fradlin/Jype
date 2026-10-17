//! Jype engine: turns raw key presses into Bangla text.
//!
//! This crate knows nothing about Windows. The Windows app feeds it key events
//! and acts on the [`Reply`] it returns. Keeping it separate means the whole
//! typing logic is unit-tested on any OS.
//!
//! Model (same as Avro): while you type a word it is *not* inserted into the
//! target app. It lives in a small candidate popup. Space/Enter/punctuation
//! commits the highlighted candidate into the app.

pub mod keymap;
pub mod layouts;
pub mod loanwords;
pub mod learn;

use std::{
    fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
};

use riti::{config::Config, context::RitiContext, context::MODIFIER_ALT_GR, suggestion::Suggestion};

pub use layouts::Layout;
use learn::Learn;
use loanwords::{Loanwords, Source};
use keymap::*;

/// One physical key press, as reported by the OS.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyInput {
    /// Windows virtual-key code.
    pub vk: u32,
    pub shift: bool,
    pub caps: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Right Alt, used as AltGr by fixed layouts.
    pub altgr: bool,
    pub win: bool,
}

/// What the OS layer should do with the key it just gave us.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    /// Not ours: let the key through to the application untouched.
    Pass,
    /// Ours: swallow the key (the popup may have changed, see [`Engine::view`]).
    Consume,
    /// Insert `text` into the application, then do `then`.
    Commit { text: String, then: Then },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Then {
    Nothing,
    /// Follow the text with a space.
    Space,
    /// Re-send this key (e.g. Enter, an arrow, Ctrl+V) after the text.
    Replay(u32),
}

/// What the candidate popup should show. `None` from [`Engine::view`] = hidden.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// The Latin text typed so far (e.g. "ami"), empty for fixed layouts.
    pub typed: String,
    pub candidates: Vec<String>,
    pub selected: usize,
}

/// Marks a candidate that Riti does not know about (our English reading).
const SYNTHETIC: usize = usize::MAX;

pub struct Engine {
    ctx: RitiContext,
    config: Config,
    layout: Layout,
    data_dir: PathBuf,
    include_english: bool,
    view: View,
    /// display index -> Riti's own candidate index (SYNTHETIC for ones we added)
    order: Vec<usize>,
    loan: Loanwords,
    learn: Learn,
    composing: bool,
}

#[derive(Debug)]
pub struct EngineError(pub String);
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EngineError {}

fn build_config(data_dir: &Path, layout: Layout, include_english: bool) -> Result<Config, EngineError> {
    fs::create_dir_all(data_dir).map_err(|e| EngineError(format!("cannot create data dir: {e}")))?;
    let mut config = Config::default();
    let dir = data_dir.to_string_lossy();
    if !config.set_user_dir(&dir) {
        return Err(EngineError(format!("bad user dir {dir}")));
    }
    // Riti ships its dictionaries inside the library; database_dir is only
    // required to exist.
    config.set_database_dir(&dir);
    let path = layout
        .riti_path(data_dir)
        .map_err(|e| EngineError(format!("cannot write layout file: {e}")))?;
    if !config.set_layout_file_path(&path) {
        return Err(EngineError(format!("layout not accepted: {path}")));
    }
    // Dictionary suggestions give the candidate list.
    config.set_phonetic_suggestion(layout.is_phonetic());
    config.set_suggestion_include_english(include_english);
    Ok(config)
}

impl Engine {
    pub fn new(data_dir: &Path, layout: Layout, include_english: bool) -> Result<Engine, EngineError> {
        let config = build_config(data_dir, layout, include_english)?;
        loanwords::warm_up();
        let ctx = RitiContext::new_with_config(&config);
        Ok(Engine {
            ctx,
            config,
            layout,
            data_dir: data_dir.to_path_buf(),
            include_english,
            view: View::default(),
            order: Vec::new(),
            loan: Loanwords::load(&data_dir.join("loanwords.txt")),
            learn: Learn::load(&data_dir.join("learned.tsv")),
            composing: false,
        })
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn set_layout(&mut self, layout: Layout) -> Result<(), EngineError> {
        let config = build_config(&self.data_dir, layout, self.include_english)?;
        // A fresh context is the simplest way to guarantee a clean switch.
        self.ctx = RitiContext::new_with_config(&config);
        self.config = config;
        self.layout = layout;
        self.loan = Loanwords::load(&self.data_dir.join("loanwords.txt"));
        self.reset();
        Ok(())
    }

    pub fn set_include_english(&mut self, include: bool) -> Result<(), EngineError> {
        self.include_english = include;
        self.set_layout(self.layout)
    }

    /// The user's own English -> Bangla words (personal dictionary), sorted.
    pub fn user_words(&self) -> Vec<(String, String)> {
        self.loan.user_entries()
    }

    /// Add (or change) a personal-dictionary word. Saved to loanwords.txt at once.
    pub fn add_user_word(&mut self, english: &str, bangla: &str) -> Result<(), String> {
        self.loan.set_user(english, bangla)
    }

    /// Remove a personal-dictionary word.
    pub fn remove_user_word(&mut self, english: &str) -> Result<(), String> {
        self.loan.remove_user(english)
    }

    /// Everything learned so far: (word, [(suggestion, times picked), ..]).
    pub fn learned_words(&self) -> Vec<(String, Vec<(String, u32)>)> {
        self.learn.entries()
    }

    /// Forget one learned word.
    pub fn forget_word(&mut self, word: &str) -> Result<(), String> {
        self.learn.remove(word)
    }

    /// Forget everything learned.
    pub fn forget_all(&mut self) -> Result<(), String> {
        self.learn.clear()
    }

    pub fn has_learned_words(&self) -> bool {
        !self.learn.is_empty()
    }

    /// Write a copy of the learned-word file for the person to send
    /// themselves; Jype itself never transmits this.
    pub fn export_learned(&self, dest: &Path) -> Result<(), String> {
        self.learn.export(dest)
    }

    /// `Some` while a word is being composed.
    pub fn view(&self) -> Option<&View> {
        if self.composing && !self.view.candidates.is_empty() {
            Some(&self.view)
        } else {
            None
        }
    }

    pub fn is_composing(&self) -> bool {
        self.composing
    }

    /// Throw away the word in progress without typing anything.
    pub fn reset(&mut self) {
        let _ = self.guard(|e| e.ctx.finish_input_session());
        self.composing = false;
        self.view = View::default();
        self.order.clear();
    }

    /// Run engine code; if Riti ever panics, rebuild it instead of crashing the host.
    fn guard<T>(&mut self, f: impl FnOnce(&mut Engine) -> T) -> Option<T> {
        match catch_unwind(AssertUnwindSafe(|| f(self))) {
            Ok(v) => Some(v),
            Err(_) => {
                self.ctx = RitiContext::new_with_config(&self.config);
                self.composing = false;
                self.view = View::default();
                self.order.clear();
                None
            }
        }
    }

    fn apply(&mut self, s: Suggestion, typed_hint: Option<String>) {
        match s {
            Suggestion::Full { auxiliary, suggestions, selection, .. } => {
                // (text, Riti's own index). Real Bangla words first, emoji last
                // (Riti ranks an exact emoji-name match first, which surprises).
                let mut entries: Vec<(String, usize)> =
                    suggestions.iter().cloned().enumerate().map(|(i, t)| (t, i)).collect();
                entries.sort_by_key(|(t, _)| is_emoji_only(t));

                // English word typed in English spelling: add its Bangla reading.
                if self.layout.is_phonetic() {
                    if let Some((bn, src, _letters)) = self.loan.reading_for_typed(&auxiliary) {
                        let bangla = |t: &str| t.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c));
                        // Candidates in Riti's Bangla dictionary. The plain letter-by-letter
                        // spelling is always listed too, so it does not count as a hit.
                        let dictionary_hits =
                            entries.iter().filter(|(t, _)| bangla(t)).count().saturating_sub(1);
                        // If Riti knows a real Bangla word for what was typed, that stays
                        // first (bangla = বাংলা, hat = হাট, to = তো) and the English reading
                        // is the next choice. If Riti knows nothing (customized), the English
                        // reading goes first - the letter-by-letter guess is only ever junk.
                        // The user's own words and the hand-checked list always win.
                        let english_first = src != Source::Builtin || dictionary_hits == 0;
                        let existing = entries.iter().position(|(t, _)| *t == bn);
                        match existing {
                            // Already one of Riti's candidates: only move it to the front
                            // when English should lead; never push Riti's own words down.
                            Some(p) if english_first => {
                                let item = entries.remove(p);
                                entries.insert(0, item);
                            }
                            Some(_) => {}
                            None => {
                                let at = if english_first { 0 } else { 1.min(entries.len()) };
                                entries.insert(at, (bn, SYNTHETIC));
                            }
                        }
                    }
                }

                // Honour a choice *this* user made earlier for this exact word (on-device
                // learning; never shared unless the user exports it themselves).
                if self.layout.is_phonetic() {
                    if let Some(pref) = self.learn.best(&auxiliary) {
                        if let Some(p) = entries.iter().position(|(t, _)| *t == pref) {
                            if p != 0 {
                                let item = entries.remove(p);
                                entries.insert(0, item);
                            }
                        }
                    }
                }

                // Honour a choice the user made earlier for this word (Riti remembers it).
                let selected = if selection == 0 {
                    0
                } else {
                    entries.iter().position(|(_, i)| *i == selection).unwrap_or(0)
                };
                self.order = entries.iter().map(|(_, i)| *i).collect();
                let candidates = entries.into_iter().map(|(t, _)| t).collect();
                self.view = View { typed: auxiliary, candidates, selected };
            }
            Suggestion::Single { suggestion, .. } => {
                self.order = vec![0];
                self.view = View {
                    typed: typed_hint.unwrap_or_default(),
                    candidates: if suggestion.is_empty() { vec![] } else { vec![suggestion] },
                    selected: 0,
                };
            }
        }
        self.composing = self.ctx.ongoing_input_session() && !self.view.candidates.is_empty();
        if !self.composing {
            self.view = View::default();
            self.order.clear();
        }
    }

    /// Riti's index for the highlighted candidate.
    fn riti_index(&self) -> usize {
        self.order.get(self.view.selected).copied().unwrap_or(0)
    }

    /// Index to hand back to Riti as "the previous selection"; 0 for our own candidates.
    fn riti_hint(&self) -> u8 {
        match self.riti_index() {
            SYNTHETIC => 0,
            i => i.min(255) as u8,
        }
    }

    fn current_text(&self) -> String {
        self.view
            .candidates
            .get(self.view.selected)
            .cloned()
            .unwrap_or_else(|| self.view.typed.clone())
    }

    /// Commit the highlighted candidate and end the session.
    fn commit(&mut self, then: Then) -> Reply {
        let text = self.current_text();
        if self.layout.is_phonetic() && !self.view.typed.is_empty() {
            self.learn.record(&self.view.typed, &text);
        }
        let idx = self.riti_index();
        if idx == SYNTHETIC {
            // Not one of Riti's candidates: just close its session.
            let _ = self.guard(|e| e.ctx.finish_input_session());
        } else {
            let _ = self.guard(|e| e.ctx.candidate_committed(idx));
        }
        self.composing = false;
        self.view = View::default();
        self.order.clear();
        Reply::Commit { text, then }
    }

    /// Finish the word in progress (if any) and return its text, e.g. when the
    /// user switches typing mode in the middle of a word.
    pub fn finish(&mut self) -> Option<String> {
        if !self.composing {
            return None;
        }
        match self.commit(Then::Nothing) {
            Reply::Commit { text, .. } => Some(text),
            _ => None,
        }
    }

    /// Commit candidate `index` (used when the user clicks a candidate).
    pub fn commit_candidate(&mut self, index: usize) -> Reply {
        if !self.composing || index >= self.view.candidates.len() {
            return Reply::Pass;
        }
        self.view.selected = index;
        self.commit(Then::Nothing)
    }

    fn move_selection(&mut self, delta: isize) {
        let n = self.view.candidates.len() as isize;
        if n > 0 {
            self.view.selected = (self.view.selected as isize + delta).rem_euclid(n) as usize;
        }
    }

    /// Feed one key-down event.
    pub fn handle_key(&mut self, k: KeyInput) -> Reply {
        // Shortcuts (Ctrl+C, Alt+Tab, Win+E ...) are never typing. If a word is
        // in progress, finish it first so nothing is lost, then replay the key.
        if k.ctrl || k.alt || k.win {
            if self.composing {
                return self.commit(Then::Replay(k.vk));
            }
            return Reply::Pass;
        }

        if self.composing {
            match k.vk {
                VK_SPACE => return self.commit(Then::Space),
                VK_RETURN => return self.commit(Then::Replay(k.vk)),
                VK_ESCAPE => {
                    self.reset();
                    return Reply::Consume;
                }
                VK_BACK => {
                    let s = self.guard(|e| e.ctx.backspace_event(false));
                    match s {
                        Some(s) => {
                            let typed = self.view.typed.clone();
                            // Keep a plausible "typed" echo for fixed layouts.
                            self.apply(s, Some(typed));
                        }
                        None => {}
                    }
                    return Reply::Consume;
                }
                VK_DOWN => {
                    self.move_selection(1);
                    return Reply::Consume;
                }
                VK_UP => {
                    self.move_selection(-1);
                    return Reply::Consume;
                }
                VK_TAB => {
                    self.move_selection(if k.shift { -1 } else { 1 });
                    return Reply::Consume;
                }
                VK_LEFT | VK_RIGHT | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_DELETE => {
                    return self.commit(Then::Replay(k.vk));
                }
                _ => {}
            }
        }

        // Is it a typing key?
        let Some(vc) = vk_to_vc(k.vk, k.shift, k.caps) else {
            // F-keys, media keys, etc.
            if self.composing {
                return self.commit(Then::Replay(k.vk));
            }
            return Reply::Pass;
        };

        let modifier = if k.altgr { MODIFIER_ALT_GR } else { 0 };
        let selected = self.riti_hint();
        let before = self.view.typed.clone();
        let latin = latin_of(k);

        let s = self.guard(|e| e.ctx.get_suggestion_for_key(vc, modifier, selected));
        match s {
            Some(s) => {
                let hint = if self.layout.is_phonetic() {
                    None
                } else {
                    Some(format!("{before}{}", latin.unwrap_or_default()))
                };
                self.apply(s, hint);
                if self.composing {
                    Reply::Consume
                } else {
                    // The engine produced nothing; don't swallow the key.
                    Reply::Pass
                }
            }
            None => Reply::Pass,
        }
    }
}

/// True for suggestions that are pictographs (🇧🇩 ❤ ⛵ 🆖) rather than Bangla text.
fn is_emoji_only(s: &str) -> bool {
    let has_bangla = s.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c));
    if has_bangla {
        return false;
    }
    s.chars().any(|c| {
        let u = c as u32;
        u >= 0x1F000 || (0x2190..=0x2BFF).contains(&u) || u == 0xFE0F
    })
}

/// Printable Latin char for a key, used only for the "what you typed" echo.
fn latin_of(k: KeyInput) -> Option<char> {
    match k.vk {
        0x41..=0x5A => {
            let c = (b'a' + (k.vk - 0x41) as u8) as char;
            Some(if k.shift ^ k.caps { c.to_ascii_uppercase() } else { c })
        }
        0x30..=0x39 if !k.shift => Some((b'0' + (k.vk - 0x30) as u8) as char),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
