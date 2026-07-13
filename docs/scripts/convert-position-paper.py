#!/usr/bin/env python3
"""Convert position-paper.tex to HTML for the regelrecht docs site.

Usage: python3 docs/scripts/convert-position-paper.py
Needs: pandoc (with citeproc), and the paper source checked out at
~/regelrecht-research/position-paper. Regenerates
docs/src/research/rules-as-executed.html and its headings JSON.
The figure SVG in docs/public/research/ is produced separately:
compile figures/depgraph-zorgtoeslag.tex standalone (\documentclass[tikz]
{standalone}) and run pdftocairo -svg on the result.

Pipeline: extract listings -> strip comments -> expand glossaries-extra
macros (first-use semantics) -> number sections/figures/tables -> restructure
floats -> resolve \\ref -> pandoc (citeproc) -> postprocess HTML
(nldd-code-viewer blocks, hand-built landscape table, headings JSON).
"""

import json
import re
import subprocess
import sys
from pathlib import Path

SRC = Path("/Users/anneschuth/regelrecht-research/position-paper")
TEX = SRC / "position-paper.tex"
GLOSSARY = SRC / "glossary.tex"
BIB = SRC / "regelrecht.bib"
OUT_DIR = Path("/Users/anneschuth/regelrecht/docs/src/research")
SCRATCH = Path("/tmp")


# --------------------------------------------------------------------------
# Balanced-brace helpers
# --------------------------------------------------------------------------

def read_group(s: str, i: int) -> tuple[str, int]:
    """Read a {…} group starting at s[i] == '{'; return (content, next_index)."""
    assert s[i] == "{", f"expected {{ at {i}: {s[i:i+30]!r}"
    depth = 0
    start = i + 1
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            i += 2
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return s[start:i], i + 1
        i += 1
    raise ValueError("unbalanced braces")


def read_optional(s: str, i: int) -> tuple[str | None, int]:
    """Read an optional [...] argument (brace-aware) if present at s[i]."""
    if i >= len(s) or s[i] != "[":
        return None, i
    depth = 0
    start = i + 1
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            i += 2
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
        elif c == "]" and depth == 0:
            return s[start:i], i + 1
        i += 1
    raise ValueError("unbalanced optional arg")


def split_kv(body: str) -> dict[str, str]:
    """Split a key=value,key={value} list (glossaries entry body)."""
    out = {}
    i = 0
    n = len(body)
    while i < n:
        # key
        m = re.match(r"\s*([a-zA-Z0-9]+)\s*=\s*", body[i:])
        if not m:
            break
        key = m.group(1)
        i += m.end()
        if i < n and body[i] == "{":
            val, i = read_group(body, i)
        else:
            j = i
            depth = 0
            while j < n and (body[j] != "," or depth > 0):
                if body[j] == "{":
                    depth += 1
                elif body[j] == "}":
                    depth -= 1
                j += 1
            val = body[i:j].strip()
            i = j
        out[key] = val
        # skip comma
        while i < n and body[i] in ", \n\t%":
            i += 1
    return out


# --------------------------------------------------------------------------
# Glossary parsing
# --------------------------------------------------------------------------

class Entry:
    def __init__(self, kind, key, short=None, long=None, longplural=None,
                 name=None, desc=None, url=None):
        self.kind = kind  # 'acronym' | 'dutchterm' | 'propernoun'
        self.key = key
        self.short = short
        self.long = long
        self.longplural = longplural
        self.name = name
        self.desc = desc
        self.url = url


def parse_glossary(text: str) -> dict[str, Entry]:
    entries: dict[str, Entry] = {}
    # \newabbreviation[opts]{key}{short}{long}
    for m in re.finditer(r"\\newabbreviation", text):
        i = m.end()
        opts, i = read_optional(text, i)
        key, i = read_group(text, i)
        short, i = read_group(text, i)
        long, i = read_group(text, i)
        longplural = None
        if opts:
            kv = split_kv(opts)
            longplural = kv.get("longplural")
        entries[key] = Entry("acronym", key, short=short, long=long,
                             longplural=longplural)
    # \newglossaryentry{key}{body}
    for m in re.finditer(r"\\newglossaryentry", text):
        i = m.end()
        key, i = read_group(text, i)
        body, i = read_group(text, i)
        kv = split_kv(body)
        kind = kv.get("type", kv.get("category", ""))
        entries[key] = Entry(kind, key, name=kv.get("name"),
                             desc=kv.get("description"),
                             url=kv.get("user1"))
    return entries


