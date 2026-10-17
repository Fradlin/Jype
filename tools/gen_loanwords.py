#!/usr/bin/env python3
"""Generate crates/engine/data/loanwords.tsv: English word -> Bangla transliteration.

Input : cmudict.dict (CMU Pronouncing Dictionary, BSD-style licence)
          https://github.com/cmusphinx/cmudict
Method: ARPAbet pronunciation -> Bangla, following the spelling conventions that
        Bangla newspapers use for English loanwords (T->ট, D->ড, S+T->স্ট ...).
Run   : python3 tools/gen_loanwords.py cmudict.dict crates/engine/data/loanwords.tsv
"""
import re, sys

HAS = "\u09CD"  # hasanta (virama)
YA = "\u09DF"   # য়

CONS = {
    "B": "ব", "CH": "চ", "D": "ড", "DH": "দ", "F": "ফ", "G": "গ", "HH": "হ",
    "JH": "জ", "K": "ক", "L": "ল", "M": "ম", "N": "ন", "P": "প", "R": "র",
    "S": "স", "SH": "শ", "T": "ট", "TH": "থ", "V": "ভ", "Z": "জ", "ZH": "জ",
}

# (vowel sign after a consonant, independent vowel letter)
VOW = {
    "AA": ("া", "আ"), "AE": ("্যা", "অ্যা"), "AH": ("া", "আ"), "AO": ("", "অ"),
    "AW": ("াউ", "আউ"), "AY": ("াই", "আই"), "EH": ("ে", "এ"), "EY": ("ে", "এ"),
    "IH": ("ি", "ই"), "IY": ("ি", "ই"), "OW": ("ো", "ও"), "OY": (YA, "অয়"),
    "UH": ("ু", "উ"), "UW": ("ু", "উ"), "ERV": ("া", "আ"),
}

# pairs that form a real conjunct in common English loanwords
_JOIN = {
    "স": "টকপলমনফথ", "ক": "সটলর", "গ": "লর", "প": "টলরস", "ব": "লর", "ফ": "টলর",
    "ট": "র", "ড": "র", "থ": "র", "শ": "র", "ভ": "র", "ন": "টডসথ",
    "ম": "পব", "ল": "ডটকপফমব", "ঞ": "জচ", "ক্স": "",
}
REPH_OK = set("টডকগচজতথদধনপফবভমলশষসহ")


def joinable(a, b):
    if a == "র":
        return b in REPH_OK
    return b in _JOIN.get(a, "")


def split_phones(pron):
    out = []
    for p in pron.split():
        m = re.match(r"([A-Z]+)(\d?)$", p)
        out.append((m.group(1), int(m.group(2)) if m.group(2) else None))
    return out


