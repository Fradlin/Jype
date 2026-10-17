use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

fn engine(layout: Layout) -> Engine {
    let dir = std::env::temp_dir().join(format!(
        "banglatype-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    Engine::new(&dir, layout, false).expect("engine")
}

fn key(vk: u32) -> KeyInput {
    KeyInput { vk, ..Default::default() }
}

fn press(e: &mut Engine, c: char) -> Reply {
    let (vk, shift) = char_to_vk(c).expect("typable char");
    e.handle_key(KeyInput { vk, shift, ..Default::default() })
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        assert_eq!(press(e, c), Reply::Consume, "key {c:?} should be consumed");
    }
}

/// Type `latin` then press Space; return the committed text.
fn word(e: &mut Engine, latin: &str) -> String {
    type_str(e, latin);
    match e.handle_key(key(VK_SPACE)) {
        Reply::Commit { text, then: Then::Space } => text,
        other => panic!("expected commit, got {other:?}"),
    }
}

#[test]
fn phonetic_basic_words() {
    let mut e = engine(Layout::AvroPhonetic);
    assert_eq!(word(&mut e, "ami"), "আমি");
    assert_eq!(word(&mut e, "tumi"), "তুমি");
    // lowercase o = inherent vowel (no sign); capital O = the "o-kar" sign ো
    assert_eq!(word(&mut e, "bhalobashi"), "ভালবাসি");
    assert_eq!(word(&mut e, "bhalObashi"), "ভালোবাসি");
}

#[test]
fn phonetic_conjuncts_and_digits() {
    let mut e = engine(Layout::AvroPhonetic);
    assert_eq!(word(&mut e, "kShoma"), "ক্ষমা");
    assert_eq!(word(&mut e, "123"), "১২৩");
    assert_eq!(word(&mut e, "bangla"), "বাংলা");
}

#[test]
fn emoji_never_outranks_bangla() {
    let mut e = engine(Layout::AvroPhonetic);
    assert_eq!(word(&mut e, "ng"), "ঙ");
    assert_eq!(word(&mut e, "nOukA"), "নৌকা");
    // emoji are still reachable, just not first
    type_str(&mut e, "nOukA");
    let v = e.view().unwrap();
    assert!(v.candidates.iter().any(|c| c == "⛵"));
    assert_eq!(v.selected, 0);
}

#[test]
fn candidates_are_offered() {
    let mut e = engine(Layout::AvroPhonetic);
    type_str(&mut e, "ami");
    let v = e.view().expect("popup visible while composing");
    assert_eq!(v.typed, "ami");
    assert!(v.candidates.len() >= 1);
    assert!(v.candidates.iter().any(|c| c == "আমি"));
}

#[test]
fn punctuation_becomes_part_of_word() {
    let mut e = engine(Layout::AvroPhonetic);
    assert_eq!(word(&mut e, "ami."), "আমি।");
}

#[test]
fn space_when_idle_is_not_ours() {
    let mut e = engine(Layout::AvroPhonetic);
    assert_eq!(e.handle_key(key(VK_SPACE)), Reply::Pass);
    assert_eq!(e.handle_key(key(VK_RETURN)), Reply::Pass);
    assert_eq!(e.handle_key(key(VK_BACK)), Reply::Pass);
    assert!(e.view().is_none());
}

#[test]
fn backspace_edits_the_word() {
    let mut e = engine(Layout::AvroPhonetic);
    type_str(&mut e, "amix");
    assert_eq!(e.handle_key(key(VK_BACK)), Reply::Consume);
    assert_eq!(e.view().unwrap().typed, "ami");
    // erase the rest: popup closes and keys flow normally again
    for _ in 0..3 {
        assert_eq!(e.handle_key(key(VK_BACK)), Reply::Consume);
    }
    assert!(e.view().is_none());
    assert_eq!(e.handle_key(key(VK_BACK)), Reply::Pass);
}

#[test]
fn escape_cancels_without_typing() {
    let mut e = engine(Layout::AvroPhonetic);
    type_str(&mut e, "ami");
    assert_eq!(e.handle_key(key(VK_ESCAPE)), Reply::Consume);
    assert!(e.view().is_none());
}

#[test]
fn enter_commits_then_replays_enter() {
    let mut e = engine(Layout::AvroPhonetic);
    type_str(&mut e, "ami");
    match e.handle_key(key(VK_RETURN)) {
        Reply::Commit { text, then } => {
            assert_eq!(text, "আমি");
            assert_eq!(then, Then::Replay(VK_RETURN));
        }
        o => panic!("{o:?}"),
    }
}

#[test]
fn shortcuts_pass_when_idle_and_commit_when_composing() {
    let mut e = engine(Layout::AvroPhonetic);
    let ctrl_v = KeyInput { vk: 0x56, ctrl: true, ..Default::default() };
    assert_eq!(e.handle_key(ctrl_v), Reply::Pass);
    type_str(&mut e, "ami");
    match e.handle_key(ctrl_v) {
        Reply::Commit { text, then } => {
            assert_eq!(text, "আমি");
            assert_eq!(then, Then::Replay(0x56));
        }
        o => panic!("{o:?}"),
    }
    assert!(e.view().is_none());
}

#[test]
fn arrows_cycle_candidates_and_choice_is_committed() {
    let mut e = engine(Layout::AvroPhonetic);
    type_str(&mut e, "ami");
    let n = e.view().unwrap().candidates.len();
    let first = e.view().unwrap().candidates[e.view().unwrap().selected].clone();
    if n > 1 {
        assert_eq!(e.handle_key(key(VK_DOWN)), Reply::Consume);
        let v = e.view().unwrap();
        assert_ne!(v.candidates[v.selected], first);
        assert_eq!(e.handle_key(key(VK_UP)), Reply::Consume);
        let v = e.view().unwrap();
        assert_eq!(v.candidates[v.selected], first);
    }
}

#[test]
fn clicking_a_candidate_commits_it() {
    let mut e = engine(Layout::AvroPhonetic);
    type_str(&mut e, "ami");
    let want = e.view().unwrap().candidates[0].clone();
    match e.commit_candidate(0) {
        Reply::Commit { text, then: Then::Nothing } => assert_eq!(text, want),
        o => panic!("{o:?}"),
    }
    assert!(e.view().is_none());
}

#[test]
fn switching_layouts_works_for_every_layout() {
    let mut e = engine(Layout::AvroPhonetic);
    for &l in Layout::ALL {
        e.set_layout(l).unwrap_or_else(|er| panic!("{l:?}: {er}"));
        assert_eq!(e.layout(), l);
        // type a couple of keys on each layout; must not panic or hang
        press(&mut e, 'a');
        press(&mut e, 'k');
        let _ = e.handle_key(key(VK_SPACE));
        e.reset();
    }
}

#[test]
fn fixed_layout_produces_bangla() {
    let mut e = engine(Layout::Probhat);
    // Probhat: 'k' is ক, 'a' is a vowel/kar
    let out = word(&mut e, "k");
    assert!(out.chars().all(|c| ('\u{0980}'..='\u{09FF}').contains(&c)), "got {out:?}");
    assert!(!out.is_empty());
}

#[test]
fn unmapped_keys_pass_through() {
    let mut e = engine(Layout::AvroPhonetic);
    assert_eq!(e.handle_key(key(0x70)), Reply::Pass); // F1
    type_str(&mut e, "ami");
    // F1 while composing: finish the word then replay
    match e.handle_key(key(0x70)) {
        Reply::Commit { then: Then::Replay(0x70), .. } => {}
        o => panic!("{o:?}"),
    }
}

#[test]
#[ignore]
fn print_guide_samples() {
    let mut e = engine(Layout::AvroPhonetic);
    for w in ["t","th","d","dh","Dh","T","Th","D","kh","gh","chh","c","s","sh","Sh","S","n","N","ng","NG","h","H","Z","z","j","w","O","o","a","A","i","I","u","U","e","E","ri","rri","Ng","M","m","ee","oo","``",",,", "Rh","R","Y","y","f","ph","bh","v","q","x"] {
        let mut e2 = engine(Layout::AvroPhonetic);
        let _ = &mut e;
        let mut out = vec![];
        for part in w.split(' ') {
            type_str(&mut e2, part);
            let v = e2.view().unwrap();
            out.push(format!("{} => {:?}", part, &v.candidates[..v.candidates.len().min(4)]));
            e2.reset();
        }
        println!("{}", out.join(" | "));
    }
}

#[test]
#[ignore]
fn timing() {
    use std::time::Instant;
    let t = Instant::now();
    let mut e = engine(Layout::AvroPhonetic);
    println!("TIMING engine new: {:?}", t.elapsed());
    for (i, c) in "a".chars().enumerate() {
        let t = Instant::now();
        press(&mut e, c);
        println!("TIMING first key #{i}: {:?}", t.elapsed());
    }
    e.reset();
    let mut worst = std::time::Duration::ZERO;
    for w in ["bangladesher", "shobhyotar", "prothomalo", "kShomapratharthi", "onushondhan", "bishshobiddyaloy"] {
        for c in w.chars() {
            let t = Instant::now();
            press(&mut e, c);
            worst = worst.max(t.elapsed());
        }
        e.reset();
    }
    println!("TIMING worst per-key over long words: {:?}", worst);
}

// ---- English words typed in English spelling ------------------------------

#[test]
fn english_words_become_bangla_loanwords() {
    let mut e = engine(Layout::AvroPhonetic);
    // the two words from the bug report
    assert_eq!(word(&mut e, "customize"), "কাস্টমাইজ");
    assert_eq!(word(&mut e, "customized"), "কাস্টমাইজড");
    assert_eq!(word(&mut e, "computer"), "কম্পিউটার");
    assert_eq!(word(&mut e, "internet"), "ইন্টারনেট");
    assert_eq!(word(&mut e, "mobile"), "মোবাইল");
    assert_eq!(word(&mut e, "software"), "সফটও\u{09DF}\u{09CD}যার");
    assert_eq!(word(&mut e, "Customized"), "কাস্টমাইজড");
    assert_eq!(word(&mut e, "customized."), "কাস্টমাইজড।");
}

#[test]
fn bangla_typing_is_not_hijacked_by_english() {
    let mut e = engine(Layout::AvroPhonetic);
    assert_eq!(word(&mut e, "ami"), "আমি");
    assert_eq!(word(&mut e, "tumi"), "তুমি");
    assert_eq!(word(&mut e, "kShoma"), "ক্ষমা"); // capitals = phonetic tricks
    assert_eq!(word(&mut e, "bangla"), "বাংলা");
    // short words that exist in Bangla keep the Bangla reading first
    for w in ["hat", "bar", "to", "me"] {
        type_str(&mut e, w);
        let v = e.view().unwrap().clone();
        e.reset();
        let first = &v.candidates[v.selected];
        assert!(
            first.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c)),
            "{w}: {:?}", v.candidates
        );
    }
}

