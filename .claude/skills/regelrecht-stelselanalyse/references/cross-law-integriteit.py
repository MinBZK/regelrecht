#!/usr/bin/env python3
"""Cross-law binding integrity check (corpus-agnostic).

Verifies that every cross-law source binding is REAL and RESOLVABLE:

  MISPLACED  - a `source:` block under `parameters:` (or anywhere other than the
               `input:` list). The engine's Parameter struct has no `source`
               field, so the binding is silently dropped at parse time and the
               value is treated as a plain direct parameter. Cross-law never fires.
  DANGLING   - a `source: {regulation, output}` whose target law does not produce
               that output (engine fails at resolution: "variable not found").
  PLAIN-PARAM- an input whose description references another regulation
               ("conceptueel", "forward naar", "tijdelijk als directe parameter")
               but has no `source:` block at all.

  IMPL-DANGLING - an `implements: {law, article, open_term}` whose target does
               not declare that open_term on that law+article (the IoC binding
               points at nothing; the delegation never resolves).
  IMPL-NO-DATE - a regulation that carries an `implements:` block but has no
               top-level `valid_from`. RFC-003's temporal filter then matches it
               for EVERY calculation date, silently overriding the correct
               version. Implementing regulations must be dated.

All findings are MODELLING ERRORS, never engine limitations. Exit code != 0 if
any are found (usable as a CI gate).

Usage:  python3 cross-law-integriteit.py [corpus_root]   (default: regulation)
"""
import sys, glob

try:
    import yaml
except ImportError:
    sys.stderr.write("cross-law-integriteit.py requires pyyaml (pip install pyyaml)\n")
    sys.exit(2)

root = sys.argv[1] if len(sys.argv) > 1 else 'regulation'

laws = {}
for path in glob.glob(f'{root}/**/*.yaml', recursive=True):
    try:
        doc = yaml.safe_load(open(path))
    except Exception:
        continue
    if isinstance(doc, dict) and '$id' in doc:
        laws[doc['$id']] = doc


def action_outputs(doc):
    outs = set()
    for art in doc.get('articles', []) or []:
        ex = (art.get('machine_readable') or {}).get('execution') or {}
        for a in ex.get('actions', []) or []:
            if isinstance(a, dict) and 'output' in a:
                outs.add(a['output'])
        for o in ex.get('output', []) or []:
            if isinstance(o, dict) and 'name' in o:
                outs.add(o['name'])
    return outs


law_outputs = {lid: action_outputs(doc) for lid, doc in laws.items()}


def declared_open_terms(doc):
    # law_id -> {article_number(str) -> set(open_term ids)}
    idx = {}
    for art in doc.get('articles', []) or []:
        num = str(art.get('number', '?'))
        mr = art.get('machine_readable') or {}
        ids = {o.get('id') for o in (mr.get('open_terms') or []) if isinstance(o, dict)}
        if ids:
            idx.setdefault(num, set()).update(ids)
    return idx


open_terms_idx = {lid: declared_open_terms(doc) for lid, doc in laws.items()}

# Markers that signal "this SHOULD be a cross-law binding but isn't". Note: do NOT
# include generic words like "forward naar" — those legitimately describe leaf
# parameters that FEED a binding's parameters-mapping (e.g. an upstream data field).
PLAIN_MARKERS = ('conceptueel', 'tijdelijk als directe parameter')
misplaced, dangling, plain, ok = [], [], [], 0
impl_dangling, impl_nodate = [], []

for lid, doc in laws.items():
    # IMPL-NO-DATE: any implements block in a regulation without a valid_from.
    has_implements = any(
        (a.get('machine_readable') or {}).get('implements')
        for a in doc.get('articles', []) or []
    )
    if has_implements and not doc.get('valid_from'):
        impl_nodate.append(f'{lid}: implements zonder valid_from (matcht elke datum)')

    for art in doc.get('articles', []) or []:
        num = art.get('number', '?')
        mr = art.get('machine_readable') or {}
        ex = mr.get('execution') or {}

        # IMPL-DANGLING: implements must point at a declared open_term.
        for im in mr.get('implements', []) or []:
            if not isinstance(im, dict):
                continue
            tlaw, tart, term = im.get('law'), str(im.get('article')), im.get('open_term')
            if tlaw not in open_terms_idx:
                impl_dangling.append(f'{lid} art {num}: implements onbekende wet {tlaw}')
            elif term not in open_terms_idx[tlaw].get(tart, set()):
                impl_dangling.append(f'{lid} art {num}: {tlaw} art {tart} declareert open_term "{term}" niet')
            else:
                ok += 1

        # MISPLACED: any source under parameters: (engine ignores it).
        # Also flag plain-param placeholders (description names another law, no source).
        for p in ex.get('parameters', []) or []:
            if not isinstance(p, dict):
                continue
            if p.get('source'):
                misplaced.append(f'{lid} art {num}: {p.get("name")} (source onder parameters: -> genegeerd; verplaats naar input:)')
            else:
                d = (p.get('description') or '').lower()
                if any(mk in d for mk in PLAIN_MARKERS):
                    plain.append(f'{lid} art {num}: {p.get("name")}')

        # input: real cross-law bindings -> resolvability (dangling) check
        for inp in ex.get('input', []) or []:
            if not isinstance(inp, dict):
                continue
            src = inp.get('source')
            if not isinstance(src, dict):
                continue
            reg, out = src.get('regulation'), src.get('output')
            if reg is None and out is None:
                continue  # data-registry binding (source: {})
            if reg is None:  # intra-law reference
                if out not in law_outputs.get(lid, set()):
                    dangling.append(f'{lid} art {num}: intra-law {out} bestaat niet')
                else:
                    ok += 1
            else:
                if reg not in law_outputs or (out is not None and out not in law_outputs[reg]):
                    dangling.append(f'{lid} art {num}: {reg}.{out} bestaat niet in doelwet')
                else:
                    ok += 1

print(f'clean={ok} misplaced={len(misplaced)} dangling={len(dangling)} '
      f'plain-param={len(plain)} impl-dangling={len(impl_dangling)} impl-no-date={len(impl_nodate)}')
for x in misplaced:
    print('  MISPLACED', x)
for x in dangling:
    print('  DANGLING', x)
for x in plain:
    print('  PLAIN', x)
for x in impl_dangling:
    print('  IMPL-DANGLING', x)
for x in impl_nodate:
    print('  IMPL-NO-DATE', x)
sys.exit(1 if (misplaced or dangling or plain or impl_dangling or impl_nodate) else 0)
