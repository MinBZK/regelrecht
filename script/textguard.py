#!/usr/bin/env python3
"""textguard — per-lid tekstgetrouwheid voor regelrecht-corpus.

Two-layer text-fidelity guard (see the drift-check skill for the rationale):

  bless  - capture the current normalised `text:` chunks of every regulation
           into a committed fixture (hash + text + provenance). This is the
           "approval" step; provenance starts as `verified_against: pending`
           until the live drift-check upgrades it to a wetten.overheid.nl date.
  check  - recompute the normalised chunk hashes from the corpus and compare
           them to the committed fixtures. Any mismatch (changed/added/removed
           chunk) exits non-zero. Hermetic: a pure function of repo content,
           usable as a CI gate.

A "chunk" is one blank-line-delimited paragraph of an article's `text:` block —
in this corpus that is exactly one lid / onderdeel / aanhef. Chunk identity is
positional (article number + ordinal index); the label (aanhef / lid N /
onderdeel x) is derived for readability only.

Normalisation is the SINGLE allowed rule from
`law-version-drift-check/reference.md` §2: within a paragraph collapse every run
of whitespace to one space and trim; paragraph boundaries are preserved.

Usage:
  python3 script/textguard.py bless [corpus_root] [fixture_root]
  python3 script/textguard.py check [corpus_root] [fixture_root]
Defaults: corpus_root=corpus/regulation  fixture_root=textguard
"""
import sys
import os
import re
import glob
import hashlib
import datetime

try:
    import yaml
except ImportError:
    sys.stderr.write("textguard requires pyyaml (pip install pyyaml)\n")
    sys.exit(2)

DEFAULT_CORPUS = "corpus/regulation"
DEFAULT_FIXTURES = "textguard"

_LID = re.compile(r"^(\d+)\.\s")
_ONDERDEEL = re.compile(r"^([a-z])\.\s")


def normalize_paragraph(p):
    """Collapse every run of whitespace (incl. newlines) to one space, trim."""
    return " ".join(p.split())


def chunks(text):
    """Split an article `text:` into normalised, labelled chunks.

    Returns a list of dicts: {index, label, text, sha256}.
    """
    if not isinstance(text, str):
        return []
    paras = re.split(r"\n[ \t]*\n", text.strip("\n"))
    out = []
    for p in paras:
        norm = normalize_paragraph(p)
        if not norm:
            continue
        idx = len(out)
        out.append({
            "index": idx,
            "label": _label(norm, idx),
            "text": norm,
            "sha256": hashlib.sha256(norm.encode("utf-8")).hexdigest(),
        })
    return out


def _label(norm, idx):
    m = _LID.match(norm)
    if m:
        return f"lid {m.group(1)}"
    m = _ONDERDEEL.match(norm)
    if m:
        return f"onderdeel {m.group(1)}"
    return "aanhef" if idx == 0 else "tekst"


def regulation_files(corpus_root):
    return sorted(glob.glob(f"{corpus_root}/**/*.yaml", recursive=True))


def fixture_path(corpus_root, law_path, fixture_root):
    rel = os.path.relpath(law_path, corpus_root)
    return os.path.join(fixture_root, rel)


def article_chunks(doc):
    """law doc -> list of (article_number_str, [chunk dicts])."""
    res = []
    for art in doc.get("articles", []) or []:
        num = str(art.get("number", "?"))
        res.append((num, chunks(art.get("text"))))
    return res


# --- YAML emission: force block scalars for the multi-line text, keep order ---
def _str_representer(dumper, data):
    style = "|" if "\n" in data else None
    return dumper.represent_scalar("tag:yaml.org,2002:str", data, style=style)


yaml.add_representer(str, _str_representer)


def cmd_bless(corpus_root, fixture_root):
    today = datetime.date.today().isoformat()
    written = 0
    for law_path in regulation_files(corpus_root):
        try:
            doc = yaml.safe_load(open(law_path))
        except Exception:
            continue
        if not isinstance(doc, dict) or "$id" not in doc:
            continue
        arts = []
        for num, chs in article_chunks(doc):
            if not chs:
                continue
            arts.append({
                "number": num,
                "chunks": [{
                    "index": c["index"],
                    "label": c["label"],
                    "sha256": c["sha256"],
                    "verified_against": "pending",
                    "captured_at": today,
                    "text": c["text"],
                } for c in chs],
            })
        if not arts:
            continue
        fixture = {
            "$id": doc["$id"],
            "valid_from": doc.get("valid_from"),
            "source": law_path,
            "articles": arts,
        }
        fpath = fixture_path(corpus_root, law_path, fixture_root)
        os.makedirs(os.path.dirname(fpath), exist_ok=True)
        with open(fpath, "w") as fh:
            yaml.dump(fixture, fh, default_flow_style=False,
                      sort_keys=False, allow_unicode=True, width=4096)
        written += 1
    print(f"textguard bless: {written} fixtures geschreven onder {fixture_root}/")
    return 0


def cmd_check(corpus_root, fixture_root):
    findings = []
    n_chunks = 0
    for law_path in regulation_files(corpus_root):
        try:
            doc = yaml.safe_load(open(law_path))
        except Exception:
            continue
        if not isinstance(doc, dict) or "$id" not in doc:
            continue
        live = {num: chs for num, chs in article_chunks(doc) if chs}
        if not live:
            continue
        fpath = fixture_path(corpus_root, law_path, fixture_root)
        if not os.path.exists(fpath):
            findings.append(f"{law_path}: geen fixture (run `bless`)")
            continue
        fix = yaml.safe_load(open(fpath)) or {}
        fix_arts = {str(a.get("number")): a.get("chunks", []) for a in fix.get("articles", []) or []}
        for num, chs in live.items():
            expected = fix_arts.get(num)
            if expected is None:
                findings.append(f"{law_path} art {num}: artikel niet in fixture")
                continue
            if len(chs) != len(expected):
                findings.append(f"{law_path} art {num}: {len(chs)} chunks, fixture heeft {len(expected)}")
                continue
            for c, e in zip(chs, expected):
                n_chunks += 1
                if c["sha256"] != e.get("sha256"):
                    findings.append(
                        f"{law_path} art {num} [{c['label']}]: tekst wijkt af van geverifieerde fixture")
        # articles present in fixture but gone from the law
        for num in fix_arts:
            if num not in live:
                findings.append(f"{law_path} art {num}: in fixture maar niet meer in de wet")

    print(f"textguard check: {n_chunks} chunks vergeleken, {len(findings)} afwijkingen")
    for f in findings:
        print("  DRIFT", f)
    return 1 if findings else 0


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    corpus_root = sys.argv[2] if len(sys.argv) > 2 else DEFAULT_CORPUS
    fixture_root = sys.argv[3] if len(sys.argv) > 3 else DEFAULT_FIXTURES
    if cmd == "bless":
        return cmd_bless(corpus_root, fixture_root)
    if cmd == "check":
        return cmd_check(corpus_root, fixture_root)
    sys.stderr.write(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