def convert(word, pron):
    ph = split_phones(pron)
    # ER -> vowel + R (spelling "-or" keeps a silent vowel: doctor -> ডক্টর)
    exp = []
    for p, s in ph:
        if p == "ER":
            if s == 0 and word.endswith("or") and len(exp) > 0:
                exp.append(("ERQ", s))  # silent
            else:
                exp.append(("ERV", s))
            # an unstressed ER before a consonant is written plainly (ইন্টারনেট)
            exp.append(("R", 0 if s == 0 else None))
        else:
            exp.append((p, s))
    ph = exp
    n = len(ph)
    is_v = lambda p: p in VOW or p == "ERQ"
    out = []
    last = None      # None | "C" (consonant waiting for a vowel) | "V"
    prevC = None
    prevV = None
    prevW = False
    has_a = "a" in word

    cluster = []

    def add_c(letter, plain=False):
        nonlocal last, prevC, cluster
        join = (last == "C" and prevC and not plain and joinable(prevC, letter)
                and (len(cluster) < 2 or letter in "রল" or cluster[0] == "র"))
        if join:
            out.append(HAS)
            cluster.append(letter)
        else:
            cluster = [letter]
        out.append(letter)
        last, prevC = "C", letter
        if plain:
            prevC = None

    i = 0
    while i < n:
        p, s = ph[i]
        nx = ph[i + 1][0] if i + 1 < n else None
        nx2 = ph[i + 2][0] if i + 2 < n else None
        if not is_v(p):
            wasW = False
            if p == "NG":
                out.append("ং"); last, prevC, prevV = "V", None, None
            elif p == "N" and nx in ("JH", "CH"):
                add_c("ঞ")
            elif p == "W":
                if nx and is_v(nx):
                    if last == "C":
                        if nx in ("IH", "IY"):
                            out.append("ুই"); i += 1; last, prevC = "V", None
                        elif nx in ("UH", "UW"):
                            out.append("ু"); i += 1; last, prevC = "V", None
                        elif nx in ("AA", "AO", "AH", "EH"):
                            out.append("ো" + YA); last, prevC = "C", None; wasW = True
                        else:
                            out.append("ু" + YA); last, prevC = "C", None; wasW = True
                    else:
                        if nx in ("IH", "IY"):
                            out.append("উই"); i += 1; last, prevC = "V", None
                        elif nx in ("UH", "UW"):
                            out.append("উ"); i += 1; last, prevC = "V", None
                        else:
                            out.append("ও" + YA); last, prevC = "C", None; wasW = True
                else:
                    out.append("ও"); last, prevC = "V", None
            elif p == "Y":
                if nx == "UW":
                    out.append("ি" + "উ" if last == "C" else "ইউ")
                    i += 1; last, prevC = "V", None
                elif nx in ("IH", "IY") and nx2 == "R":
                    out.append("ইয়া" if last != "C" else "িয়া"); i += 1; last, prevC = "V", None
                elif last == "C":
                    out.append("ি" + YA); last, prevC = "C", None
                elif last == "V":
                    out.append(YA); last, prevC = "C", None
                elif nx and is_v(nx):
                    out.append("ই" + YA); last, prevC = "C", None
                else:
                    out.append("ই"); last, prevC = "V", None
            else:
                letter = CONS[p]
                if p == "Z" and nx is None and last == "C":
                    letter = "স"
                add_c(letter, plain=(p == "R" and s == 0))
            prevW = wasW
            if p not in ("NG",):
                prevV = None if last == "C" else prevV
            i += 1
            continue

        # ---- vowel ----
        v = p
        sign, indep = VOW.get(v, ("", "অ"))
        # glide between vowels
        if last == "V" and prevV is not None and v in ("AH", "ERV", "ERQ", "AA", "AO", "EH", "AE"):
            if prevV in ("IY", "IH"):
                out.append(YA); last = "C"; prevC = None
        before_vowel = bool(nx and is_v(nx))
        if v == "AW" and before_vowel:
            out.append("া" if last == "C" else "আ"); out.append("ওয়"); last, prevC, prevV = "C", None, None
            i += 1; continue
        if v in ("AY", "EY", "OW") and before_vowel:
            sg = {"AY": "া", "EY": "ে", "OW": "ো"}[v]
            ind = {"AY": "আ", "EY": "এ", "OW": "ও"}[v]
            out.append(sg if last == "C" else ind); out.append(YA)
            last, prevC, prevV = "C", None, None
            i += 1; continue
        if v in ("IY", "IH") and before_vowel and nx in ("AH", "ERV", "ERQ", "AA", "AO", "EH", "AE"):
            out.append(sign if last == "C" else indep); out.append(YA)
            last, prevC, prevV = "C", None, None
            i += 1; continue

        if v == "AH":
            if s == 0:
                if i == 0:
                    sign, indep = "া", "আ"
                elif i == n - 1:
                    sign, indep = "া", "আ"
                else:
                    sign, indep = "", "অ"
        elif v == "AA":
            if not has_a and nx != "R":
                sign, indep = "", "অ"
        elif v == "AO":
            if nx == "R":
                sign, indep = "ো", "ও"
            elif prevW:
                sign, indep = "া", "আ"
        elif v == "IH" and s == 0 and nx in ("D", "Z", "S") and i + 2 == n:
            sign, indep = "ে", "এ"
        elif v == "ERQ":
            sign, indep = "", "অ"

        cluster = []
        if last == "C":
            out.append(sign)
            if v == "OY":
                last = "V"; prevC = None
            else:
                last, prevC = "V", None
        else:
            out.append(indep)
            last, prevC = "V", None
        prevV = v if v in ("IY", "IH") else None
        prevW = False
        i += 1

    return "".join(out)


OVERRIDES = {"the": "দ্য", "a": "এ", "of": "অফ"}

