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

All three are MODELLING ERRORS, never engine limitations. Exit code != 0 if any
are found (usable as a CI gate).

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

# Markers that signal "this SHOULD be a cross-law binding but isn't". Note: do NOT
# include generic words like "forward naar" — those legitimately describe leaf
# parameters that FEED a binding's parameters-mapping (e.g. an upstream data field).
PLAIN_MARKERS = ('conceptueel', 'tijdelijk als directe parameter')
misplaced, dangling, plain, ok = [], [], [], 0

for lid, doc in laws.items():
    for art in doc.get('articles', []) or []:
        num = art.get('number', '?')
        ex = (art.get('machine_readable') or {}).get('execution') or {}

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

print(f'clean={ok} misplaced={len(misplaced)} dangling={len(dangling)} plain-param={len(plain)}')
for x in misplaced:
    print('  MISPLACED', x)
for x in dangling:
    print('  DANGLING', x)
for x in plain:
    print('  PLAIN', x)
sys.exit(1 if (misplaced or dangling or plain) else 0)
