"""ffmpeg and ffprobe, wrapped for what the walkthrough needs."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

FPS = 30


def require(tool: str) -> str:
    path = shutil.which(tool)
    if not path:
        raise SystemExit(f"{tool} ontbreekt; installeer het (brew install ffmpeg)")
    return path


def run(args: list[str], quiet: bool = True) -> subprocess.CompletedProcess:
    cmd = [str(a) for a in args]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        tail = "\n".join(proc.stderr.strip().splitlines()[-15:])
        raise SystemExit(f"mislukt: {' '.join(cmd[:6])} ...\n{tail}")
    if not quiet and proc.stderr:
        print(proc.stderr)
    return proc


def ffmpeg(*args) -> subprocess.CompletedProcess:
    return run([require("ffmpeg"), "-hide_banner", "-loglevel", "error", "-y", *args])


def probe(path: Path) -> dict:
    out = run([require("ffprobe"), "-v", "error", "-show_streams", "-show_format", "-of", "json", path]).stdout
    return json.loads(out)


def duration(path: Path) -> float:
    info = probe(path)
    d = info.get("format", {}).get("duration")
    if d not in (None, "N/A"):
        return float(d)
    return max(float(s.get("duration") or 0) for s in info.get("streams", []))


def video_size(path: Path) -> tuple[int, int]:
    for s in probe(path).get("streams", []):
        if s.get("codec_type") == "video":
            return int(s["width"]), int(s["height"])
    raise SystemExit(f"geen videospoor in {path}")


def has_audio(path: Path) -> bool:
    return any(s.get("codec_type") == "audio" for s in probe(path).get("streams", []))


def even(n: float) -> int:
    n = int(round(n))
    return n - (n % 2)


def snap(t: float, fps: int = FPS) -> float:
    """To the frame grid, so video and audio pieces have the same length."""
    return round(round(t * fps) / fps, 4)


def frames(seconds: float, fps: int = FPS) -> int:
    return max(1, int(round(seconds * fps)))


def silences(path: Path, noise_db: float = -38.0, min_len: float = 0.6) -> list[tuple[float, float]]:
    """Stretches of silence in an audio file, measured, as (start, end)."""
    err = run([require("ffmpeg"), "-hide_banner", "-i", path, "-af", f"silencedetect=noise={noise_db}dB:d={min_len}", "-f", "null", "-"]).stderr
    out, start = [], None
    for line in err.splitlines():
        if "silence_start:" in line:
            start = float(line.split("silence_start:")[1].split()[0])
        elif "silence_end:" in line and start is not None:
            out.append((round(start, 3), round(float(line.split("silence_end:")[1].split()[0]), 3)))
            start = None
    return out


def loudnorm(src: Path, dst: Path, target: float = -16.0, peak: float = -1.5, lra: float = 11.0) -> dict:
    """Two-pass EBU R128 loudness normalisation to `dst` (48 kHz mono WAV)."""
    first = run(
        [require("ffmpeg"), "-hide_banner", "-y", "-i", src, "-af", f"loudnorm=I={target}:TP={peak}:LRA={lra}:print_format=json", "-f", "null", "-"]
    ).stderr
    blob = first[first.rfind("{") : first.rfind("}") + 1]
    m = json.loads(blob)
    ffmpeg(
        "-i",
        src,
        "-af",
        (
            f"loudnorm=I={target}:TP={peak}:LRA={lra}:measured_I={m['input_i']}:measured_TP={m['input_tp']}"
            f":measured_LRA={m['input_lra']}:measured_thresh={m['input_thresh']}:offset={m['target_offset']}:linear=true"
        ),
        "-ar",
        "48000",
        "-ac",
        "1",
        dst,
    )
    return m
