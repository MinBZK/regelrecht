#!/usr/bin/env python3
"""Statement-register gates (dossier-agnostic).

Four gates prove that a statement register is a faithful, complete reading of a
secondary text (toelichting, beleidsregel, werkinstructie). Each is a mechanical
check on the ledger against the canonical text; none of them takes the author's
word for anything.

  VERBATIM   - every quote in the ledger is a literal substring of canonical.md
               under the one documented normalization (see normalize()). A
               paraphrase, a "cleaned up" ellipsis or a re-typed quote fails
               here. Also enforces quote is inside its own segment's text, and
               that a `niet-gevonden` anchoring records the search terms used
               (a negative finding nobody can redo is not a finding).
  COVERAGE   - the segments tile canonical.md completely: segment i starts where
               segment i-1 ended. Any uncovered region is a GAP and is reported
               with its first 80 characters, so silent skipping is impossible.
               Also enforces that a non-normative disposition carries a reason.
  ANCHOR     - every {prefix, exact, suffix} resolves to exactly ONE position in
               canonical.md. Zero matches = orphaned, more than one = ambiguous;
               both make the statement unusable for a cross-version diff (RFC-005
               uniqueness requirement).
  SIGNAALNET - every sentence in a `normative` segment that trips a detector
               (deontic verb, condition word, softener, amount, term, definition,
               cross-reference) must be covered by at least one statement. This
               is the recall net: it does not care what the reader found
               interesting, only that nothing norm-shaped was passed over in
               silence.

Exit code != 0 if any gate finds anything (usable as a CI gate).

Usage:
  python3 statement_gates.py all        --canonical canonical.md --ledger statements.yaml
  python3 statement_gates.py verbatim   --canonical canonical.md --ledger statements.yaml
  python3 statement_gates.py coverage   --canonical canonical.md --ledger statements.yaml
  python3 statement_gates.py anchor     --canonical canonical.md --ledger statements.yaml
  python3 statement_gates.py signaalnet --canonical canonical.md --ledger statements.yaml
                                        [--lexicon lexicon.yaml]
"""
import argparse
import re
import sys
import unicodedata

try:
    import yaml
except ImportError:
    sys.stderr.write("statement_gates.py requires pyyaml (pip install pyyaml)\n")
    sys.exit(2)

# --------------------------------------------------------------------------
# The one allowed normalization.
#
# Anything beyond this is drift, not formatting. Kept deliberately small and
# printed by --explain so a reviewer can audit exactly what was folded away.
# Line-break de-hyphenation is NOT here: it belongs to canonicalize.sh, which
# writes it into canonical.md once, visibly, instead of hiding it at compare
# time.
# --------------------------------------------------------------------------
CHAR_FOLD = {
    "‘": "'", "’": "'", "‚": "'", "′": "'",
    "“": '"', "”": '"', "„": '"',
    "–": "-", "—": "-", "−": "-",
    " ": " ", " ": " ", " ": " ", " ": " ",
    "ﬁ": "fi", "ﬂ": "fl", "ﬀ": "ff",
}
SOFT_HYPHEN = "­"


class Norm:
    """Normalized view of a text, with an index map back to raw offsets.

    Substring searching happens in normalized space (so whitespace and quote
    style never cause a false 'not verbatim'), while every reported position is
    a raw offset into canonical.md, so a human can find it in the file.
    """

    def __init__(self, raw: str):
        self.raw = raw
        out, idx = [], []
        prev_space = False
        for i, ch in enumerate(unicodedata.normalize("NFC", raw)):
            if ch == SOFT_HYPHEN:
                continue
            ch = CHAR_FOLD.get(ch, ch)
            if ch.isspace():
                if prev_space or not out:
                    continue
                out.append(" ")
                idx.append(i)
                prev_space = True
                continue
            prev_space = False
            for c in ch:  # a fold may expand (ﬁ -> fi)
                out.append(c)
                idx.append(i)
        # drop a trailing collapsed space
        while out and out[-1] == " ":
            out.pop()
            idx.pop()
        self.text = "".join(out)
        self.map = idx

    def raw_pos(self, norm_pos: int) -> int:
        if not self.map:
            return 0
        if norm_pos >= len(self.map):
            return len(self.raw)
        return self.map[norm_pos]

    def raw_slice(self, start: int, end: int) -> str:
        return self.raw[self.raw_pos(start):self.raw_pos(end)]