# --------------------------------------------------------------------------
# Glossary expansion (stateful first use)
# --------------------------------------------------------------------------

def clean_protect(s: str) -> str:
    return s.replace("\\protect\\footnote", "\\footnote")


def expand_glossary(body: str, entries: dict[str, Entry]) -> str:
    used: set[str] = set()
    out = []
    i = 0
    n = len(body)
    pat = re.compile(r"\\(glsresetall|glsentryname|glspl|gls|Glspl|Gls)\b")
    while i < n:
        m = pat.search(body, i)
        if not m:
            out.append(body[i:])
            break
        out.append(body[i:m.start()])
        cmd = m.group(1)
        j = m.end()
        if cmd == "glsresetall":
            used.clear()
            i = j
            continue
        key, j = read_group(body, j)
        e = entries.get(key)
        if e is None:
            sys.exit(f"unknown glossary key: {key}")
        plural = cmd in ("glspl", "Glspl")
        capital = cmd in ("Gls", "Glspl")
        if cmd == "glsentryname":
            text = e.short if e.kind == "acronym" else e.name
        elif e.kind == "acronym":
            first = key not in used
            used.add(key)
            if first:
                long = e.longplural if (plural and e.longplural) else (
                    e.long + "s" if plural else e.long)
                short = e.short + "s" if plural else e.short
                # footnote must come after the closing paren, per the
                # long-short style the paper compiles with the footnote is
                # part of the long form; keep it inline where it appears.
                text = f"{clean_protect(long)} ({short})"
            else:
                text = e.short + "s" if plural else e.short
        elif e.kind == "dutchterm":
            first = key not in used
            used.add(key)
            name = e.name + "s" if plural else e.name
            if capital:
                name = name[0].upper() + name[1:]
            text = f"\\emph{{{name}}}"
            if first and e.desc:
                text += f" ({e.desc})"
        elif e.kind == "propernoun":
            first = key not in used
            used.add(key)
            name = e.name + "s" if plural else e.name
            if capital:
                name = name[0].upper() + name[1:]
            text = name
            if first:
                if e.desc:
                    text += f" ({e.desc})"
                if e.url:
                    text += f"\\footnote{{\\url{{{e.url}}}}}"
        else:
            sys.exit(f"unknown entry kind {e.kind} for {key}")
        if capital and e.kind == "acronym":
            text = text[0].upper() + text[1:]
        out.append(text)
        i = j
    return "".join(out)


# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------

