# /// script
# dependencies = []
# ///
"""Make every cross-law call to `wet_brp` pass `referentiedatum` explicitly.

`wet_brp` determines a person's age on a reference date. Under RFC-036 an
optional parameter the caller leaves out is *unknown*, which made the age, and
everything that depends on it, unknown for every caller that relied on the old
fallback to the calculation date. The date is the calling law's decision, so
the caller writes it: `referentiedatum: $referencedate` ("as of the calculation
date") everywhere, except where a law already names another date (the kieswet
passes its election date).

Text-based on purpose: the demo laws carry comments at positions a YAML
round-trip would move. Idempotent; run from the repository root:

    uv run corpus/demo/tools/pass_brp_referentiedatum.py corpus/demo/regulation/nl
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

PARAM = "referentiedatum"
DEFAULT_VALUE = "$referencedate"


def rewrite(text: str) -> tuple[str, int]:
    lines = text.split("\n")
    out: list[str] = []
    added = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        out.append(line)
        m = re.match(r"^(\s*)regulation: wet_brp\s*$", line)
        if not m:
            i += 1
            continue
        indent = m.group(1)
        # Walk the rest of this `source:` block: lines indented deeper than
        # `regulation:` belong to it (output:, parameters: and its entries).
        j = i + 1
        block: list[str] = []
        while j < len(lines) and lines[j].startswith(indent) and not lines[j][len(indent):].startswith("-"):
            # A sibling key of `regulation:` (same indent) or one of its children
            # (deeper); the next input starts with `- name:` at a shallower indent.
            block.append(lines[j])
            j += 1
        params_at = next((k for k, l in enumerate(block) if l.strip() == "parameters:"), None)
        if params_at is None:
            # A call without parameters: add a parameters block.
            block.append(f"{indent}parameters:")
            block.append(f"{indent}  {PARAM}: {DEFAULT_VALUE}")
            added += 1
        else:
            entries_end = params_at + 1
            while entries_end < len(block) and block[entries_end].startswith(indent + "  ") and block[entries_end].strip():
                entries_end += 1
            entries = block[params_at + 1 : entries_end]
            if not any(re.match(rf"^\s*{PARAM}:", e) for e in entries):
                block.insert(entries_end, f"{indent}  {PARAM}: {DEFAULT_VALUE}")
                added += 1
        out.extend(block)
        i = j
    return "\n".join(out), added


def main() -> None:
    root = Path(sys.argv[1])
    total = 0
    for path in sorted(root.rglob("*.yaml")):
        text = path.read_text()
        if "regulation: wet_brp\n" not in text:
            continue
        new, added = rewrite(text)
        if added:
            path.write_text(new)
            total += added
            print(f"{path}: {added} call(s) now pass {PARAM}")
    print(f"{total} wet_brp call(s) rewritten")


if __name__ == "__main__":
    main()