def norm_str(s: str) -> str:
    return Norm(s).text


def excerpt(s: str, n: int = 80) -> str:
    s = " ".join(s.split())
    return s[:n] + ("…" if len(s) > n else "")


# --------------------------------------------------------------------------
# Ledger access
# --------------------------------------------------------------------------
def load(canonical_path: str, ledger_path: str):
    with open(canonical_path, encoding="utf-8") as fh:
        canonical = Norm(fh.read())
    with open(ledger_path, encoding="utf-8") as fh:
        ledger = yaml.safe_load(fh) or {}
    return canonical, ledger


def segments(ledger):
    return ledger.get("segments") or []


def statements(ledger):
    return ledger.get("statements") or []


NON_NORMATIVE = {"informative", "navigational", "duplicate", "non-textual"}
VALID_DISPOSITIONS = {"normative"} | NON_NORMATIVE


# --------------------------------------------------------------------------
# Gate 1 - VERBATIM
# --------------------------------------------------------------------------
def gate_verbatim(canonical: Norm, ledger) -> list:
    findings = []
    seg_text = {s.get("id"): norm_str(s.get("text", "")) for s in segments(ledger)}

    for seg in segments(ledger):
        sid = seg.get("id", "?")
        text = norm_str(seg.get("text", ""))
        if not text:
            findings.append(f"segment {sid}: lege text")
        elif text not in canonical.text:
            findings.append(f"segment {sid}: text niet verbatim in canonical ({excerpt(text)})")

    for st in statements(ledger):
        stid = st.get("id", "?")
        quote = norm_str(st.get("quote", ""))
        if not quote:
            findings.append(f"{stid}: lege quote")
            continue
        if quote not in canonical.text:
            findings.append(f"{stid}: quote niet verbatim in canonical ({excerpt(quote)})")
        parent = seg_text.get(st.get("segment"))
        if parent is None:
            findings.append(f"{stid}: verwijst naar onbekend segment {st.get('segment')!r}")
        elif quote not in parent:
            findings.append(f"{stid}: quote valt buiten segment {st.get('segment')} ({excerpt(quote)})")

        anchoring = st.get("anchoring") or {}
        status = anchoring.get("status")
        if status == "niet-gevonden" and not (anchoring.get("search_terms") or []):
            findings.append(f"{stid}: anchoring niet-gevonden zonder search_terms "
                            "(negatieve bevinding moet overdoenbaar zijn)")
        if status in {"verankerd", "geparafraseerd"} and not anchoring.get("norm_ref"):
            findings.append(f"{stid}: anchoring {status} zonder norm_ref")
    return findings


