#!/usr/bin/env python3
"""HTML source -> reading text (the HTML branch of fase 1).

The PDF branch has pdftotext; HTML needs its own deterministic reader. Same
contract: same bytes in, same text out, no injected characters, no judgement.
Stdlib only - a converter that needs an install is a converter that silently
differs between machines, and then canonical.md stops being evidence.

What it drops, and why that is the HTML equivalent of a running header: a
handbook page carries a navigation tree, a breadcrumb, a search box and a
footer on every page. Left in, they become "text" that trips the signal net on
every single page and buries the actual content. They are furniture, so they go
- but visibly, in one place, and counted in the report.

Structure is preserved because the tiler needs it: headings on their own line,
list items with a leading marker, table rows as tab-separated lines, blocks
separated by a blank line.

Usage:  python3 html_canonical.py page.html
        python3 html_canonical.py page.html --keep-nav      (diagnose furniture)
        python3 html_canonical.py page.html --root '#content'
"""
import argparse
import re
import sys
import unicodedata
from html.parser import HTMLParser

DROP = {"script", "style", "noscript", "svg", "canvas", "iframe", "form",
        "nav", "header", "footer", "aside", "button", "select", "template"}
# Labels that exist for an icon or a screen reader and are not visible text.
# A notification block typically renders an icon plus the word, but ships both
# the icon's label and the word - which reads back as "Let op! Let op". These
# class names are cross-framework conventions, not one site's markup.
HIDDEN_CLASS = re.compile(r"\b(sr-only|visually-hidden|screen-reader(-text)?|"
                          r"icon-label|a11y-hidden|hidden-visually)\b")
BLOCK = {"p", "div", "section", "article", "main", "ul", "ol", "dl", "dd", "dt",
         "table", "thead", "tbody", "figure", "figcaption", "blockquote", "pre",
         "h1", "h2", "h3", "h4", "h5", "h6", "tr", "li"}
HEADING = {"h1", "h2", "h3", "h4", "h5", "h6"}
VOID = {"br", "img", "hr", "input", "meta", "link", "source", "col"}
# Content wrappers, most specific first: a page that marks its own main content
# is telling us where the text is, and that beats any heuristic.
PREFERRED_ROOTS = ["main", "article"]


class Extract(HTMLParser):
    def __init__(self, keep_nav=False):
        super().__init__(convert_charrefs=True)
        self.keep_nav = keep_nav
        self.out = []
        self.drop_depth = 0
        self.dropped = 0
        self.hidden_labels = 0
        self.stack = []
        self.in_cell = False
        self.links = []          # [(anchor text, href)] in document order
        self.link_href = None
        self.link_start = None
        self.hidden_open = []
        self.pending_tabs = 0

    # -- helpers ----------------------------------------------------------
    def nl(self, n=1):
        while self.out and self.out[-1] in ("\n", " "):
            self.out.pop()
        self.out.append("\n" * n)

    def handle_starttag(self, tag, attrs):
        if tag in VOID:
            if tag == "br":
                self.out.append("\n")
            return
        self.stack.append(tag)
        attr = dict(attrs)
        if tag in DROP and not self.keep_nav:
            self.drop_depth += 1
            self.dropped += 1
            return
        if not self.keep_nav and HIDDEN_CLASS.search(attr.get("class") or ""):
            self.drop_depth += 1
            self.hidden_labels += 1
            self.hidden_open.append(tag)
            return
        if self.drop_depth:
            return
        if tag == "a" and attr.get("href"):
            self.link_href = attr["href"]
            self.link_start = len(self.out)
        if tag in HEADING:
            self.nl(2)
        elif tag == "li":
            self.nl()
            self.out.append("- ")
        elif tag in ("td", "th"):
            if self.in_cell:
                while self.out and self.out[-1] == " ":
                    self.out.pop()
                self.out.append("\t")
            self.in_cell = True
            # A cell that spans N columns must still advance the column count,
            # or every value after it shifts one column left and ends up under
            # the wrong header. That is a silent wrong answer, not a layout nit.
            try:
                span = int(attr.get("colspan", "1"))
            except ValueError:
                span = 1
            if span > 1:
                self.pending_tabs = span - 1
            else:
                self.pending_tabs = 0
        elif tag in BLOCK:
            self.nl(2)

    def handle_endtag(self, tag):
        if tag in VOID:
            return
        if self.hidden_open and self.hidden_open[-1] == tag:
            self.hidden_open.pop()
            self.drop_depth = max(0, self.drop_depth - 1)
            return
        if tag in self.stack:
            while self.stack and self.stack.pop() != tag:
                pass
        if tag in DROP and not self.keep_nav:
            self.drop_depth = max(0, self.drop_depth - 1)
            return
        if self.drop_depth:
            return
        if tag == "a" and self.link_href:
            anchor = "".join(self.out[self.link_start:]).strip()
            if anchor:
                self.links.append((anchor, self.link_href))
            self.link_href = None
        if tag in ("td", "th") and self.pending_tabs:
            self.out.append("\t" * self.pending_tabs)
            self.pending_tabs = 0
        if tag == "tr":
            self.in_cell = False
            self.pending_tabs = 0
            self.nl()
        elif tag == "li":
            self.nl()          # list items stay adjacent; a list is one block
        elif tag in HEADING or tag in BLOCK:
            self.nl(2)

    def handle_data(self, data):
        if self.drop_depth or not data.strip():
            if data.strip() == "" and self.out and not self.out[-1].endswith("\n"):
                self.out.append(" ")
            return
        self.out.append(re.sub(r"\s+", " ", data).strip())
        self.out.append(" ")

    def text(self):
        raw = "".join(self.out)
        raw = unicodedata.normalize("NFC", raw)
        raw = raw.replace("­", "")
        # Spaces only, never tabs: a tab at the start or end of a line is an
        # empty leading/trailing table cell. Stripping it collapses the column
        # count and every value in the row lands under the wrong header.
        raw = re.sub(r"[ ]+\n", "\n", raw)
        raw = re.sub(r"\n[ ]+", "\n", raw)
        raw = re.sub(r"\n{3,}", "\n\n", raw)
        return raw.strip() + "\n"


