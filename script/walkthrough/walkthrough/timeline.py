"""The timeline arithmetic of a walkthrough: which parts of which takes are
kept, where everything lands after the cuts, and the chapters and captions
that follow from that.

Pure functions on plain data, so the decisions that are easy to get subtly
wrong (an event that lands in a cut, a cut through someone typing, a caption
that straddles a jump) are tested without ffmpeg. All times are seconds.
"""

from __future__ import annotations

from dataclasses import dataclass, field


class CutError(ValueError):
    """A cut list that cannot be applied as written."""


@dataclass(frozen=True)
class Piece:
    """A kept stretch of one take, and where it starts in the output."""

    take: str
    start: float  # in the take
    end: float  # in the take
    out: float  # in the output

    @property
    def length(self) -> float:
        return self.end - self.start


@dataclass
class Track:
    """One playable track (the main line or an answer), before rendering."""

    pieces: list[Piece] = field(default_factory=list)

    @property
    def duration(self) -> float:
        return sum(p.length for p in self.pieces)

    def to_output(self, take: str, t: float) -> float | None:
        """Where take time `t` lands in the output; None when it was cut."""
        for p in self.pieces:
            if p.take == take and p.start <= t < p.end:
                return round(p.out + (t - p.start), 3)
        # The very end of the last piece of a take still exists.
        for p in self.pieces:
            if p.take == take and t == p.end:
                return round(p.out + p.length, 3)
        return None


def subtract(start: float, end: float, cuts: list[tuple[float, float]]) -> list[tuple[float, float]]:
    """[start, end) minus the cuts, as ordered intervals. Tiny slivers go."""
    keep = [(start, end)]
    for c0, c1 in sorted(cuts):
        if c1 <= c0:
            raise CutError(f"cut {c0}-{c1} ends before it starts")
        nxt = []
        for k0, k1 in keep:
            if c1 <= k0 or c0 >= k1:
                nxt.append((k0, k1))
                continue
            if c0 > k0:
                nxt.append((k0, c0))
            if c1 < k1:
                nxt.append((c1, k1))
        keep = nxt
    return [(round(a, 3), round(b, 3)) for a, b in keep if b - a >= 0.05]


def protected_spans(events: list[dict], gap: float = 1.5, before: float = 0.2, after: float = 0.6) -> list[tuple[float, float]]:
    """Stretches where the presenter was typing, from the recorder's key events.

    A cut inside one would make characters appear in a jump while the voice
    goes on, which reads as an edit and is exactly what the recording is meant
    to avoid.
    """
    keys = sorted(e["t"] / 1000 for e in events if e.get("type") in ("key", "input"))
    spans: list[tuple[float, float]] = []
    for k in keys:
        if spans and k - spans[-1][1] <= gap:
            spans[-1] = (spans[-1][0], k)
        else:
            spans.append((k, k))
    return [(round(a - before, 3), round(b + after, 3)) for a, b in spans]


def check_cuts(cuts: list[tuple[float, float]], protected: list[tuple[float, float]]) -> None:
    for c0, c1 in cuts:
        for p0, p1 in protected:
            if c0 < p1 and c1 > p0:
                raise CutError(f"cut {c0:.2f}-{c1:.2f} runs through typing at {p0:.2f}-{p1:.2f}")


def build_track(segments: list[dict], cuts_by_take: dict[str, list[tuple[float, float]]], durations: dict[str, float], fps: int | None = None) -> Track:
    """Lay the kept pieces of the segments end to end.

    With `fps`, every piece starts and ends on a frame. The video of a piece is
    a whole number of frames; if the audio were cut at the exact second
    instead, a few milliseconds per cut would add up to a voice that runs
    visibly ahead of the picture by the end.
    """
    track = Track()
    out = 0.0
    for seg in segments:
        take = seg["take"]
        if take not in durations:
            raise CutError(f"unknown take {take}")
        a = float(seg.get("from") or 0.0)
        b = float(seg.get("to") or durations[take])
        b = min(b, durations[take])
        if b <= a:
            raise CutError(f"segment {take} {a}-{b} is empty")
        kept = subtract(a, b, cuts_by_take.get(take, []))
        if fps:
            kept = [(round(round(k0 * fps) / fps, 4), round(round(k1 * fps) / fps, 4)) for k0, k1 in kept]
            kept = [(k0, k1) for k0, k1 in kept if k1 > k0]
        for k0, k1 in kept:
            track.pieces.append(Piece(take, k0, k1, round(out, 3)))
            out += k1 - k0
    return track


def slide_at(events: list[dict], t: float) -> dict | None:
    """The last slide event at or before take time `t`."""
    current = None
    for e in events:
        if e.get("type") != "slide":
            continue
        if e["t"] / 1000 <= t + 1e-6:
            current = e
        else:
            break
    return current


