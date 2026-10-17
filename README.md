# ![Jype](crates/app/assets/jype.ico)Jype
### Fast, friendly Bangla typing for Windows

Jype is a small tray app that lets you type Bangla anywhere on Windows — in your
browser, Word, Messenger, Discord, your code editor, anything — just by typing
English letters the way they sound. No separate keyboard layout to install, no
restarting your PC, no internet connection required.

This document covers everything Jype can do as of **v0.3**, the first public
test build.

---

## Type Bangla by sound, anywhere

Turn Bangla typing on with a shortcut (Ctrl+Space by default) and start typing.
Jype watches your keystrokes system-wide and turns them into Bangla as you go
— you don't need to click into a special box first.

```
ami banglay kotha boli   →   আমি বাংলায় কথা বলি
```

As soon as you start a word, a small card appears next to your cursor showing
what you've typed so far and every way it could be spelled in Bangla:

* **Space** or **click** accepts the highlighted spelling
* **↑ / ↓** or the **number keys** move between the other options
* **Esc** cancels the word and leaves what you typed as plain English

### Seven keyboard layouts

Pick whichever you already know, or try Avro Phonetic if you're new to Bangla
typing on a computer:

| Layout | What it is |
|---|---|
| **Avro Phonetic** | The most popular phonetic system — type words the way they sound |
| **Probhat** | The classic fixed Bangla layout, same as on a physical keyboard |
| **Jatiya (National)** | Bangladesh's official national keyboard layout |
| **Munir Optima** | A popular optimized fixed layout |
| **Borno** | Another widely used fixed layout |
| **Avro Easy** | A simplified phonetic layout |
| **Khipro** | A compact phonetic layout |

Switch layouts any time from the tray menu, the floating bar, or Jype Studio
— your words keep typing correctly even mid-sentence.

---

## It learns how *you* spell things

### English words that should stay as loanwords

Bangla sentences are full of English words typed the way they sound —
`bkash`, `order`, `market`, `payment`. Jype recognizes well over a hundred
thousand of these automatically, plus a hand-checked list of everyday words,
brand names and Bangladeshi services (bKash, Nagad, Pathao, Grameenphone and
more) that are tuned to come out exactly the way Bangla speakers actually
write them:

```
bkash     →   বিকাশ
order     →   অর্ডার
market    →   মার্কেট
payment   →   পেমেন্ট
```

### Your own dictionary

Typing a name, a nickname, or a word Jype doesn't know yet? Teach it once in
**Jype Studio → Dictionary**, and it's remembered from then on:

```
zoom   →   জুম
```

Your words always come first, ahead of everything Jype already knows.

### On-device learning

When a word has more than one common spelling, Jype quietly remembers which
one *you* picked last time and offers that one first the next time you type
the same word — no setup, no settings to dig through. This happens entirely
on your own PC. Jype has no network code at all, so this information is never
sent anywhere by the app itself.

If you ever want a clean slate, **Studio → Dictionary** has a one-click
**Reset learned** button, and an **Export word data** button if you ever want
to save a copy of what's been learned to share yourself.

---

## A floating bar that stays out of the way

A small, draggable bar shows your current mode (Bangla or English) at a
glance. Click its badge to switch instantly, or click the arrow to expand it
into quick-access buttons for layout, your dictionary, fonts, and settings —
then collapse it back down to a compact pill when you don't need it. It
remembers where you left it and whether you had it expanded.

## Jype Studio

A proper settings window, not a maze of checkboxes:

* **Dictionary** — add, edit and remove your own words; see everything Jype
  has learned
* **Look** — pick your Bangla font, text size, light/dark theme, and one of
  four accent colors
* **Typing** — choose your layout, your switch shortcut, and toggle options
  like showing the floating bar or starting Jype with Windows
* **About** — version, credits, and a shortcut to your settings folder

## Five Bangla fonts, built in

No downloads, no font installer. Jype ships with:

* **Kohinoor Bangla**
* **Hind Siliguri**
* **Anek Bangla**
* **Tiro Bangla**
* **Baloo Da 2**

Switch between them instantly in **Studio → Look** — the whole UI and the
suggestion popup update immediately. Want another font? Drop its `.ttf` file
into Jype's settings folder and it shows up in the list automatically.

---

## Built with your privacy in mind

* **No internet connection, ever.** Jype doesn't phone home, check for
  updates online, or talk to any server. Everything — your dictionary, your
  learned words, your settings — stays in one folder on your own PC.
* **Nothing is collected automatically.** The only data Jype ever produces
  about how you type is the on-device learning described above, and it never
  leaves your computer unless you choose to export and send it yourself.
* **Open-source engine.** Jype's typing engine is built on **Riti**, from the
  OpenBangla Keyboard project (MPL-2.0), with English readings from the CMU
  Pronouncing Dictionary. Full credits are in the app's About page.

---

## Getting started

1. Run `Jype-Setup.exe` (or use the portable `Jype.exe` — no install needed).
2. Look for the maroon **J** icon near your clock.
3. Press **Ctrl+Space** (or your chosen shortcut) and start typing.

That's it — no restart, no extra setup.

---

*Jype is an early test release. Found a word that comes out wrong, or a font
that doesn't look right? That's exactly what this test is for — let us know.*
