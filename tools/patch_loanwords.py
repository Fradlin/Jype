#!/usr/bin/env python3
"""Post-process crates/engine/data/loanwords.tsv.

1. Spelling rules the pronunciation converter cannot know (Bangla writers insert
   an 'e' in -ment, -ket, ...; initial 'or' is অর্, not ওর্).
2. tools/curated.tsv (brands, Bangladeshi services, everyday words) always wins.

Run: python3 tools/patch_loanwords.py crates/engine/data/loanwords.tsv   (safe to re-run)
"""
import sys, pathlib

CONS = "কখগঘচছজঝটঠডঢতথদধনপফবভমযরলশষসহ"
E_SIGN = "\u09C7"   # ে


def fix(word, bn):
    # initial "or-": order, organize, original ...  ওর্ -> অর্
    if word.startswith("or") and bn.startswith("\u0993\u09B0\u09CD"):
        bn = "\u0985\u09B0\u09CD" + bn[3:]
    # -ment / -ments: payment, statement, management
    for suf, wrong, right in (("ment", "মন্ট", "মেন্ট"), ("ments", "মন্টস", "মেন্টস")):
        if word.endswith(suf) and bn.endswith(wrong):
            bn = bn[: -len(wrong)] + right
    # -ket / -kets: market, rocket, pocket, ticket(curated)
    for suf, wrong, right in (("ket", "কট", "কেট"), ("kets", "কটস", "কেটস")):
        if word.endswith(suf) and bn.endswith(wrong):
            bn = bn[: -len(wrong)] + right
    return bn


def main(path):
    p = pathlib.Path(path)
    rows = {}
    for line in p.read_text(encoding="utf-8").splitlines():
        w, _, b = line.partition("\t")
        if w and b:
            rows[w] = b
    changed = 0
    for w in list(rows):
        n = fix(w, rows[w])
        if n != rows[w]:
            rows[w] = n
            changed += 1
    cur = pathlib.Path(__file__).with_name("curated.tsv")
    curated = 0
    for line in cur.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        w, _, b = line.partition("\t")
        rows[w.strip().lower()] = b.strip().replace("\u09AF\u09BC", "\u09DF")
        curated += 1
    with p.open("w", encoding="utf-8", newline="\n") as f:
        for w in sorted(rows):
            f.write(f"{w}\t{rows[w]}\n")
    print(f"{len(rows)} words, {changed} fixed by rules, {curated} curated")


if __name__ == "__main__":
    main(sys.argv[1])