def chapters(track: Track, events_by_take: dict[str, list[dict]], overrides: dict | None = None) -> list[dict]:
    """Chapter boundaries in the output, one per slide, merged across cuts.

    A chapter starts where a slide event lands, and also where a piece starts
    on a different slide than the one before it (a retake can splice in at any
    slide). `overrides` replaces fields of a slide by its index: the text on
    a slide can be corrected after recording without a retake.
    """
    overrides = overrides or {}
    marks: list[tuple[float, dict]] = []
    for p in track.pieces:
        events = events_by_take.get(p.take, [])
        first = slide_at(events, p.start)
        if first is not None:
            marks.append((p.out, first))
        for e in events:
            if e.get("type") != "slide":
                continue
            t = e["t"] / 1000
            if p.start < t < p.end:
                marks.append((round(p.out + t - p.start, 3), e))
    marks.sort(key=lambda m: m[0])

    result: list[dict] = []
    for at, e in marks:
        index = e.get("index", 0)
        if result and result[-1]["slideIndex"] == index:
            continue
        slide = dict(e.get("slide") or {})
        slide.update(overrides.get(index, overrides.get(str(index), {})) or {})
        if result and at - result[-1]["start"] < 0.05:
            result.pop()
        result.append({"start": round(at, 3), "slideIndex": index, "slide": slide, "profile": e.get("profile"), "state": e.get("state")})
    if not result:
        result.append({"start": 0.0, "slideIndex": 0, "slide": {}, "profile": None, "state": None})
    result[0]["start"] = 0.0
    total = round(track.duration, 3)
    for i, c in enumerate(result):
        c["end"] = result[i + 1]["start"] if i + 1 < len(result) else total
    return result


def remap_points(track: Track, events_by_take: dict[str, list[dict]], kind: str) -> list[dict]:
    """Events of one kind (clicks, flubs) at their output time; cut ones drop."""
    out = []
    for take, events in events_by_take.items():
        for e in events:
            if e.get("type") != kind:
                continue
            t = track.to_output(take, e["t"] / 1000)
            if t is not None:
                out.append({**{k: v for k, v in e.items() if k not in ("t", "type")}, "t": t})
    return sorted(out, key=lambda e: e["t"])


def remap_words(track: Track, words_by_take: dict[str, list[dict]]) -> list[dict]:
    """Words at their output time.

    A word belongs to the piece its start falls in, with its end clipped to
    that piece. Not "the whole word must fit": the aligner stretches a word's
    end into the pause after it, and a cut in that pause would otherwise take
    the last word of every sentence out of the captions.
    """
    out = []
    for p in track.pieces:
        for w in words_by_take.get(p.take, []):
            s, e = w.get("start"), w.get("end")
            if s is None or e is None:
                continue
            if p.start - 1e-6 <= s < p.end:
                e = min(e, p.end)
                out.append({"word": w["word"], "start": round(p.out + s - p.start, 3), "end": round(p.out + e - p.start, 3)})
    return sorted(out, key=lambda w: w["start"])


def captions(words: list[dict], max_chars: int = 84, max_len: float = 6.0, pause: float = 0.7, breaks: list[float] | None = None) -> list[dict]:
    """Group words into caption cues.

    A cue ends at the end of a sentence, after a pause, when it gets too long
    to read, or at a break (a chapter boundary: a caption must not carry
    over a slide change, where the picture jumps).
    """
    breaks = sorted(breaks or [])
    cues: list[dict] = []
    cur: list[dict] = []

    def flush():
        if cur:
            cues.append({"start": cur[0]["start"], "end": cur[-1]["end"], "text": " ".join(w["word"] for w in cur).strip()})
            cur.clear()

    for w in words:
        if cur:
            text_len = len(" ".join(x["word"] for x in cur)) + 1 + len(w["word"])
            crosses = any(cur[-1]["end"] <= b <= w["start"] or cur[0]["start"] < b <= w["start"] for b in breaks)
            if text_len > max_chars or w["end"] - cur[0]["start"] > max_len or w["start"] - cur[-1]["end"] > pause or crosses:
                flush()
        cur.append(w)
        if w["word"].rstrip().endswith((".", "?", "!")):
            flush()
    flush()
    # Hold each cue until the next one starts, if that is close: a caption that
    # blinks off between two sentences is harder to read than one that stays.
    # Not across a break, though: there the picture jumps and the caption
    # belongs to what came before.
    for i, c in enumerate(cues[:-1]):
        nxt = cues[i + 1]["start"]
        gap = nxt - c["end"]
        if 0 < gap < 0.8 and not any(c["end"] < b <= nxt for b in breaks):
            c["end"] = nxt
    return [{**c, "text": wrap(c["text"])} for c in cues]


def wrap(text: str, width: int = 42) -> str:
    """At most two lines, broken near the middle at a space."""
    if len(text) <= width:
        return text
    mid = len(text) // 2
    left, right = text.rfind(" ", 0, mid + 1), text.find(" ", mid)
    candidates = [i for i in (left, right) if i > 0]
    if not candidates:
        return text
    cut = min(candidates, key=lambda i: abs(i - mid))
    return text[:cut] + "\n" + text[cut + 1 :]


