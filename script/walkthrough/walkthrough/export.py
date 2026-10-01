"""A shareable MP4 per track: the recording of the window, with the presenter.

The take is recorded with the deck as a rail next to the demo, so the video of
the window already looks like the walkthrough. This adds the presenter in a
circle, in the rail's bottom corner, the captions as a subtitle track and the
slides as MP4 chapters, so a viewer can turn the one on and jump through the
other. The same file is what a phone plays, where the live demo does not fit.
"""

from __future__ import annotations

import sys
from pathlib import Path

from .media import FPS, ffmpeg, video_size

# The bubble, as a share of the picture: in the rail (36% wide), bottom left.
CAM_SHARE = 0.11
MARGIN_SHARE = 0.02


def chapter_metadata(chapters: list[dict]) -> str:
    lines = [";FFMETADATA1"]
    for i, c in enumerate(chapters):
        s = c.get("slide") or {}
        title = s.get("title") or " ".join(s.get("lines") or []).replace("**", "") or f"Hoofdstuk {i + 1}"
        lines += ["[CHAPTER]", "TIMEBASE=1/1000", f"START={int(c['start'] * 1000)}", f"END={int(c['end'] * 1000)}", f"title={title}"]
    return "\n".join(lines) + "\n"


def export_track(name: str, track: dict, media_dir: Path, captions_dir: Path, out_dir: Path) -> Path:
    work = out_dir / f".{name}"
    work.mkdir(parents=True, exist_ok=True)
    (work / "chapters.txt").write_text(chapter_metadata(track["chapters"]))
    video = media_dir / track["video"]["src"]
    w, h = video_size(video)
    inputs = ["-i", video]
    graph = []
    last = "0:v"
    if track.get("cam"):
        size = int(h * CAM_SHARE * 2) // 2 * 2 or 2
        margin = int(h * MARGIN_SHARE)
        inputs += ["-i", media_dir / track["cam"]["src"]]
        graph += [
            f"[1:v]scale={size}:{size},format=yuva420p,"
            f"geq=lum='p(X,Y)':cb='p(X,Y)':cr='p(X,Y)':a='255*lte(hypot(X-(W-1)/2,Y-(H-1)/2),W/2-1)'[round]",
            f"[0:v][round]overlay=x={margin}:y={h - size - margin}:shortest=1,fps={FPS},format=yuv420p[out]",
        ]
        last = "[out]"
    n = inputs.count("-i")
    vtt_name = (track.get("captions") or {}).get("nl")
    vtt = captions_dir / vtt_name if vtt_name else None
    extra = ["-i", vtt] if vtt and vtt.exists() else []
    meta = n + (1 if extra else 0)
    out = out_dir / f"{name}.mp4"
    args = [*inputs, *extra, "-i", work / "chapters.txt"]
    if graph:
        script = work / "graph.txt"
        script.write_text(";\n".join(graph))
        args += ["-filter_complex_script", script, "-map", last]
    else:
        args += ["-map", "0:v"]
    args += ["-map", "0:a"]
    if extra:
        args += ["-map", f"{n}:s", "-c:s", "mov_text", "-metadata:s:s:0", "language=dut"]
    args += ["-map_metadata", str(meta), "-map_chapters", str(meta)]
    args += ["-c:v", "libx264", "-preset", "medium", "-crf", "20"] if graph else ["-c:v", "copy"]
    args += ["-c:a", "copy", "-movflags", "+faststart", out]
    ffmpeg(*args)
    return out


def export_all(timeline: dict, media_dir: Path, captions_dir: Path, out_dir: Path) -> None:
    tracks = [("main", timeline["main"])] + [(f"faq-{f['id']}", f) for f in timeline.get("faq") or []]
    for name, track in tracks:
        print(f"{name}: {export_track(name, track, media_dir, captions_dir, out_dir)}", file=sys.stderr)