# --------------------------------------------------------------------------
# Gate 2 - COVERAGE
# --------------------------------------------------------------------------
def gate_coverage(canonical: Norm, ledger) -> tuple:
    """Walk the segments through the canonical text in order.

    Each segment must start exactly where the previous one ended. A segment that
    starts later leaves a GAP; a segment that cannot be found at all breaks the
    chain and is reported as UNPLACED (the walk resumes from the cursor).
    """
    findings, cursor, covered = [], 0, 0
    total = len(canonical.text)

    for seg in segments(ledger):
        sid = seg.get("id", "?")
        disp = seg.get("disposition")
        if disp not in VALID_DISPOSITIONS:
            findings.append(f"segment {sid}: disposition {disp!r} onbekend "
                            f"(kies uit {sorted(VALID_DISPOSITIONS)})")
        if disp in NON_NORMATIVE and not seg.get("reason"):
            findings.append(f"segment {sid}: disposition {disp} zonder reason "
                            "(overslaan mag, stil overslaan niet)")

        text = norm_str(seg.get("text", ""))
        if not text:
            continue
        pos = canonical.text.find(text, cursor)
        if pos < 0:
            back = canonical.text.find(text)
            if back >= 0:
                findings.append(f"segment {sid}: staat vóór het vorige segment "
                                f"(volgorde-fout, gevonden op {canonical.raw_pos(back)})")
            else:
                findings.append(f"segment {sid}: UNPLACED - niet gevonden in canonical")
            continue
        if pos > cursor:
            gap = canonical.text[cursor:pos]
            # Whitespace between two segments is the join, not content.
            if gap.strip():
                findings.append(f"GAP voor segment {sid}: {len(gap)} tekens ongedekt "
                                f"op raw-offset {canonical.raw_pos(cursor)} -> {excerpt(gap)!r}")
            covered += len(gap)
        covered += len(text)
        cursor = pos + len(text)

    if cursor < total:
        tail = canonical.text[cursor:]
        if tail.strip():
            findings.append(f"GAP aan het eind: {len(tail)} tekens ongedekt "
                            f"op raw-offset {canonical.raw_pos(cursor)} -> {excerpt(tail)!r}")
        else:
            covered += len(tail)

    pct = (covered / total * 100) if total else 100.0
    return findings, pct


# --------------------------------------------------------------------------
# Gate 3 - ANCHOR
# --------------------------------------------------------------------------
def anchor_pattern(anchor) -> str:
    return norm_str("{}{}{}".format(anchor.get("prefix", ""),
                                    anchor.get("exact", ""),
                                    anchor.get("suffix", "")))


def count_occurrences(hay: str, needle: str) -> int:
    if not needle:
        return 0
    n, start = 0, 0
    while True:
        p = hay.find(needle, start)
        if p < 0:
            return n
        n += 1
        start = p + 1


def gate_anchor(canonical: Norm, ledger) -> list:
    findings = []
    for st in statements(ledger):
        stid = st.get("id", "?")
        anchor = st.get("anchor") or {}
        if not anchor.get("exact"):
            findings.append(f"{stid}: anchor zonder exact")
            continue
        hits = count_occurrences(canonical.text, anchor_pattern(anchor))
        if hits == 0:
            findings.append(f"{stid}: ORPHANED - prefix+exact+suffix niet gevonden")
        elif hits > 1:
            findings.append(f"{stid}: AMBIGUOUS - {hits} treffers, breid prefix/suffix uit")
    return findings


# --------------------------------------------------------------------------
# Gate 4 - SIGNAALNET
# --------------------------------------------------------------------------
DEFAULT_LEXICON = {
    "deontisch": r"\b(moet(en)?|dient|dienen|verplicht|mag|mogen|kan|kunnen|"
                 r"wordt geacht|worden geacht|bevoegd|recht op|in aanmerking)\b",
    "conditioneel": r"\b(indien|tenzij|mits|voor zover|behoudens|met dien verstande|"
                    r"in het geval dat|wanneer)\b",
    "zachtheid": r"\b(in beginsel|in de regel|doorgaans|zoveel mogelijk|"
                 r"naar (het )?oordeel van|maatwerk|bijzondere omstandigheden|schrijnend)\b",
    "kwantiteit": r"(€\s?\d|\d+\s?%|\b\d+(\.\d{3})*(,\d+)?\s?(euro|procent|dagen|weken|"
                  r"maanden|jaar|jaren)\b)",
    "definitie": r"\b(wordt verstaan onder|worden verstaan onder|geldt als|"
                 r"hieronder valt|wordt aangemerkt als)\b",
    "verwijzing": r"\b(artikel\s+\d|bijlage\s+\w|zie\s+(hoofdstuk|paragraaf|artikel))",
}

SENTENCE_END = re.compile(r"[.!?;]\s")


def sentence_bounds(text: str, pos: int) -> tuple:
    """Expand a position to the sentence containing it (normalized space)."""
    start = 0
    for m in SENTENCE_END.finditer(text, 0, pos):
        start = m.end()
    m = SENTENCE_END.search(text, pos)
    end = m.end() if m else len(text)
    return start, end


