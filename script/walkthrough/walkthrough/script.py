"""Chapters spoken from a script instead of a recorded voice.

A scripted chapter is text with markers, plus a silent take that holds the
presenter's clicks and typing for it. The voice is generated (ElevenLabs, or
the Mac's own voice while trying things out); the actions are laid on the
generated speech at their markers.

    slide: 5
    take: 2026-10-03T10-00-00
    lines:
      - Dit is de wet op de zorgtoeslag, zoals een computer hem leest.
      - Ik open [1] de lijst met wetten en zoek [2] de huurtoeslag.

`[n]` is the moment the n-th beat of the take starts: the word after the
marker. A beat is a run of actions in the take with no pause longer than
`BEAT_GAP` between them (a click, then the typing that follows it), and it
keeps its own pace, so typed text appears as fast as it was typed. When a
beat takes longer than the words around it, the voice waits.

Pure functions on plain data; generation and rendering are in cli.py.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

BEAT_GAP = 1.2  # seconds
ACTIONS = ("click", "input", "change", "key", "scroll", "route", "viewport")
MARK = re.compile(r"\s*\[(\d+)\]\s*")


class ScriptError(ValueError):
    """A script that cannot be laid out as written."""


@dataclass
class Line:
    text: str  # what is spoken, markers removed
    marks: list[tuple[int, int]] = field(default_factory=list)  # (beat, char offset in text)


def parse_line(raw: str) -> Line:
    """Split `[n]` markers out of a line, keeping where in the text each was."""
    text, marks, pos = "", [], 0
    for m in MARK.finditer(raw):
        text += raw[pos : m.start()]
        if text and not text.endswith(" ") and m.end() < len(raw):
            text += " "
        marks.append((int(m.group(1)), len(text)))
        pos = m.end()
    text += raw[pos:]
    return Line(text=text.strip(), marks=marks)


def beats(events: list[dict], gap: float = BEAT_GAP) -> list[list[dict]]:
    """The take's actions in runs; times in seconds, relative to the run's start."""
    acts = [e for e in events if e.get("type") in ACTIONS]
    runs: list[list[dict]] = []
    last = None
    for e in acts:
        t = e["t"] / 1000
        if last is None or t - last > gap:
            runs.append([])
        runs[-1].append({**e, "t": t})
        last = t
    out = []
    for run in runs:
        # A run of nothing but navigation is the deck opening its tab, not
        # something the presenter did: it has no word to land on.
        if all(e["type"] == "route" for e in run):
            continue
        t0 = run[0]["t"]
        out.append([{**e, "t": round(e["t"] - t0, 3)} for e in run])
    return out


def beat_starts(events: list[dict], gap: float = BEAT_GAP) -> list[float]:
    """When each beat of `beats(events)` starts, in seconds of the take."""
    acts = [e for e in events if e.get("type") in ACTIONS]
    runs: list[list[dict]] = []
    last = None
    for e in acts:
        t = e["t"] / 1000
        if last is None or t - last > gap:
            runs.append([])
        runs[-1].append(e)
        last = t
    return [run[0]["t"] / 1000 for run in runs if not all(e["type"] == "route" for e in run)]


def draft_lines(words: list[dict], starts: list[float], lead: float = 0.3) -> list[str]:
    """A script from what was said while recording: one line per sentence,
    with each beat's marker before the word the presenter was saying when the
    action began (or the next word, when it began in a pause).

    This is the draft `walkthrough script` writes: the presenter's own words,
    to be read again by the generated voice, with the clicks where they were.
    """
    if not words:
        return []
    # Where each marker goes: before word index i.
    before: dict[int, list[int]] = {}
    for n, s in enumerate(starts, 1):
        i = next((k for k, w in enumerate(words) if w["start"] >= s - lead), None)
        if i is None:
            before.setdefault(len(words), []).append(n)
        else:
            before.setdefault(i, []).append(n)
    lines, current = [], []
    for k, w in enumerate(words):
        current += [f"[{n}]" for n in before.get(k, [])]
        current.append(w["word"])
        if w["word"].rstrip().endswith((".", "?", "!")):
            lines.append(" ".join(current))
            current = []
    current += [f"[{n}]" for n in before.get(len(words), [])]
    if current:
        if lines and all(c.startswith("[") for c in current):
            lines[-1] += " " + " ".join(current)
        else:
            lines.append(" ".join(current))
    return lines


def char_time(alignment: dict, offset: int) -> float:
    """Seconds at character `offset` in an ElevenLabs-style alignment
    (`characters`, `character_start_times_seconds`)."""
    starts = alignment["character_start_times_seconds"]
    if not starts:
        return 0.0
    return float(starts[min(max(offset, 0), len(starts) - 1)])


def layout(lines: list[Line], audio: list[dict], runs: list[list[dict]], pause: float = 0.35) -> dict:
    """Place the spoken lines and the beats on one timeline.

    `audio[i]` is line i's generated speech: `{duration, alignment}`. Returns
    `{clips: [{line, at}], events: [...], words: [...], duration}`: where each
    line's audio starts, the actions at their times, and the words for the
    captions. A beat that is still running when the next line would start
    pushes that line back, so the voice never talks over an action that is
    meant to be watched.
    """
    if len(audio) != len(lines):
        raise ScriptError("every line needs its generated audio")
    used = set()
    clips, events, words = [], [], []
    t = 0.0
    busy_until = 0.0
    for i, (line, sound) in enumerate(zip(lines, audio)):
        t = max(t, busy_until)
        clips.append({"line": i, "at": round(t, 3)})
        for beat, offset in line.marks:
            if not 1 <= beat <= len(runs):
                raise ScriptError(f"line {i + 1}: beat [{beat}] does not exist; the take has {len(runs)}")
            if beat in used:
                raise ScriptError(f"beat [{beat}] is used twice")
            used.add(beat)
            start = t + char_time(sound["alignment"], offset)
            run = runs[beat - 1]
            for e in run:
                events.append({**e, "t": round(start + e["t"], 3)})
            busy_until = max(busy_until, start + (run[-1]["t"] if run else 0) + 0.6)
        words += [{**w, "start": round(t + w["start"], 3), "end": round(t + w["end"], 3)} for w in words_of(line.text, sound["alignment"])]
        t += sound["duration"] + pause
    missing = sorted(set(range(1, len(runs) + 1)) - used)
    if missing:
        raise ScriptError(f"beats {missing} of the take are not in the script")
    duration = round(max(t - pause, busy_until), 3)
    return {"clips": clips, "events": sorted(events, key=lambda e: e["t"]), "words": words, "duration": duration}


def words_of(text: str, alignment: dict) -> list[dict]:
    """Word timings from a character alignment, for the captions."""
    starts = alignment["character_start_times_seconds"]
    ends = alignment.get("character_end_times_seconds") or starts
    out = []
    for m in re.finditer(r"\S+", text):
        a, b = m.start(), m.end() - 1
        if a >= len(starts):
            break
        out.append({"word": m.group(0), "start": float(starts[a]), "end": float(ends[min(b, len(ends) - 1)])})
    return out


def even_alignment(text: str, duration: float) -> dict:
    """An alignment that spreads the characters evenly over `duration`: for a
    voice that does not report its own timing (the Mac's, for trying out)."""
    n = max(len(text), 1)
    step = duration / n
    starts = [round(i * step, 4) for i in range(len(text))]
    return {"characters": list(text), "character_start_times_seconds": starts, "character_end_times_seconds": [round(s + step, 4) for s in starts]}
