"""A shareable MP4 per track: what the player shows, as one video file.

The player is a web page with a video in it; an MP4 is what goes into a mail
or onto LinkedIn. Rather than screen-record the player, this rebuilds the same
picture from its parts: one still of the deck per chapter (from the player's
still mode), the app video in the stage next to the rail, the presenter in a
circle where the player puts the bubble. The stills report where the stage
and the bubble sit, so the layout comes from the page and not from numbers
copied out of its stylesheet. Captions go in as a subtitle track and the
slides as MP4 chapters, so a viewer can turn the one on and jump through the
other.

The click ripples are left out: they help in the player, where the video is
small next to the rail, and add little in a full-screen MP4.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

from .media import FPS, ffmpeg

W, H = 1920, 1080
REM = 16
# Where the player's stylesheet puts the bubble on a slide that covers the
# screen (WalkthroughView.vue, `.cam`). In the rail it sits on `.cam-slot`,
# and the stills report that position per chapter.
CAM_FULL = {"x": W - 240 - 4 * REM, "y": H - 240 - 12 * REM, "width": 240, "height": 240}


def fit(box_w: float, box_h: float, w: int, h: int) -> tuple[int, int, int, int]:
    scale = min(box_w / w, box_h / h)
    sw, sh = int(w * scale) // 2 * 2, int(h * scale) // 2 * 2
    return sw, sh, int((box_w - sw) / 2), int((box_h - sh) / 2)


def windows(chapters: list[dict]) -> str:
    parts = [f"between(t,{c['start']:.3f},{c['end'] - 0.001:.3f})" for c in chapters]
    return "+".join(parts) or "0"


def circle(label_in: str, size: int, label_out: str) -> str:
    return (
        f"[{label_in}]scale={size}:{size},format=yuva420p,"
        f"geq=lum='p(X,Y)':cb='p(X,Y)':cr='p(X,Y)':a='255*lte(hypot(X-(W-1)/2,Y-(H-1)/2),W/2-1)'[{label_out}]"
    )


def chapter_metadata(chapters: list[dict]) -> str:
    lines = [";FFMETADATA1"]
    for i, c in enumerate(chapters):
        s = c.get("slide") or {}
        title = s.get("title") or " ".join(s.get("lines") or []).replace("**", "") or f"Hoofdstuk {i + 1}"
        lines += ["[CHAPTER]", "TIMEBASE=1/1000", f"START={int(c['start'] * 1000)}", f"END={int(c['end'] * 1000)}", f"title={title}"]
    return "\n".join(lines) + "\n"


def stills(jobs: list[dict], env: list[str]) -> None:
    proc = subprocess.run(
        [shutil.which("uv") or "uv", "run", "--quiet", "--no-project", *env, "python", str(Path(__file__).with_name("export_stills_run.py"))],
        input=json.dumps(jobs),
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        hint = "\nDraait `just dev-demo`? En staat Playwright's browser er (`uvx playwright install chromium`)?"
        raise SystemExit(f"stills maken mislukt:\n{proc.stderr[-1500:]}{hint}")


def export_track(name: str, track: dict, url: str, media_dir: Path, captions_dir: Path, out_dir: Path, env: list[str]) -> Path:
    work = out_dir / f".{name}"
    work.mkdir(parents=True, exist_ok=True)
    chapters = track["chapters"]
    query = "main" if name == "main" else name.removeprefix("faq-")
    jobs = []
    for i, c in enumerate(chapters):
        at = c["start"] + min(0.5, (c["end"] - c["start"]) / 2)
        jobs.append({"url": f"{url.rstrip('/')}/rondleiding?still={at:.3f}&track={query}", "out": str(work / f"still-{i:03d}.png")})
    stills(jobs, env)
    layouts = [json.loads(Path(j["out"] + ".json").read_text()) for j in jobs]

    listing = work / "stills.txt"
    entries = ["ffconcat version 1.0"]
    for i, c in enumerate(chapters):
        entries += [f"file '{work / f'still-{i:03d}.png'}'", f"duration {c['end'] - c['start']:.3f}"]
    entries.append(f"file '{work / f'still-{len(chapters) - 1:03d}.png'}'")
    listing.write_text("\n".join(entries) + "\n")
    (work / "chapters.txt").write_text(chapter_metadata(chapters))

    video = track["video"]
    inputs = ["-f", "concat", "-safe", "0", "-i", listing, "-i", media_dir / video["src"]]
    graph = [f"[0:v]fps={FPS},scale={W}:{H},setsar=1,format=yuv420p[bg]"]
    last = "bg"

    # The app video, in the stage, on every slide that shows the rail.
    rail = [(c, lay) for c, lay in zip(chapters, layouts) if not lay["full"] and lay["stage"]]
    if rail:
        stage = rail[0][1]["stage"]
        sw, sh, ox, oy = fit(stage["width"], stage["height"], video["width"], video["height"])
        graph += [
            f"[1:v]scale={sw}:{sh},setsar=1[app]",
            f"[bg][app]overlay=x={int(stage['x']) + ox}:y={int(stage['y']) + oy}:enable='{windows([c for c, _ in rail])}'[app_on]",
        ]
        last = "app_on"

    # The presenter: one placement per distinct spot.
    if track.get("cam"):
        inputs += ["-i", media_dir / track["cam"]["src"]]
        spots: dict[tuple[int, int, int], list[dict]] = {}
        for c, lay in zip(chapters, layouts):
            r = CAM_FULL if lay["full"] else lay["cam"]
            if r:
                spots.setdefault((int(r["x"]), int(r["y"]), int(r["width"]) // 2 * 2), []).append(c)
        if spots:
            graph.append("[2:v]split=" + str(len(spots)) + "".join(f"[cam{i}]" for i in range(len(spots))))
            for i, ((x, y, size), chs) in enumerate(spots.items()):
                graph.append(circle(f"cam{i}", size, f"round{i}"))
                graph.append(f"[{last}][round{i}]overlay=x={x}:y={y}:enable='{windows(chs)}'[with{i}]")
                last = f"with{i}"

    graph.append(f"[{last}]format=yuv420p[out]")
    script = work / "graph.txt"
    script.write_text(";\n".join(graph))
    n_inputs = inputs.count("-i")
    vtt = captions_dir / track["captions"]["nl"] if (track.get("captions") or {}).get("nl") else None
    extra_in = ["-i", vtt] if vtt and vtt.exists() else []
    meta_index = n_inputs + (1 if extra_in else 0)
    out = out_dir / f"{name}.mp4"
    args = [*inputs, *extra_in, "-i", work / "chapters.txt", "-filter_complex_script", script, "-map", "[out]", "-map", "1:a"]
    if extra_in:
        args += ["-map", f"{n_inputs}:s", "-c:s", "mov_text", "-metadata:s:s:0", "language=dut"]
    args += [
        "-map_metadata",
        str(meta_index),
        "-map_chapters",
        str(meta_index),
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-crf",
        "20",
        "-c:a",
        "copy",
        "-t",
        f"{track['duration']:.3f}",
        "-movflags",
        "+faststart",
        out,
    ]
    ffmpeg(*args)
    return out


def export_all(timeline: dict, url: str, media_dir: Path, captions_dir: Path, out_dir: Path, env: list[str]) -> None:
    tracks = [("main", timeline["main"])] + [(f"faq-{f['id']}", f) for f in timeline.get("faq") or []]
    for name, track in tracks:
        out = export_track(name, track, url, media_dir, captions_dir, out_dir, env)
        print(f"{name}: {out}", file=sys.stderr)