def main():
    tex = TEX.read_text()
    body = tex.split("\\begin{document}", 1)[1].split("\\end{document}", 1)[0]

    # -- 1. extract listings ------------------------------------------------
    listings: list[tuple[str, str]] = []  # (language, code)

    def stash_listing(m):
        env = m.group(1)
        code = m.group(2).strip("\n")
        lang = {"narra": "yaml", "enginetrace": "text"}[env]
        listings.append((lang, code))
        return f"\n\nRRCODEBLOCK{len(listings) - 1}\n\n"

    body = re.sub(
        r"\\begin\{(narra|enginetrace)\}\n(.*?)\\end\{\1\}",
        stash_listing, body, flags=re.S)

    # -- 2. strip comments ----------------------------------------------------
    body = re.sub(r"(?m)^\s*%.*\n", "", body)
    body = re.sub(r"(?<!\\)%.*", "", body)

    # -- 3. abstract / boilerplate -------------------------------------------
    body = body.replace("\\maketitle", "")
    body = body.replace("\\begin{abstract}", "\\section*{Abstract}\n")
    body = body.replace("\\end{abstract}", "")

    # -- 4. glossary expansion (before numbering; captions included) ----------
    entries = parse_glossary(GLOSSARY.read_text())
    body = expand_glossary(body, entries)

    # -- 5. section numbering --------------------------------------------------
    labels: dict[str, str] = {}
    sec = 0
    sub = 0

    def number_heading(m):
        nonlocal sec, sub
        cmd = m.group(1)
        star = m.group(2)
        rest = m.group(0)[m.end(2) - m.start(0):]
        # read optional + required arg from the full source at match position
        return m.group(0)  # placeholder; replaced by loop below

    # Do it with a manual scan (headings can contain \\ and optional args).
    out = []
    i = 0
    pat = re.compile(r"\\(section|subsection)(\*?)")
    while True:
        m = pat.search(body, i)
        if not m:
            out.append(body[i:])
            break
        out.append(body[i:m.start()])
        cmd, star = m.group(1), m.group(2)
        j = m.end()
        _opt, j = read_optional(body, j)
        title, j = read_group(body, j)
        title = title.replace("\\\\", " ").strip()
        title = re.sub(r"\s+", " ", title)
        num = None
        if not star:
            if cmd == "section":
                sec += 1
                sub = 0
                num = f"{sec}"
            else:
                sub += 1
                num = f"{sec}.{sub}"
        # label directly after?
        lm = re.match(r"\s*\\label\{([^}]*)\}", body[j:])
        label = None
        if lm:
            label = lm.group(1)
            j += lm.end()
        if num and label:
            labels[label] = num
        shown = f"{num} {title}" if num else title
        out.append(f"\\{cmd}{star}{{{shown}}}")
        if label:
            out.append(f"\\label{{{label}}}")
        i = j
    body = "".join(out)

    # -- 6. figures & tables ----------------------------------------------------
    fig_no = 0
    tab_no = 0

    def do_figure(m):
        nonlocal fig_no
        content = m.group(1)
        fig_no += 1
        cap = re.search(r"\\caption\{", content)
        caption, _ = read_group(content, cap.end() - 1)
        lm = re.search(r"\\label\{([^}]*)\}", content)
        label = lm.group(1)
        labels[label] = str(fig_no)
        # body of the figure without caption/label/centering commands
        inner = content[:cap.start()]
        inner = (inner.replace("\\centering", "")
                      .replace("\\small", "").strip())
        rb = re.search(r"\\resizebox\{[^}]*\}\{[^}]*\}\{\\input\{([^}]*)\}\}",
                       inner)
        if rb:
            name = Path(rb.group(1)).name
            inner = (f"\\begin{{center}}"
                     f"\\includegraphics{{/research/{name}.svg}}"
                     f"\\end{{center}}")
        return (f"\n\n{inner}\n\n"
                f"\\hypertarget{{{label}}}{{}}\\textbf{{Figure {fig_no}:}} "
                f"{caption}\n\n")

    def do_table(m):
        nonlocal tab_no
        content = m.group(1)
        tab_no += 1
        cap = re.search(r"\\caption\{", content)
        caption, _ = read_group(content, cap.end() - 1)
        lm = re.search(r"\\label\{([^}]*)\}", content)
        label = lm.group(1)
        labels[label] = str(tab_no)
        return (f"\n\n\\hypertarget{{{label}}}{{}}\\textbf{{Table {tab_no}:}} "
                f"{caption}\n\nRRTABLE{tab_no}\n\n")

    body = re.sub(r"\\begin\{figure\}(?:\[[^\]]*\])?(.*?)\\end\{figure\}",
                  do_figure, body, flags=re.S)
    body = re.sub(r"\\begin\{table\}(?:\[[^\]]*\])?(.*?)\\end\{table\}",
                  do_table, body, flags=re.S)

    # -- 7. \ref resolution ------------------------------------------------------
    unresolved = []

    def do_ref(m):
        label = m.group(1)
        if label not in labels:
            unresolved.append(label)
            return m.group(0)
        return f"\\hyperref[{label}]{{{labels[label]}}}"

    body = re.sub(r"\\ref\{([^}]*)\}", do_ref, body)
    if unresolved:
        sys.exit(f"unresolved refs: {unresolved}")

    # -- 8. references heading -----------------------------------------------------
    body = body.replace("\\printbibliography", "\\section*{References}")

    pre = SCRATCH / "pre.tex"
    pre.write_text(body)

    # -- 9. pandoc -------------------------------------------------------------------
    html = subprocess.run(
        ["pandoc", str(pre), "-f", "latex", "-t", "html5",
         "--citeproc", "--bibliography", str(BIB),
         "--metadata", "link-citations=true",
         "--metadata", "lang=en-US",
         "--shift-heading-level-by=1",
         "--wrap=none"],
        capture_output=True, text=True, check=True).stdout

    # -- 10. postprocess ---------------------------------------------------------------
    # code blocks -> nldd-code-viewer
    def html_escape(s):
        return (s.replace("&", "&amp;").replace("<", "&lt;")
                 .replace(">", "&gt;"))

    def put_code(m):
        idx = int(m.group(1))
        lang, code = listings[idx]
        return (f'<nldd-code-viewer language="{lang}">'
                f"{html_escape(code)}</nldd-code-viewer>")

    html = re.sub(r"<p>RRCODEBLOCK(\d+)</p>", put_code, html)

    # landscape table (hand-built: multicolumn header pandoc cannot express)
    table_html = """<div class="rr-table-scroll"><table class="rr-landscape">
<thead>
<tr><td></td><th scope="colgroup" colspan="2"><em>authoring &amp; execution</em></th><th scope="col"><em>reader-side</em></th></tr>
<tr><th scope="col">Approach</th><th scope="col">exec.</th><th scope="col">publ.</th><th scope="col">bound to exec.</th></tr>
</thead>
<tbody>
<tr><td>Akoma Ntoso / LegalRuleML</td><td>&ndash;</td><td>yes</td><td>&ndash;</td></tr>
<tr><td>RegelSpraak / ALEF (Netherlands)</td><td>yes</td><td>part</td><td>&ndash;</td></tr>
<tr><td>FLINT / Calculemus (Netherlands)</td><td>yes</td><td>part</td><td>&ndash;</td></tr>
<tr><td>Oracle Policy Automation</td><td>yes</td><td>&ndash;</td><td>&ndash;</td></tr>
<tr><td>OpenFisca (France)</td><td>yes</td><td>yes</td><td>&ndash;</td></tr>
<tr><td>Catala (France)</td><td>yes</td><td>yes</td><td>&ndash;</td></tr>
<tr><td>Better Rules (New Zealand)</td><td>yes</td><td>yes</td><td>&ndash;</td></tr>
<tr><td>STOP/TPOD, toepasbare regel (Netherlands)</td><td>yes</td><td>yes</td><td>&ndash;</td></tr>
<tr class="rr-rule"><td>This paper</td><td>yes</td><td>yes</td><td>yes</td></tr>
</tbody>
</table></div>"""
    html = html.replace("<p>RRTABLE1</p>", table_html)

    # LaTeX control-space leaking out of .bib note fields ("nr.\ 287")
    html = html.replace("\\ ", " ")

    # \paragraph headings land on h5 after the +1 shift, one level below
    # their h3 subsection — demote to h4 so the outline has no jumps.
    html = html.replace("<h5", "<h4").replace("</h5>", "</h4>")

    # \url{wetten.overheid.nl} (scheme-less on purpose in print) must not
    # become a relative link on the web.
    html = re.sub(r'href="(?!https?:|#|/|mailto:)([^"]+)"',
                  r'href="https://\1"', html)

    # caption paragraphs get a class for styling (the anchor is the empty
    # <div id="fig:..."> pandoc emits from the \hypertarget just before)
    html = re.sub(
        r"<p>(<strong>(?:Figure|Table) \d+:</strong>)",
        r'<p class="rr-caption">\1', html)

    # meaningful alt text for the one real image
    html = html.replace(
        'alt="image"',
        'alt="Dependency graph: the Wet op de zorgtoeslag at the center, '
        'with labeled arrows to the ten regulations it draws on"')

    # -- 11. headings for the outline ------------------------------------------------
    headings = [
        {"depth": int(m.group(1)), "slug": m.group(2), "text": re.sub(r"<[^>]+>", "", m.group(3))}
        for m in re.finditer(r'<h([23]) id="([^"]*)"[^>]*>(.*?)</h\1>', html)
    ]

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    (OUT_DIR / "rules-as-executed.html").write_text(html)
    (OUT_DIR / "rules-as-executed.headings.json").write_text(
        json.dumps(headings, indent=2))
    print(f"sections: {sec}, figures: {fig_no}, tables: {tab_no}, "
          f"headings: {len(headings)}, listings: {len(listings)}")
    print(f"wrote {OUT_DIR}/rules-as-executed.html ({len(html)} bytes)")


if __name__ == "__main__":
    main()
