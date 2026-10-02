"""The walkthrough pipeline, from a raw take to what the demo plays.

    walkthrough prepare <take>   ingest + clean + transcribe + suggest, in one go
    walkthrough ingest <take>    remux, take the voice out, read the event log
    walkthrough clean <take>     denoise and tidy the voice (no loudness yet)
    walkthrough transcribe <take>  words with timestamps (WhisperX, Dutch)
    walkthrough suggest <take>   draft cuts: long silences and flub marks
    walkthrough transcript <take|main>  the text per slide, to check the slides
    walkthrough build            walkthrough.yaml -> media, timeline.json, captions
    walkthrough export           a shareable MP4 per track
    walkthrough status           what is recorded and processed so far

Raw takes live in `.walkthrough/takes/<take>/` (not in git). What decides the
result is `corpus/demo/walkthrough/walkthrough.yaml`, in git: which takes,
which cuts, which slide texts, which questions. `build` is a pure function of
that file and the takes, so a correction is an edit there and a rebuild.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import yaml

from . import media
from .media import FPS, ffmpeg
from .timeline import (
    CutError,
    build_track,
    captions,
    chapters,
    check_cuts,
    protected_spans,
    remap_actions,
    remap_words,
    suggest_cuts,
    to_vtt,
)

ROOT = Path(__file__).resolve().parents[3]
WORK = ROOT / ".walkthrough"
TAKES = WORK / "takes"
CORPUS = ROOT / "corpus" / "demo" / "walkthrough"
CONFIG = CORPUS / "walkthrough.yaml"
PUBLIC = ROOT / "frontend-demo" / "public" / "walkthrough"
HERE = Path(__file__).resolve().parent

DENOISE = ["--python", "3.11", "--with", "deepfilternet==0.5.6", "--with", "torch==2.0.1", "--with", "torchaudio==2.0.2", "--with", "numpy<2"]
WHISPERX = ["--python", "3.12", "--with", "whisperx"]
OPENCV = ["--with", "opencv-python-headless<5"]

MAX_WIDTH = 2560
CAM_SIZE = 480


def say(msg: str) -> None:
    print(msg, file=sys.stderr)


def take_dir(take: str) -> Path:
    d = TAKES / take
    if not d.is_dir():
        raise SystemExit(f"opname {take} niet gevonden in {TAKES}")
    return d


def latest_take() -> str:
    takes = sorted(p.name for p in TAKES.glob("*") if p.is_dir())
    if not takes:
        raise SystemExit(f"nog geen opnames in {TAKES}; neem op met `just demo` en ?record")
    return takes[-1]


def read_json(path: Path, default=None):
    return json.loads(path.read_text()) if path.exists() else default


def uv_run(env: list[str], *args) -> subprocess.CompletedProcess:
    uv = shutil.which("uv") or "uv"
    cmd = [uv, "run", "--quiet", "--no-project", *env, *[str(a) for a in args]]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"mislukt: {' '.join(cmd[:8])} ...\n{proc.stderr[-2000:]}")
    return proc


# ---- per take ----------------------------------------------------------------


def ingest(take: str) -> dict:
    d = take_dir(take)
    events = read_json(d / "events.json", [])
    meta = read_json(d / "meta.json", {})
    if not (d / "app.webm").exists():
        raise SystemExit(f"{take}: app.webm ontbreekt")
    # MediaRecorder writes WebM without a duration or a seek index. A remux
    # into Matroska gives both, without touching the streams.
    ffmpeg("-i", d / "app.webm", "-c", "copy", d / "app.mkv")
    if (d / "cam.webm").exists():
        ffmpeg("-i", d / "cam.webm", "-c", "copy", d / "cam.mkv")
    # The voice, as 48 kHz mono, padded at the start if the audio began a
    # moment after the video, so take time is the same for both.
    ffmpeg("-i", d / "app.mkv", "-map", "0:a:0", "-af", "aresample=async=1:first_pts=0", "-ac", "1", "-ar", "48000", d / "mic.wav")
    # Tab capture has a variable frame rate: on a still screen Chrome sends
    # hardly any frames. Cutting such a file at a second would start the piece
    # at the next frame that happens to exist, seconds late on a still slide.
    # So the take becomes constant frame rate once, here, every held frame
    # repeated, and pieces are cut from that on the frame grid.
    w0, h0 = media.video_size(d / "app.mkv")
    scale = min(1.0, MAX_WIDTH / w0)
    w, h = media.even(w0 * scale), media.even(h0 * scale)
    cfr = ["-c:v", "libx264", "-preset", "veryfast", "-g", FPS]
    ffmpeg("-i", d / "app.mkv", "-an", "-vf", f"fps={FPS}:start_time=0,scale={w}:{h},setsar=1,format=yuv420p", *cfr, "-crf", "14", d / "app.cfr.mp4")
    if (d / "cam.mkv").exists():
        ffmpeg("-i", d / "cam.mkv", "-an", "-vf", f"fps={FPS}:start_time=0,format=yuv420p", *cfr, "-crf", "18", d / "cam.cfr.mp4")
    video_len = media.duration(d / "app.cfr.mp4")
    end = next((e["t"] / 1000 for e in reversed(events) if e.get("type") == "end"), video_len)
    info = {
        "take": take,
        "duration": round(min(video_len, end), 3),
        "width": w,
        "height": h,
        "camOffset": (meta.get("camOffsetMs") or 0) / 1000 if (d / "cam.mkv").exists() else None,
        "slides": sorted({e.get("index") for e in events if e.get("type") == "slide"}),
    }
    (d / "take.json").write_text(json.dumps(info, indent=1))
    say(f"{take}: {info['duration']:.1f}s, {w}x{h}, dia's {info['slides']}")
    return info


def clean(take: str, denoise: bool = True) -> None:
    d = take_dir(take)
    if not (d / "mic.wav").exists():
        ingest(take)
    src = d / "mic.wav"
    if denoise:
        say(f"{take}: ruis verwijderen (DeepFilterNet)")
        with tempfile.TemporaryDirectory() as tmp:
            uv_run(DENOISE, "deepFilter", src, "-o", tmp)
            out = next(Path(tmp).glob("*.wav"))
            shutil.copy(out, d / "denoised.wav")
        src = d / "denoised.wav"
    # Below 80 Hz is rumble, not voice. The de-esser takes the edge off sharp
    # s-sounds that a laptop or USB microphone exaggerates; the compressor
    # evens out a voice that leans in and out. Loudness comes later, over the
    # whole cut, so every chapter ends up equally loud.
    chain = "highpass=f=80,deesser=i=0.4,acompressor=threshold=-21dB:ratio=3:attack=5:release=120:makeup=2"
    if not denoise:
        chain = "highpass=f=80,afftdn=nf=-25," + chain.split(",", 1)[1]
    ffmpeg("-i", src, "-af", chain, "-ac", "1", "-ar", "48000", d / "voice.wav")
    say(f"{take}: voice.wav klaar")


def glossary() -> str:
    cfg = load_config(required=False)
    words = cfg.get("glossary") or []
    # A sentence, not a bare list: Whisper copies the prompt's style, and a
    # list without punctuation gave a transcript without full stops, which
    # leaves the captions nothing to break on.
    if not words:
        return "Dit is een rondleiding door de demo van RegelRecht."
    return f"Dit is een rondleiding door de demo van RegelRecht. Begrippen die erin voorkomen: {', '.join(words)}."


def transcribe(take: str, model: str = "large-v3") -> None:
    d = take_dir(take)
    if not (d / "voice.wav").exists():
        clean(take)
    say(f"{take}: transcriberen met WhisperX ({model}); dit duurt even")
    uv_run(WHISPERX, HERE / "whisperx_run.py", d / "voice.wav", d / "words.json", "--model", model, "--prompt", glossary())
    say(f"{take}: words.json klaar")


def suggest(take: str) -> list[dict]:
    d = take_dir(take)
    words = read_json(d / "words.json", {}).get("words", [])
    events = read_json(d / "events.json", [])
    quiet = media.silences(d / "voice.wav") if (d / "voice.wav").exists() else None
    cuts = suggest_cuts(words, events, silences=quiet)
    text = yaml.safe_dump({"takes": {take: {"cuts": cuts}}}, allow_unicode=True, sort_keys=False)
    (d / "cuts.suggested.yaml").write_text(text)
    say(f"{take}: {len(cuts)} voorgestelde knippen in {d / 'cuts.suggested.yaml'}")
    return cuts


def cam_crop(take: str) -> dict | None:
    d = take_dir(take)
    if not (d / "cam.mkv").exists():
        return None
    cached = d / "cam_crop.json"
    if cached.exists():
        return read_json(cached)
    say(f"{take}: gezicht zoeken voor de webcam-uitsnede")
    crop = json.loads(uv_run(OPENCV, "python", HERE / "camcrop_run.py", d / "cam.mkv").stdout)
    cached.write_text(json.dumps(crop))
    return crop


def prepare(take: str, model: str, denoise: bool) -> None:
    ingest(take)
    clean(take, denoise=denoise)
    transcribe(take, model=model)
    suggest(take)
    cam_crop(take)


# ---- the config ----------------------------------------------------------------


def load_config(required: bool = True) -> dict:
    if not CONFIG.exists():
        if required:
            raise SystemExit(f"{CONFIG.relative_to(ROOT)} ontbreekt")
        return {}
    return yaml.safe_load(CONFIG.read_text()) or {}


def cuts_for(cfg: dict, take: str) -> list[tuple[float, float]]:
    raw = ((cfg.get("takes") or {}).get(take) or {}).get("cuts") or []
    return [(float(c["from"]), float(c["to"])) for c in raw]


def take_info(take: str) -> dict:
    info = read_json(take_dir(take) / "take.json")
    if not info or not (take_dir(take) / "app.cfr.mp4").exists():
        return ingest(take)
    return info


# ---- build -----------------------------------------------------------------------


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def hashed(path: Path, stem: str, dest: Path) -> dict:
    digest = sha(path)
    name = f"{stem}-{digest[:12]}{path.suffix}"
    shutil.move(path, dest / name)
    return {"src": name, "sha256": digest, "bytes": (dest / name).stat().st_size}


def concat_files(parts: list[Path], out: Path, workdir: Path) -> None:
    listing = workdir / f"{out.stem}.txt"
    listing.write_text("".join(f"file '{p}'\n" for p in parts))
    ffmpeg("-f", "concat", "-safe", "0", "-i", listing, "-c", "copy", out)


def deck_of(take: str, overrides: dict) -> list[dict]:
    """The deck as it stood when `take` was recorded, with the slide
    corrections from walkthrough.yaml: the replay shows these slides."""
    meta = read_json(take_dir(take) / "meta.json", {})
    deck = [dict(s) for s in meta.get("slides") or []]
    for i, fix in overrides.items():
        if 0 <= i < len(deck):
            deck[i].update(fix or {})
    return deck


def render_recorded(name: str, segments: list[dict], cfg: dict, overrides: dict, workdir: Path) -> dict:
    """A stretch of recorded takes: voice, video and webcam, cut as written."""
    takes = sorted({s["take"] for s in segments})
    infos = {t: take_info(t) for t in takes}
    durations = {t: infos[t]["duration"] for t in takes}
    events = {t: read_json(take_dir(t) / "events.json", []) for t in takes}
    words = {t: read_json(take_dir(t) / "words.json", {}).get("words", []) for t in takes}
    cuts = {t: cuts_for(cfg, t) for t in takes}
    for t in takes:
        check_cuts(cuts[t], protected_spans(events[t]))
        if not (take_dir(t) / "voice.wav").exists():
            clean(t)

    track = build_track(segments, cuts, durations, fps=FPS)
    first = infos[takes[0]]
    width, height = first["width"], first["height"]
    has_cam = all((take_dir(t) / "cam.cfr.mp4").exists() for t in takes)
    say(f"{name}: {len(track.pieces)} opgenomen stukken, {track.duration:.1f}s{', met webcam' if has_cam else ''}")

    # Video and webcam piece by piece, each a whole number of frames, then
    # joined without re-encoding. One filter graph over all pieces would have
    # ffmpeg buffer raw frames of one take while it waits on another.
    vlist, clist = [], []
    for i, p in enumerate(track.pieces):
        n = media.frames(p.length)
        # A take of another window size is letterboxed into the first one's.
        vf = f"scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1,format=yuv420p"
        out = workdir / f"{name}-v{i:04d}.mp4"
        ffmpeg("-ss", p.start, "-i", take_dir(p.take) / "app.cfr.mp4", "-an", "-vf", vf, "-frames:v", n, "-c:v", "libx264", "-preset", "medium", "-crf", "20", "-g", FPS * 4, out)
        vlist.append(out)
        if has_cam:
            crop = cam_crop(p.take)
            offset = infos[p.take].get("camOffset") or 0
            cvf = f"crop={crop['size']}:{crop['size']}:{crop['x']}:{crop['y']},scale={CAM_SIZE}:{CAM_SIZE},setsar=1,format=yuv420p"
            cout = workdir / f"{name}-c{i:04d}.mp4"
            ffmpeg("-ss", max(0.0, p.start - offset), "-i", take_dir(p.take) / "cam.cfr.mp4", "-an", "-vf", cvf, "-frames:v", n, "-c:v", "libx264", "-preset", "medium", "-crf", "26", "-g", FPS * 4, cout)
            clist.append(cout)

    # The voice: the same pieces, with a 15 ms fade on each side of every cut
    # so a cut does not click. Loudness comes later, over the whole track.
    inputs, graph, labels = [], [], []
    take_index = {}
    for t in takes:
        take_index[t] = len(inputs) // 2
        inputs += ["-i", take_dir(t) / "voice.wav"]
    for i, p in enumerate(track.pieces):
        fade = min(0.015, p.length / 4)
        graph.append(
            f"[{take_index[p.take]}:a]atrim=start={p.start}:end={p.end},asetpts=PTS-STARTPTS,"
            f"afade=t=in:d={fade},afade=t=out:st={p.length - fade:.4f}:d={fade}[a{i}]"
        )
        labels.append(f"[a{i}]")
    graph.append(f"{''.join(labels)}concat=n={len(labels)}:v=0:a=1[out]")
    raw = workdir / f"{name}-voice-raw.wav"
    script = workdir / f"{name}-audio.filter"
    script.write_text(";\n".join(graph))
    ffmpeg(*inputs, "-filter_complex_script", script, "-map", "[out]", "-ac", "1", "-ar", "48000", "-t", f"{track.duration:.4f}", raw)

    meta = read_json(take_dir(track.pieces[0].take) / "meta.json", {}) if track.pieces else {}
    return {
        "duration": round(track.duration, 3),
        "voice": raw,
        "video": vlist,
        "cam": clist if has_cam else None,
        "size": (width, height),
        "chapters": chapters(track, events, overrides),
        "events": remap_actions(track, events),
        "words": remap_words(track, words),
        "slides": deck_of(takes[0], overrides),
        "recordedAt": meta.get("startedAt"),
        "viewport": meta.get("viewport"),
        "pieces": [{"take": p.take, "from": p.start, "to": p.end, "at": p.out} for p in track.pieces],
    }


def render_scripted(name: str, seg: dict, cfg: dict, overrides: dict, workdir: Path) -> dict:
    """A chapter spoken from its script, in the generated voice, with the
    actions of its silent take laid on the words (script.py)."""
    from .script import beats, layout, parse_line
    from .voice import speak

    path = CORPUS / "script" / f"{seg['script']}.yaml"
    if not path.exists():
        raise SystemExit(f"script ontbreekt: {path.relative_to(ROOT)}")
    doc = yaml.safe_load(path.read_text()) or {}
    lines = [parse_line(str(raw)) for raw in doc.get("lines") or []]
    if not lines:
        raise SystemExit(f"{path.name}: geen regels")
    voice_cfg = cfg.get("voice") or {"provider": "say"}
    texts = [line.text for line in lines]
    audio = [speak(text, voice_cfg, WORK, texts[i - 1] if i else "", texts[i + 1] if i + 1 < len(texts) else "") for i, text in enumerate(texts)]
    take = doc.get("take")
    events = read_json(take_dir(take) / "events.json", []) if take else []
    runs = beats(events)
    try:
        plan = layout(lines, audio, runs)
    except ValueError as e:
        raise SystemExit(f"{path.name}: {e}")
    say(f"{name}: script {seg['script']}, {len(lines)} regels, {len(runs)} handelingen, {plan['duration']:.1f}s ({voice_cfg.get('provider', 'say')})")

    # The lines at their places, with silence between them where a line waits.
    pieces, at = [], 0.0
    for clip in plan["clips"]:
        gap = clip["at"] - at
        if gap > 0.001:
            silence = workdir / f"{name}-gap{len(pieces):04d}.wav"
            ffmpeg("-f", "lavfi", "-i", "anullsrc=r=48000:cl=mono", "-t", f"{gap:.4f}", silence)
            pieces.append(silence)
        pieces.append(audio[clip["line"]]["wav"])
        at = clip["at"] + audio[clip["line"]]["duration"]
    tail = plan["duration"] - at
    if tail > 0.001:
        silence = workdir / f"{name}-tail.wav"
        ffmpeg("-f", "lavfi", "-i", "anullsrc=r=48000:cl=mono", "-t", f"{tail:.4f}", silence)
        pieces.append(silence)
    raw = workdir / f"{name}-voice-raw.wav"
    listing = workdir / f"{name}-voice.txt"
    listing.write_text("".join(f"file '{p}'\n" for p in pieces))
    ffmpeg("-f", "concat", "-safe", "0", "-i", listing, "-ac", "1", "-ar", "48000", raw)

    slide_index = int(doc.get("slide", 0))
    first = next((e for e in events if e.get("type") == "slide"), None)
    state = first.get("state") if first else None
    deck = deck_of(take, overrides) if take else []
    slide = deck[slide_index] if 0 <= slide_index < len(deck) else {}
    meta = read_json(take_dir(take) / "meta.json", {}) if take else {}
    acts = [{"t": 0.0, "type": "restore", "state": state, "slideIndex": slide_index}] if state else [{"t": 0.0, "type": "slide", "index": slide_index}]
    return {
        "duration": plan["duration"],
        "voice": raw,
        "video": None,
        "cam": None,
        "size": None,
        "chapters": [{"start": 0.0, "end": plan["duration"], "slideIndex": slide_index, "slide": slide, "profile": first.get("profile") if first else None, "state": state}],
        "events": acts + plan["events"],
        "words": plan["words"],
        "slides": deck,
        "recordedAt": meta.get("startedAt"),
        "viewport": meta.get("viewport"),
        "pieces": [{"script": seg["script"], "at": 0.0}],
        "generated": True,
    }


def render_track(name: str, segments: list[dict], cfg: dict, overrides: dict, dest: Path, vtt_dest: Path, workdir: Path) -> dict:
    """A playable track from its segments: runs of recorded takes and
    scripted chapters, laid end to end on one clock."""
    parts, run = [], []
    for seg in segments:
        if "script" in seg:
            if run:
                parts.append(render_recorded(f"{name}-p{len(parts)}", run, cfg, overrides, workdir))
                run = []
            parts.append(render_scripted(f"{name}-p{len(parts)}", seg, cfg, overrides, workdir))
        else:
            run.append(seg)
    if run:
        parts.append(render_recorded(f"{name}-p{len(parts)}", run, cfg, overrides, workdir))

    offset = 0.0
    seams = []  # where one part hands over to the next
    chs, acts, words, pieces = [], [], [], []
    for part in parts:
        for c in part["chapters"]:
            chs.append({**c, "start": round(c["start"] + offset, 3), "end": round(c["end"] + offset, 3)})
        acts += [{**e, "t": round(e["t"] + offset, 3)} for e in part["events"]]
        words += [{**w, "start": round(w["start"] + offset, 3), "end": round(w["end"] + offset, 3)} for w in part["words"]]
        pieces += [{**p, "at": round(p["at"] + offset, 3)} for p in part["pieces"]]
        offset += part["duration"]
        seams.append(round(offset, 3))
    duration = round(offset, 3)
    # Neighbouring chapters on the same slide (a recorded opening that ends on
    # the slide a script continues on) are one chapter.
    merged = []
    for c in chs:
        if merged and merged[-1]["slideIndex"] == c["slideIndex"]:
            merged[-1]["end"] = c["end"]
        else:
            merged.append(c)

    # One voice: the parts end to end, then loudness over all of it, so the
    # recorded opening and the generated chapters sound equally loud.
    raw = workdir / f"{name}-voice-raw.wav"
    listing = workdir / f"{name}-voices.txt"
    listing.write_text("".join(f"file '{p['voice']}'\n" for p in parts))
    ffmpeg("-f", "concat", "-safe", "0", "-i", listing, "-ac", "1", "-ar", "48000", raw)
    voice = workdir / f"{name}-voice.wav"
    media.loudnorm(raw, voice)
    audio_out = workdir / f"{name}-voice.m4a"
    ffmpeg("-i", voice, "-c:a", "aac", "-b:a", "96k", "-ac", "1", "-movflags", "+faststart", audio_out)

    entry: dict = {"duration": duration, "audio": hashed(audio_out, f"{name}-voice", dest)}

    # The window's video only when every part was recorded: a generated
    # chapter has no picture of its own. A phone then gets the MP4 made from
    # the replay itself (`walkthrough export`).
    if all(p["video"] for p in parts):
        video_only = workdir / f"{name}-video.mp4"
        concat_files([v for p in parts for v in p["video"]], video_only, workdir)
        main = workdir / f"{name}.mp4"
        ffmpeg("-i", video_only, "-i", voice, "-map", "0:v", "-map", "1:a", "-c:v", "copy", "-c:a", "aac", "-b:a", "128k", "-shortest", "-movflags", "+faststart", main)
        w, h = parts[0]["size"]
        entry["video"] = {**hashed(main, name, dest), "width": w, "height": h}
    else:
        entry["video"] = None

    # The presenter's bubble: the webcam of the recorded parts at the start
    # (the opening). Where the generated voice takes over, the bubble goes.
    lead = []
    for p in parts:
        if not p["cam"]:
            break
        lead.append(p)
    if lead:
        cam_only = workdir / f"{name}-cam.mp4"
        concat_files([c for p in lead for c in p["cam"]], cam_only, workdir)
        cam_final = workdir / f"{name}-cam-final.mp4"
        ffmpeg("-i", cam_only, "-c", "copy", "-movflags", "+faststart", cam_final)
        entry["cam"] = {**hashed(cam_final, f"{name}-cam", dest), "until": round(sum(p["duration"] for p in lead), 3)}
    else:
        entry["cam"] = None

    first = next((p for p in parts if p["slides"]), parts[0])
    entry["slides"] = first["slides"]
    entry["recordedAt"] = next((p["recordedAt"] for p in parts if p["recordedAt"]), None)
    entry["viewport"] = first["viewport"]
    entry["generatedVoice"] = any(p.get("generated") for p in parts)
    entry["chapters"] = merged
    entry["events"] = sorted(acts, key=lambda e: e["t"])
    # A caption ends at a chapter and at a seam between parts: a sentence cut
    # off at the end of the recorded opening is not continued by the voice.
    cues = captions(sorted(words, key=lambda w: w["start"]), breaks=[c["start"] for c in merged[1:]] + seams[:-1])
    vtt = to_vtt(cues)
    vtt_name = f"{name}-{hashlib.sha256(vtt.encode()).hexdigest()[:12]}.nl.vtt"
    (vtt_dest / vtt_name).write_text(vtt)
    entry["captions"] = {"nl": vtt_name}
    entry["pieces"] = pieces
    return entry


def build(release: str | None = None) -> None:
    cfg = load_config()
    main_cfg = cfg.get("main") or {}
    if not main_cfg.get("segments"):
        raise SystemExit("walkthrough.yaml: main.segments is leeg")
    overrides = {int(k): v for k, v in (cfg.get("slides") or {}).items()}
    WORK.mkdir(parents=True, exist_ok=True)
    # Everything is rendered aside and only swapped in once all of it worked:
    # a failed or interrupted build leaves the previous timeline and the files
    # it names in place.
    with tempfile.TemporaryDirectory(dir=WORK) as tmp:
        work = Path(tmp)
        media_out, vtt_out = work / "media", work / "captions"
        media_out.mkdir()
        vtt_out.mkdir()
        try:
            main = render_track("main", main_cfg["segments"], cfg, overrides, media_out, vtt_out, work)
            faqs = []
            for f in cfg.get("faq") or []:
                segs = f.get("segments") or ([{"script": f["script"]}] if f.get("script") else [{"take": f["take"], "from": f.get("from"), "to": f.get("to")}])
                entry = render_track(f"faq-{f['id']}", segs, cfg, {}, media_out, vtt_out, work)
                offer = f.get("offer") or {}
                at = None
                if "take" in offer:
                    at = next((round(p["at"] + offer["at"] - p["from"], 3) for p in main["pieces"] if p.get("take") == offer["take"] and p["from"] <= offer["at"] < p["to"]), None)
                    if at is None:
                        say(f"let op: het aanbiedmoment van '{f['id']}' valt in een knip of buiten de rondleiding")
                elif "chapter" in offer:
                    idx = int(offer["chapter"])
                    if not 0 <= idx < len(main["chapters"]):
                        raise SystemExit(f"walkthrough.yaml: vraag '{f['id']}' wijst naar hoofdstuk {idx}, er zijn er {len(main['chapters'])}")
                    at = main["chapters"][idx]["start"] + float(offer.get("after", 0))
                faqs.append({"id": f["id"], "question": f["question"], "offer": at, **entry})
        except CutError as e:
            raise SystemExit(f"walkthrough.yaml: {e}")
        # A rebuild replaces everything: stale media under an old hash would be
        # served and fetched for nothing.
        PUBLIC.mkdir(parents=True, exist_ok=True)
        for old in [*PUBLIC.glob("*"), *CORPUS.glob("*.vtt")]:
            if old.is_file():
                old.unlink()
        for f in [*media_out.iterdir()]:
            shutil.move(f, PUBLIC / f.name)
        for f in [*vtt_out.iterdir()]:
            shutil.move(f, CORPUS / f.name)
    timeline = {
        "version": 1,
        "release": release or cfg.get("release"),
        "presenter": cfg.get("presenter") or {},
        "main": main,
        "faq": faqs,
    }
    (CORPUS / "timeline.json").write_text(json.dumps(timeline, ensure_ascii=False, indent=1) + "\n")
    total = sum(sum((t.get(k) or {}).get("bytes") or 0 for k in ("audio", "video", "cam")) for t in [main, *faqs])
    say(f"klaar: {main['duration']:.0f}s rondleiding, {len(faqs)} vragen, {total / 1e6:.0f} MB media in {PUBLIC.relative_to(ROOT)}")


# ---- reading back ------------------------------------------------------------------


def transcript(which: str) -> None:
    """The spoken text per slide: what Claude reads to bring the slides in line."""
    if which == "main" or which.startswith("faq-"):
        timeline = read_json(CORPUS / "timeline.json")
        if not timeline:
            raise SystemExit("nog geen timeline.json; draai eerst `walkthrough build`")
        track = timeline["main"] if which == "main" else next(f for f in timeline["faq"] if f"faq-{f['id']}" == which)
        from .timeline import Piece, Track

        tr = Track([Piece(p["take"], p["from"], p["to"], p["at"]) for p in track["pieces"] if "take" in p])
        words = remap_words(tr, {t: read_json(take_dir(t) / "words.json", {}).get("words", []) for t in {p.take for p in tr.pieces}})
        chs = track["chapters"]
    else:
        words = read_json(take_dir(which) / "words.json", {}).get("words", [])
        events = read_json(take_dir(which) / "events.json", [])
        from .timeline import Track, Piece

        tr = Track([Piece(which, 0, 1e9, 0)])
        chs = chapters(tr, {which: events})
    for i, c in enumerate(chs):
        end = chs[i + 1]["start"] if i + 1 < len(chs) else 1e9
        text = " ".join(w["word"] for w in words if c["start"] <= w["start"] < end)
        s = c.get("slide") or {}
        title = s.get("title") or " / ".join(s.get("lines") or []) or f"dia {c['slideIndex']}"
        print(f"\n## {c['slideIndex']}. {title}  [{c['start']:.1f}s]\n")
        print(text or "(stil)")


def status() -> None:
    cfg = load_config(required=False)
    used = {s["take"] for s in (cfg.get("main") or {}).get("segments") or []} | {f.get("take") for f in cfg.get("faq") or []}
    for d in sorted(TAKES.glob("*")):
        if not d.is_dir():
            continue
        steps = [n for n, f in (("ingest", "take.json"), ("clean", "voice.wav"), ("transcribe", "words.json"), ("suggest", "cuts.suggested.yaml")) if (d / f).exists()]
        info = read_json(d / "take.json", {})
        mark = "*" if d.name in used else " "
        print(f"{mark} {d.name}  {info.get('duration', 0):7.1f}s  dia's {info.get('slides', '?')}  [{', '.join(steps) or 'ruw'}]")
    tl = read_json(CORPUS / "timeline.json")
    if tl:
        print(f"\ntimeline.json: {tl['main']['duration']:.0f}s, {len(tl['main']['chapters'])} hoofdstukken, {len(tl['faq'])} vragen")


# ---- export ---------------------------------------------------------------------------


def export() -> None:
    from .export import export_all

    timeline = read_json(CORPUS / "timeline.json")
    if not timeline:
        raise SystemExit("nog geen timeline.json; draai eerst `walkthrough build`")
    out = WORK / "export"
    out.mkdir(parents=True, exist_ok=True)
    export_all(timeline, PUBLIC, CORPUS, out)


def main(argv: list[str] | None = None) -> None:
    ap = argparse.ArgumentParser(prog="walkthrough", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("prepare", "ingest", "clean", "transcribe", "suggest"):
        p = sub.add_parser(name)
        p.add_argument("take", nargs="?", help="map in .walkthrough/takes (standaard: de laatste)")
        if name in ("prepare", "transcribe"):
            p.add_argument("--model", default="large-v3", help="Whisper-model, bijv. large-v3-turbo voor sneller")
        if name in ("prepare", "clean"):
            p.add_argument("--no-denoise", action="store_true", help="ffmpeg-ruisfilter in plaats van DeepFilterNet")
    p = sub.add_parser("transcript")
    p.add_argument("which", nargs="?", default="main", help="main, faq-<id> of een opname")
    p = sub.add_parser("build")
    p.add_argument("--release", help="tag van de GitHub-release met de media")
    sub.add_parser("export")
    sub.add_parser("status")
    args = ap.parse_args(argv)

    take = getattr(args, "take", None) or (latest_take() if args.cmd in ("prepare", "ingest", "clean", "transcribe", "suggest") else None)
    if args.cmd == "prepare":
        prepare(take, args.model, not args.no_denoise)
    elif args.cmd == "ingest":
        ingest(take)
    elif args.cmd == "clean":
        clean(take, denoise=not args.no_denoise)
    elif args.cmd == "transcribe":
        transcribe(take, args.model)
    elif args.cmd == "suggest":
        print(yaml.safe_dump(suggest(take), allow_unicode=True, sort_keys=False))
    elif args.cmd == "transcript":
        transcript(args.which)
    elif args.cmd == "build":
        build(args.release)
    elif args.cmd == "export":
        export()
    elif args.cmd == "status":
        status()


if __name__ == "__main__":
    main()