def narrow_to_root(html: str, root: str | None) -> tuple[str, str]:
    """Return (html, which-root-was-used). Cuts to <main>/<article> when present."""
    if root:
        m = re.search(rf'<[^>]*(?:id|class)="[^"]*{re.escape(root.lstrip("#."))}[^"]*"[^>]*>',
                      html, re.I)
        if m:
            return html[m.end():], root
        sys.stderr.write(f"waarschuwing: --root {root!r} niet gevonden, hele document gebruikt\n")
        return html, "document"
    for tag in PREFERRED_ROOTS:
        m = re.search(rf"<{tag}\b[^>]*>(.*?)</{tag}>", html, re.I | re.S)
        if m:
            return m.group(1), f"<{tag}>"
    return html, "document"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("source")
    ap.add_argument("--keep-nav", action="store_true",
                    help="laat navigatie/kop/voet staan - om te zien wat er anders wegvalt")
    ap.add_argument("--root", help="CSS-achtige id/class van het inhoudsblok, bijv. '#content'")
    ap.add_argument("--links", metavar="FILE",
                    help="schrijf de hyperlinks weg als zijspoor (volgnr, ankertekst, href)")
    args = ap.parse_args()

    with open(args.source, encoding="utf-8", errors="replace") as fh:
        html = fh.read()

    body = re.search(r"<body\b[^>]*>(.*)</body>", html, re.I | re.S)
    if body:
        html = body.group(1)
    html, used = narrow_to_root(html, args.root)

    parser = Extract(keep_nav=args.keep_nav)
    parser.feed(html)
    text = parser.text()

    sys.stdout.write(text)

    # Links live beside the text, never in it. A hyperlink on "artikel 8 van de
    # Awir" is the author saying which norm they mean - exactly what the
    # verankering step reconstructs by hand - so throwing it away wastes
    # evidence. Inlining the URL would wreck verbatim quoting, hence a sidecar.
    if args.links:
        with open(args.links, "w", encoding="utf-8") as fh:
            fh.write("n\tanker\thref\n")
            for n, (anchor, href) in enumerate(parser.links, start=1):
                fh.write(f"{n}\t{anchor}\t{href}\n")

    sys.stderr.write(f"inhoudsblok={used} weggelaten_elementen={parser.dropped} "
                     f"verborgen_labels={parser.hidden_labels} links={len(parser.links)} "
                     f"chars={len(text)}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
