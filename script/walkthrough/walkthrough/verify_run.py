"""Check that the built walkthrough replays: every chapter, every action.

    python verify_run.py <timeline.json> <width> <height> [url]

Opens the walkthrough on the dev server in a headless, muted Chrome, jumps
to the end of each chapter (which applies all of its actions, fast) and
reports the actions whose element was not found, per chapter, with the page
it ended on. Run by `walkthrough verify`. Exit code 1 when anything missed.
"""

import json
import sys

from playwright.sync_api import sync_playwright


def main() -> int:
    timeline = json.load(open(sys.argv[1]))
    width, height = int(sys.argv[2]), int(sys.argv[3])
    base = sys.argv[4] if len(sys.argv) > 4 else "http://127.0.0.1:7400"
    missed = 0
    with sync_playwright() as p:
        # Muted: a voice out of nowhere in someone's meeting is not a test result.
        browser = p.chromium.launch(channel="chrome", headless=True, args=["--mute-audio"])
        page = browser.new_context(viewport={"width": width, "height": height}).new_page()
        errors: list[str] = []
        page.on("pageerror", lambda e: errors.append(str(e)))
        tracks = [("main", None, timeline["main"])] + [(f"faq-{f['id']}", f["id"], f) for f in timeline.get("faq") or []]
        for name, faq_id, track in tracks:
            page.goto(f"{base}/rondleiding" + (f"/{faq_id}" if faq_id else ""))
            page.wait_for_function("() => window.__rrReplay && window.__rrReplay.replay.active", timeout=120000)
            print(f"{name} ({width}x{height})")
            seen = 0
            for i, c in enumerate(track["chapters"]):
                end = round(c["end"] - 0.3, 3)
                page.evaluate(f"() => window.__rrReplay.seek({end}, {{ play: false }})")
                page.wait_for_function(f"() => Math.abs(window.__rrReplay.replay.now - {end}) < 0.05", timeout=300000)
                misses = page.evaluate("() => window.__rrReplay.replay.misses.map((m) => ({ t: m.t, type: m.type }))")
                new = misses[seen:]
                seen = len(misses)
                missed += len(new)
                path = page.evaluate("() => location.pathname")
                mark = "ok" if not new else f"{len(new)} gemist: " + ", ".join(f"{m['type']} @ {m['t']:.1f}s" for m in new)
                print(f"  {i}. dia {c['slideIndex']}  {c['start']:.0f}-{c['end']:.0f}s  {path}  {mark}")
        if errors:
            print("paginafouten:", *errors[:3], sep="\n  ")
        browser.close()
    return 1 if missed or errors else 0


if __name__ == "__main__":
    sys.exit(main())