def gate_signaalnet(canonical: Norm, ledger, lexicon=None) -> tuple:
    lex = lexicon or DEFAULT_LEXICON
    patterns = {k: re.compile(v, re.IGNORECASE) for k, v in lex.items()}

    # Ranges the reader actually covered: each statement expanded to whole sentences.
    covered = []
    for st in statements(ledger):
        quote = norm_str(st.get("quote", ""))
        p = canonical.text.find(quote) if quote else -1
        if p < 0:
            continue  # already reported by the verbatim gate
        s, _ = sentence_bounds(canonical.text, p)
        _, e = sentence_bounds(canonical.text, max(p, p + len(quote) - 1))
        covered.append((s, max(e, p + len(quote))))

    # Regions the author explicitly dispositioned as non-normative are exempt:
    # they were skipped on the record, which is what the method allows.
    exempt, cursor = [], 0
    for seg in segments(ledger):
        text = norm_str(seg.get("text", ""))
        if not text:
            continue
        p = canonical.text.find(text, cursor)
        if p < 0:
            p = canonical.text.find(text)
            if p < 0:
                continue
        if seg.get("disposition") in NON_NORMATIVE:
            exempt.append((p, p + len(text)))
        cursor = p + len(text)

    def inside(pos, ranges):
        return any(a <= pos < b for a, b in ranges)

    findings, hits, seen = [], 0, set()
    for name, pat in patterns.items():
        for m in pat.finditer(canonical.text):
            hits += 1
            if inside(m.start(), exempt) or inside(m.start(), covered):
                continue
            s, e = sentence_bounds(canonical.text, m.start())
            if (s, e) in seen:
                continue
            seen.add((s, e))
            findings.append(f"ONGEDEKT [{name}] raw-offset {canonical.raw_pos(s)}: "
                            f"{excerpt(canonical.text[s:e], 110)!r}")
    return findings, hits


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------
def report(title: str, findings: list, extra: str = "") -> int:
    status = "FAIL" if findings else "OK"
    print(f"[{status}] {title}{(' ' + extra) if extra else ''} findings={len(findings)}")
    for f in findings:
        print("  " + f)
    return 1 if findings else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command",
                    choices=["all", "verbatim", "coverage", "anchor", "signaalnet", "explain"])
    ap.add_argument("--canonical", required=False)
    ap.add_argument("--ledger", required=False)
    ap.add_argument("--lexicon", help="YAML mapping name -> regex, replaces the default net")
    args = ap.parse_args()

    if args.command == "explain":
        print("Toegestane normalisatie (en niets daarbuiten):")
        print("  1. unicode NFC")
        print("  2. zacht koppelteken (U+00AD) verwijderd")
        print("  3. tekenvouwing: " + ", ".join(f"{k!r}->{v!r}" for k, v in CHAR_FOLD.items()))
        print("  4. witruimte-reeksen samengevouwen tot één spatie, randen gestript")
        print("Afbreekstreepjes over regeleinden horen in canonicalize.sh, niet hier.")
        return 0

    if not args.canonical or not args.ledger:
        ap.error("--canonical en --ledger zijn verplicht")

    canonical, ledger = load(args.canonical, args.ledger)
    lexicon = None
    if args.lexicon:
        with open(args.lexicon, encoding="utf-8") as fh:
            lexicon = yaml.safe_load(fh)

    rc = 0
    if args.command in ("all", "verbatim"):
        rc |= report("VERBATIM", gate_verbatim(canonical, ledger))
    if args.command in ("all", "coverage"):
        findings, pct = gate_coverage(canonical, ledger)
        rc |= report("COVERAGE", findings, f"dekking={pct:.1f}%")
    if args.command in ("all", "anchor"):
        rc |= report("ANCHOR", gate_anchor(canonical, ledger))
    if args.command in ("all", "signaalnet"):
        findings, hits = gate_signaalnet(canonical, ledger, lexicon)
        rc |= report("SIGNAALNET", findings, f"treffers={hits}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