# Hand-checked spellings for very common words where pronunciation rules can't
# know the vowel Bangla writers insert (student -> স্টুডেন্ট), or where CMU's
# first pronunciation differs (mobile -> মোবাইল). Applied even if the word is
# not in CMUdict.
COMMON = """
mobile মোবাইল|hello হ্যালো|hi হাই|bye বাই|ok ওকে|okay ওকে|yes ইয়েস|no নো|sorry সরি|thanks থ্যাংকস|thank থ্যাংক
good গুড|morning মর্নিং|night নাইট|sir স্যার|madam ম্যাডাম|please প্লিজ|welcome ওয়েলকাম
camera ক্যামেরা|minute মিনিট|student স্টুডেন্ট|message মেসেজ|english ইংলিশ|engine ইঞ্জিন|program প্রোগ্রাম
telegram টেলিগ্রাম|whatsapp হোয়াটসঅ্যাপ|university ইউনিভার্সিটি|example এক্সাম্পল|college কলেজ|class ক্লাস
exam এক্সাম|result রেজাল্ট|report রিপোর্ট|meeting মিটিং|manager ম্যানেজার|project প্রজেক্ট|design ডিজাইন
internet ইন্টারনেট|email ইমেইল|website ওয়েবসাইট|online অনলাইন|offline অফলাইন|download ডাউনলোড|upload আপলোড
login লগইন|logout লগআউট|password পাসওয়ার্ড|username ইউজারনেম|laptop ল্যাপটপ|desktop ডেস্কটপ|android অ্যান্ড্রয়েড
iphone আইফোন|youtube ইউটিউব|instagram ইনস্টাগ্রাম|twitter টুইটার|linkedin লিংকডইন|github গিটহাব|gmail জিমেইল
chrome ক্রোম|firefox ফায়ারফক্স|microsoft মাইক্রোসফট|office অফিস|excel এক্সেল|powerpoint পাওয়ারপয়েন্ট|google গুগল
facebook ফেসবুক|photo ফটো|video ভিডিও|audio অডিও|file ফাইল|folder ফোল্ডার|document ডকুমেন্ট|screen স্ক্রিন
screenshot স্ক্রিনশট|mouse মাউস|keyboard কিবোর্ড|printer প্রিন্টার|battery ব্যাটারি|charger চার্জার|bluetooth ব্লুটুথ
wifi ওয়াইফাই|network নেটওয়ার্ক|server সার্ভার|database ডাটাবেস|hardware হার্ডওয়্যার|software সফটওয়্যার
update আপডেট|install ইনস্টল|setting সেটিং|settings সেটিংস|language ল্যাঙ্গুয়েজ|customize কাস্টমাইজ
customized কাস্টমাইজড|customizing কাস্টমাইজিং|customer কাস্টমার|customers কাস্টমারস|computer কম্পিউটার
"""
for _pair in COMMON.replace("\n", "|").split("|"):
    _pair = _pair.strip()
    if _pair:
        _w, _b = _pair.split(" ", 1)
        OVERRIDES[_w] = _b


def main(src, dst):
    seen = {}
    for line in open(src, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith(";"):
            continue
        w, _, pron = line.partition(" ")
        if "(" in w:
            continue  # alternative pronunciations: keep the first only
        if not re.fullmatch(r"[a-z]{2,24}", w) and w not in OVERRIDES:
            continue
        try:
            m = re.search(r" W EH\d R$", pron)
            if w.endswith("ware") and m and len(w) > 4:
                bn = convert(w[:-4], pron[: m.start()]) + "\u0993" + YA + HAS + "\u09AF\u09BE\u09B0"
            else:
                bn = OVERRIDES.get(w) or convert(w, pron)
        except Exception:
            continue
        if bn:
            # one encoding for য় (U+09DF), never য + nukta
            seen[w] = bn.replace("\u09AF\u09BC", YA)
    for w, b in OVERRIDES.items():
        seen[w] = b.replace("\u09AF\u09BC", YA)
    with open(dst, "w", encoding="utf-8", newline="\n") as f:
        for w in sorted(seen):
            f.write(f"{w}\t{seen[w]}\n")
    print(len(seen), "words")


if __name__ == "__main__":
    if len(sys.argv) == 3:
        main(*sys.argv[1:])
    else:
        print(__doc__)