#[test]
fn english_reading_is_always_reachable() {
    let mut e = engine(Layout::AvroPhonetic);
    type_str(&mut e, "hat");
    let v = e.view().unwrap();
    assert!(v.candidates.iter().any(|c| c == "হ্যাট"), "{:?}", v.candidates);
}

#[test]
fn users_can_add_their_own_loanwords() {
    let dir = std::env::temp_dir().join(format!("banglatype-user-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("loanwords.txt"), "# mine\njype  জাইপ\nhat=হ্যাট\n").unwrap();
    let mut e = Engine::new(&dir, Layout::AvroPhonetic, false).unwrap();
    assert_eq!(word(&mut e, "jype"), "জাইপ");
    // user words win even over a Bangla dictionary hit
    assert_eq!(word(&mut e, "hat"), "হ্যাট");
}

#[test]
fn clicking_the_english_reading_commits_it() {
    let mut e = engine(Layout::AvroPhonetic);
    type_str(&mut e, "customized");
    match e.commit_candidate(0) {
        Reply::Commit { text, .. } => assert_eq!(text, "কাস্টমাইজড"),
        o => panic!("{o:?}"),
    }
    assert!(e.view().is_none());
    // engine still healthy afterwards
    assert_eq!(word(&mut e, "ami"), "আমি");
}

#[test]
fn reported_words_come_out_right() {
    for (typed, bn) in [
        ("order", "অর্ডার"),
        ("bkash", "বিকাশ"),
        ("market", "মার্কেট"),
        ("payment", "পেমেন্ট"),
        ("customized", "কাস্টমাইজড"),
    ] {
        let mut e = engine(Layout::AvroPhonetic);
        type_str(&mut e, typed);
        let v = e.view().unwrap();
        assert_eq!(v.candidates[v.selected], bn, "{typed}: {:?}", v.candidates);
    }
}

#[test]
fn personal_words_can_be_added_and_removed_live() {
    let dir = std::env::temp_dir().join(format!("jype-live-dict-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut e = Engine::new(&dir, Layout::AvroPhonetic, false).unwrap();
    e.add_user_word("zoomer", "জুমার").unwrap();
    assert_eq!(word(&mut e, "zoomer"), "জুমার");
    assert_eq!(e.user_words(), vec![("zoomer".to_string(), "জুমার".to_string())]);
    e.remove_user_word("zoomer").unwrap();
    assert!(e.user_words().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn learns_from_repeated_picks() {
    let dir = std::env::temp_dir().join(format!("jype-engine-learn-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut e = Engine::new(&dir, Layout::AvroPhonetic, false).unwrap();
    // "ans" offers multiple candidates; whichever we pick repeatedly should
    // come first on the next, fresh word.
    type_str(&mut e, "ans");
    let v = e.view().unwrap();
    assert!(v.candidates.len() > 1, "{:?}", v.candidates);
    let pick = v.candidates[v.candidates.len() - 1].clone();
    let last = v.candidates.len() - 1;
    e.commit_candidate(last);
    for _ in 0..3 {
        type_str(&mut e, "ans");
        let v = e.view().unwrap();
        let i = v.candidates.iter().position(|c| *c == pick).unwrap();
        e.commit_candidate(i);
    }
    type_str(&mut e, "ans");
    let v = e.view().unwrap();
    assert_eq!(v.candidates[0], pick, "learned pick should now lead: {:?}", v.candidates);
    assert!(e.learned_words().iter().any(|(w, _)| w == "ans"));
    e.forget_word("ans").unwrap();
    assert!(!e.learned_words().iter().any(|(w, _)| w == "ans"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore]
fn debug_view() {
    for w in ["ami", "bangla", "customized", "hat", "to"] {
        let mut e = engine(Layout::AvroPhonetic);
        type_str(&mut e, w);
        let v = e.view().unwrap();
        println!("DBG {w}: selected={} order={:?} cands={:?}", v.selected, e.order, v.candidates);
    }
}
