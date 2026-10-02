#!/usr/bin/env python3
"""Tiny helper to run the regelrecht evaluate binary against the Wpp law.

Usage: uv run scenarios/run_eval.py '<output>' '<params-json>'
Builds the JSON payload (law + extra laws) and pipes it to the evaluate binary,
avoiding shell escaping of the YAML.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LAW = ROOT / "law"
EVALUATE = Path(
    "/Users/anneschuth/regelrecht/packages/target/release/evaluate"
)

WPP = (LAW / "wet_op_de_politieke_partijen" / "2026-01-01.yaml").read_text()
EXTRA = [
    (LAW / "regeling_subsidiebedragen" / "2026-01-01.yaml").read_text(),
    (LAW / "besluit_subsidiering_decentrale_politieke_partijen" / "2026-01-01.yaml").read_text(),
    (LAW / "algemene_wet_bestuursrecht" / "1994-01-01.yaml").read_text(),
    (LAW / "kieswet" / "1989-09-28.yaml").read_text(),
]


def evaluate(output_names, params, date="2026-06-01"):
    payload = {
        "law_yaml": WPP,
        "output_names": output_names,
        "params": params,
        "date": date,
        "extra_laws": EXTRA,
    }
    proc = subprocess.run(
        [str(EVALUATE)],
        input=json.dumps(payload),
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        print("STDERR:", proc.stderr, file=sys.stderr)
    return json.loads(proc.stdout)


if __name__ == "__main__":
    outputs = json.loads(sys.argv[1]) if len(sys.argv) > 1 else ["subsidiebedrag"]
    params = json.loads(sys.argv[2]) if len(sys.argv) > 2 else {}
    print(json.dumps(evaluate(outputs, params), indent=2, ensure_ascii=False))