def vtt_time(t: float) -> str:
    ms = int(round(t * 1000))
    h, ms = divmod(ms, 3_600_000)
    m, ms = divmod(ms, 60_000)
    s, ms = divmod(ms, 1000)
    return f"{h:02d}:{m:02d}:{s:02d}.{ms:03d}"


def to_vtt(cues: list[dict]) -> str:
    lines = ["WEBVTT", ""]
    for i, c in enumerate(cues, 1):
        lines += [str(i), f"{vtt_time(c['start'])} --> {vtt_time(c['end'])}", c["text"], ""]
    return "\n".join(lines)


def suggest_cuts(
    words: list[dict],
    events: list[dict],
    silences: list[tuple[float, float]] | None = None,
    long_pause: float = 1.2,
    keep_pause: float = 0.8,
) -> list[dict]:
    """Proposed cuts for one take, for a person (or Claude) to review.

    - a long silence is shortened to `keep_pause`, unless the presenter acted
      in it (a click, a slide change): then the silence is the viewer watching
      what happens, and it stays;
    - a flub mark cuts back to the start of the sentence it interrupted, up to
      the first word after the mark (the restart).

    `silences` come from the audio (ffmpeg silencedetect). Without them the
    gaps between words are used, which is rougher: the aligner tends to
    stretch a word's end into the pause after it.

    Never applied on its own: the output is a draft for walkthrough.yaml.
    """
    acts = [e["t"] / 1000 for e in events if e.get("type") in ("click", "slide", "route", "key")]
    protected = protected_spans(events)
    if silences is None:
        silences = [(a["end"], b["start"]) for a, b in zip(words, words[1:])]
    out: list[dict] = []
    for s0, s1 in silences:
        gap = s1 - s0
        if gap <= long_pause:
            continue
        if any(s0 - 0.2 <= c <= s1 for c in acts):
            continue
        c0 = round(s0 + keep_pause / 2, 2)
        c1 = round(s1 - keep_pause / 2, 2)
        if any(c0 < p1 and c1 > p0 for p0, p1 in protected):
            continue
        out.append({"from": c0, "to": c1, "reason": f"stilte van {gap:.1f}s"})

    for flub in (e["t"] / 1000 for e in events if e.get("type") == "flub"):
        before = [w for w in words if w["end"] <= flub]
        after = [w for w in words if w["start"] >= flub]
        if not before or not after:
            continue
        start = before[-1]
        # Back to the previous sentence end, or a pause, whichever is nearer.
        for prev, w in zip(reversed(before[:-1]), reversed(before)):
            start = w
            if prev["word"].rstrip().endswith((".", "?", "!")) or w["start"] - prev["end"] > 0.6:
                break
        out.append({"from": round(start["start"] - 0.05, 2), "to": round(after[0]["start"] - 0.05, 2), "reason": "verspreking (Shift+X)"})
    return sorted(out, key=lambda c: c["from"])


# Actions the player does again in the live demo. Everything else in the log
# (flubs, the end mark, the deck closing) is for post-processing only.
REPLAYED = ("slide", "route", "click", "input", "change", "key", "scroll")
KEEP_FIELDS = ("index", "path", "target", "fx", "fy", "value", "checked", "key", "top", "left")


def remap_actions(track: Track, events_by_take: dict[str, list[dict]]) -> list[dict]:
    """The replayable actions on the output timeline.

    An action in a cut is not dropped but moved to the cut: the words and the
    wait go, the click stays. Dropping it would leave the demo in another
    state than the presenter's from that moment on (a panel never opened, a
    law never picked). The same holds for what happened in a take before the
    stretch that is used: it all happens at the start of that stretch.

    Where a piece from another take begins, a `restore` puts the demo in the
    state that take started from (its first slide's snapshot), since a retake
    is recorded from its own starting point.
    """
    out: list[dict] = []
    for i, p in enumerate(track.pieces):
        events = events_by_take.get(p.take, [])
        same_take_before = i > 0 and track.pieces[i - 1].take == p.take
        if not same_take_before:
            slides = [e for e in events if e.get("type") == "slide"]
            # The slide the take starts on: the last one logged in its first
            # half second (a take's log can open with the slide the deck
            # showed a moment before it started).
            first = next((e for e in reversed(slides) if e["t"] <= slides[0]["t"] + 500), None) if slides else None
            if first is not None:
                out.append({"t": round(p.out, 3), "type": "restore", "state": first.get("state"), "slideIndex": first.get("index", 0)})
        # This piece collects what happened after the piece before it in the
        # same take (or from the take's start) up to its own end.
        lower = track.pieces[i - 1].end if same_take_before else float("-inf")
        for e in events:
            if e.get("type") not in REPLAYED:
                continue
            t = e["t"] / 1000
            if not lower < t <= p.end:
                continue
            at = p.out + max(0.0, t - p.start)
            out.append({"t": round(at, 3), "type": e["type"], **{k: e[k] for k in KEEP_FIELDS if k in e}})
    # Stable: actions moved to the same moment keep their recorded order.
    out.sort(key=lambda e: e["t"])
    return out
