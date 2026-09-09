#!/usr/bin/env python3
"""Audit de koppelingen in een regelrecht-corpus.

Drie declaraties, drie controles:

  implements       -> resolvet de sleutel (law, article, open_term) naar een
                      bestaand artikel dat die open term ook echt declareert?
  open_terms       -> vult iemand hem in?
  legal_character  -> filtert er ergens een hook op?

Elke bevinding is DOOD (sleutel matcht niet), ONBEANTWOORD (declaratie klopt,
maar er is geen tegenhanger) of CORRECT. DOOD is altijd een defect;
ONBEANTWOORD is vaak legitiem en vraagt een oordeel.

Gebruik:
    koppeling-audit.py <corpus-dir> [--peildatum YYYY-MM-DD] [--json]

<corpus-dir> is de map met de `nl/`-laag erin, of de `nl/`-map zelf.
Per wet wordt alleen de nieuwste versie <= peildatum geladen, net als de engine.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

import yaml

VERSION_RE = re.compile(r"^\d{4}-\d{2}-\d{2}\.yaml$")


def load_corpus(root: Path, peildatum: str) -> dict[str, dict]:
    """Nieuwste versie <= peildatum per wet, gesleuteld op $id."""
    by_id: dict[str, tuple[str, dict, Path]] = {}
    for path in sorted(root.rglob("*.yaml")):
        if not VERSION_RE.match(path.name) or "scenarios" in path.parts:
            continue
        valid_from = path.stem
        if valid_from > peildatum:
            continue
        try:
            doc = yaml.safe_load(path.read_text(encoding="utf-8"))
        except yaml.YAMLError as exc:
            print(f"WAARSCHUWING: {path} niet leesbaar: {exc}", file=sys.stderr)
            continue
        if not isinstance(doc, dict) or "$id" not in doc:
            continue
        law_id = doc["$id"]
        if law_id not in by_id or valid_from > by_id[law_id][0]:
            by_id[law_id] = (valid_from, doc, path)
    return {k: {"valid_from": v[0], "doc": v[1], "path": v[2]} for k, v in by_id.items()}


def articles(doc: dict):
    for art in doc.get("articles") or []:
        if isinstance(art, dict):
            yield str(art.get("number", "?")), (art.get("machine_readable") or {})


def audit(corpus: dict[str, dict]) -> list[dict]:
    # index: (law, article) -> set van open_term-ids; en law -> {open_term: [articles]}
    declared: dict[tuple[str, str], set[str]] = defaultdict(set)
    by_law_term: dict[str, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    open_term_meta: dict[tuple[str, str, str], dict] = {}
    implements: list[dict] = []
    produced_characters: dict[str, list[str]] = defaultdict(list)
    hook_characters: set[str] = set()

    for law_id, entry in corpus.items():
        for number, mr in articles(entry["doc"]):
            for term in mr.get("open_terms") or []:
                tid = term.get("id")
                if not tid:
                    continue
                declared[(law_id, number)].add(tid)
                by_law_term[law_id][tid].append(number)
                open_term_meta[(law_id, number, tid)] = term
            for impl in mr.get("implements") or []:
                implements.append({"from_law": law_id, "from_article": number, **impl})
            for hook in mr.get("hooks") or []:
                char = (hook.get("applies_to") or {}).get("legal_character")
                if char:
                    hook_characters.add(char)
            produces = (mr.get("execution") or {}).get("produces") or {}
            if produces.get("legal_character"):
                produced_characters[produces["legal_character"]].append(f"{law_id}:{number}")

    findings: list[dict] = []

    # 1 - implements
    for impl in implements:
        target_law, target_article = impl.get("law"), str(impl.get("article"))
        term = impl.get("open_term")
        bron = f"{impl['from_law']}:{impl['from_article']}"
        doel = f"{target_law}:{target_article}:{term}"
        if target_law not in corpus:
            findings.append(dict(soort="DOOD", regel="implements", bron=bron, doel=doel,
                                 reden=f"wet '{target_law}' niet in het geladen corpus"))
        elif term in declared[(target_law, target_article)]:
            findings.append(dict(soort="CORRECT", regel="implements", bron=bron, doel=doel,
                                 reden="sleutel resolvet"))
        else:
            elders = by_law_term[target_law].get(term or "", [])
            if elders:
                reden = (f"open term '{term}' bestaat in {target_law}, maar onder artikel "
                         f"{', '.join(elders)} — niet onder '{target_article}'")
            elif (target_law, target_article) in declared:
                reden = (f"artikel {target_article} van {target_law} bestaat en declareert open terms "
                         f"({', '.join(sorted(declared[(target_law, target_article)]))}), maar niet '{term}'")
            else:
                reden = (f"artikel {target_article} van {target_law} declareert geen open terms "
                         f"(nummer bestaat niet of is anders genoteerd)")
            gevolg = ""
            for (law, art, tid), meta in open_term_meta.items():
                if law == target_law and tid == term and meta.get("default") is not None:
                    gevolg = " — de open term heeft een default, dus dit faalt STIL"
                    break
            findings.append(dict(soort="DOOD", regel="implements", bron=bron, doel=doel,
                                 reden=reden + gevolg))

    # 2 - open_terms zonder invuller
    ingevuld = {(i.get("law"), str(i.get("article")), i.get("open_term")) for i in implements}
    for (law_id, number, tid), meta in sorted(open_term_meta.items()):
        if (law_id, number, tid) in ingevuld:
            continue
        heeft_default = meta.get("default") is not None
        findings.append(dict(
            soort="ONBEANTWOORD", regel="open_term", bron=f"{law_id}:{number}:{tid}", doel="-",
            reden=("geen enkel implements-blok richt zich op deze sleutel"
                   + (" — er is een default, dus de engine valt daar stil op terug"
                      if heeft_default else " — geen default: de waarde blijft leeg")),
        ))

    # 3 - legal_character zonder hook
    for char, producers in sorted(produced_characters.items()):
        if char in hook_characters:
            findings.append(dict(soort="CORRECT", regel="legal_character", bron=char,
                                 doel=f"{len(producers)} artikel(en)", reden="er filtert een hook op"))
        else:
            findings.append(dict(
                soort="ONBEANTWOORD", regel="legal_character", bron=char,
                doel=f"{len(producers)} artikel(en)",
                reden="geen enkele hook filtert op dit legal_character in het geladen corpus",
            ))

    return findings


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("corpus", type=Path)
    p.add_argument("--peildatum", default="9999-12-31")
    p.add_argument("--json", action="store_true")
    args = p.parse_args()

    root = args.corpus / "nl" if (args.corpus / "nl").is_dir() else args.corpus
    corpus = load_corpus(root, args.peildatum)
    if not corpus:
        print(f"Geen wetten gevonden onder {root}", file=sys.stderr)
        return 2

    findings = audit(corpus)
    if args.json:
        print(json.dumps(findings, ensure_ascii=False, indent=2))
        return 0

    print(f"{len(corpus)} wetten geladen (peildatum {args.peildatum})\n")
    for soort in ("DOOD", "ONBEANTWOORD", "CORRECT"):
        groep = [f for f in findings if f["soort"] == soort]
        print(f"== {soort} ({len(groep)})")
        for f in groep:
            pijl = f" -> {f['doel']}" if f["doel"] != "-" else ""
            print(f"  [{f['regel']}] {f['bron']}{pijl}\n      {f['reden']}")
        print()
    return 1 if any(f["soort"] == "DOOD" for f in findings) else 0


if __name__ == "__main__":
    sys.exit(main())
