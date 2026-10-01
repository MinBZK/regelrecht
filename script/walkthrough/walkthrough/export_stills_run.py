"""Screenshot the walkthrough player once per chapter, without its video.

Run in its own environment (export.py starts it with `uv run --with
playwright`). Takes a JSON list of jobs on stdin:

    [{"url": "http://localhost:7400/rondleiding?still=12.5&track=main", "out": "/tmp/x.png"}, ...]

The player's still mode (`?still=`) renders the deck as it stands at that
moment, with the video, the bubble, the captions and the controls left out:
export.py lays those over it with ffmpeg. Uses the installed Chrome, and
falls back to Playwright's own Chromium.
"""

import json
import sys


def main() -> None:
    from playwright.sync_api import sync_playwright

    jobs = json.load(sys.stdin)
    with sync_playwright() as p:
        try:
            browser = p.chromium.launch(channel="chrome")
        except Exception:
            browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": 1920, "height": 1080}, device_scale_factor=1, color_scheme="light")
        layout = """() => {
          const rect = (sel) => {
            const r = document.querySelector(sel)?.getBoundingClientRect();
            return r && r.width > 0 ? { x: r.x, y: r.y, width: r.width, height: r.height } : null;
          };
          return { stage: rect('.wt-stage'), cam: rect('.cam-slot'), full: !!document.querySelector('.walkthrough.full') };
        }"""
        for job in jobs:
            page.goto(job["url"], wait_until="networkidle")
            page.wait_for_selector("[data-still-ready]", timeout=30000)
            page.screenshot(path=job["out"])
            # Where the page puts the stage and the bubble on this slide, so
            # export.py lays the videos exactly there.
            with open(job["out"] + ".json", "w") as f:
                json.dump(page.evaluate(layout), f)
        browser.close()


if __name__ == "__main__":
    main()
