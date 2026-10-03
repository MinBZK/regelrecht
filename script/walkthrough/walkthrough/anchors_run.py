"""Measure scroll anchors for a take recorded without them.

    python anchors_run.py <timeline.json> <viewport.json> <out.json> [url]

A take from before the recorder logged scroll anchors only knows its scroll
positions in pixels, which hold for the layout it was recorded in and no
other. This replays the walkthrough on the dev server in that layout (the
viewport and pixel ratio from the take's meta.json), applies every chapter's
actions fast, and after each scroll asks the page what is in the middle of
the scroller: the anchor the recorder would have logged. Run by
`walkthrough anchors`, in its own uv environment with Playwright.
"""

import json
import sys

from playwright.sync_api import sync_playwright


def main() -> None:
    timeline_path, viewport_path, out_path = sys.argv[1:4]
    base = sys.argv[4] if len(sys.argv) > 4 else "http://127.0.0.1:7400"
    timeline = json.load(open(timeline_path))
    vp = json.load(open(viewport_path))
    found: dict[str, dict] = {}
    with sync_playwright() as p:
        browser = p.chromium.launch(channel="chrome", headless=True, args=["--mute-audio"])
        page = browser.new_context(viewport={"width": vp["width"], "height": vp["height"]}, device_scale_factor=vp.get("dpr", 1)).new_page()
        tracks = [("main", None, timeline["main"])] + [(f"faq-{f['id']}", f["id"], f) for f in timeline.get("faq") or []]
        for name, faq_id, track in tracks:
            if not any(e.get("type") == "scroll" and e.get("src") and not e.get("anchor") for e in track.get("events") or []):
                continue
            page.goto(f"{base}/rondleiding" + (f"/{faq_id}" if faq_id else ""))
            page.wait_for_function("() => window.__rrReplay && window.__rrReplay.replay.active", timeout=120000)
            page.evaluate(
                """() => {
                  window.__anchors = [];
                  window.__rrReplay.afterScroll = (e, el) => {
                    if (!e.src || e.anchor) return;
                    window.__anchors.push({ src: e.src, max: Math.round(el.scrollHeight - el.clientHeight), anchor: window.__rrReplay.scrollAnchor(el) });
                  };
                }"""
            )
            for c in track["chapters"]:
                end = round(c["end"] - 0.05, 3)
                page.evaluate(f"() => window.__rrReplay.seek({end}, {{ play: false }})")
                page.wait_for_function(f"() => Math.abs(window.__rrReplay.replay.now - {end}) < 0.01", timeout=300000)
            for a in page.evaluate("() => window.__anchors"):
                found[json.dumps(a["src"])] = {"max": a["max"], "anchor": a["anchor"]}
            print(f"{name}: {len(found)} scroll anchors", file=sys.stderr)
        browser.close()
    json.dump(found, open(out_path, "w"))


if __name__ == "__main__":
    main()
