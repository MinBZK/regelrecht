import pytest

from walkthrough.timeline import (
    CutError,
    build_track,
    captions,
    chapters,
    check_cuts,
    protected_spans,
    remap_points,
    remap_words,
    subtract,
    suggest_cuts,
    to_vtt,
    wrap,
)


def ev(t, type_, **kw):
    return {"t": int(t * 1000), "type": type_, **kw}


def test_subtract_removes_cuts_and_slivers():
    assert subtract(0, 10, [(2, 3), (5, 5.02), (9.98, 12)]) == [(0, 2), (3, 5), (5.02, 9.98)]
    assert subtract(0, 10, [(-1, 11)]) == []
    with pytest.raises(CutError):
        subtract(0, 10, [(5, 4)])


def test_track_maps_take_time_to_output_time():
    track = build_track([{"take": "a"}], {"a": [(2, 4)]}, {"a": 10})
    assert track.duration == 8
    assert track.to_output("a", 1) == 1
    assert track.to_output("a", 3) is None  # cut
    assert track.to_output("a", 5) == 3
    assert track.to_output("a", 10) == 8


def test_segments_from_several_takes_play_end_to_end():
    track = build_track(
        [{"take": "a", "to": 5}, {"take": "b", "from": 2}],
        {},
        {"a": 10, "b": 6},
    )
    assert [(p.take, p.start, p.end, p.out) for p in track.pieces] == [("a", 0, 5, 0), ("b", 2, 6, 5)]
    assert track.to_output("b", 3) == 6


def test_unknown_take_or_empty_segment_is_an_error():
    with pytest.raises(CutError):
        build_track([{"take": "x"}], {}, {"a": 1})
    with pytest.raises(CutError):
        build_track([{"take": "a", "from": 5, "to": 5}], {}, {"a": 10})


def test_a_cut_through_typing_is_refused():
    events = [ev(10, "key"), ev(10.5, "key"), ev(11.2, "key"), ev(30, "key")]
    spans = protected_spans(events)
    assert spans == [(9.8, 11.8), (29.8, 30.6)]
    check_cuts([(2, 3)], spans)
    with pytest.raises(CutError):
        check_cuts([(11, 12)], spans)


def test_chapters_follow_slides_across_cuts_and_takes():
    events = {
        "a": [ev(0, "slide", index=0, slide={"kind": "title"}), ev(4, "slide", index=1, slide={"title": "Wet", "route": "/wetten"}, profile="merijn")],
        "b": [ev(0, "slide", index=2, slide={"title": "Portaal", "route": "/portaal"})],
    }
    track = build_track([{"take": "a"}, {"take": "b"}], {"a": [(1, 2)]}, {"a": 6, "b": 5})
    result = chapters(track, events, overrides={1: {"title": "Wetten lezen"}})
    assert [(c["start"], c["end"], c["slideIndex"]) for c in result] == [(0, 3, 0), (3, 5, 1), (5, 10, 2)]
    assert result[1]["slide"] == {"title": "Wetten lezen", "route": "/wetten"}
    assert result[1]["profile"] == "merijn"


def test_a_slide_cut_away_entirely_leaves_no_empty_chapter():
    events = {"a": [ev(0, "slide", index=0), ev(2, "slide", index=1), ev(3, "slide", index=2)]}
    track = build_track([{"take": "a"}], {"a": [(1.9, 3.1)]}, {"a": 6})
    assert [c["slideIndex"] for c in chapters(track, events)] == [0, 2]


def test_clicks_in_a_cut_disappear_and_the_rest_shift():
    events = {"a": [ev(1, "click", x=0.1, y=0.2), ev(3, "click", x=0.5, y=0.5), ev(6, "click", x=0.9, y=0.9)]}
    track = build_track([{"take": "a"}], {"a": [(2, 4)]}, {"a": 10})
    assert remap_points(track, events, "click") == [{"x": 0.1, "y": 0.2, "t": 1}, {"x": 0.9, "y": 0.9, "t": 4}]


def test_a_word_follows_its_start_and_is_clipped_at_the_cut():
    words = {
        "a": [
            {"word": "Wetten", "start": 0.5, "end": 0.9},
            # Stretched by the aligner into the pause that is cut.
            {"word": "worden", "start": 1.6, "end": 2.6},
            {"word": "weg", "start": 2.5, "end": 2.8},
            {"word": "vertaald.", "start": 4.1, "end": 4.6},
        ]
    }
    track = build_track([{"take": "a"}], {"a": [(2, 4)]}, {"a": 10})
    got = [(w["word"], w["start"], w["end"]) for w in remap_words(track, words)]
    assert got == [("Wetten", 0.5, 0.9), ("worden", 1.6, 2.0), ("vertaald.", 2.1, 2.6)]


def test_captions_break_at_sentences_pauses_and_chapters():
    w = lambda text, s, e: {"word": text, "start": s, "end": e}
    words = [w("Wetten", 0, 0.4), w("worden", 0.45, 0.8), w("vertaald.", 0.85, 1.3), w("Elke", 1.4, 1.6), w("keer", 1.65, 1.9), w("opnieuw", 3.0, 3.4)]
    cues = captions(words, breaks=[])
    assert [c["text"] for c in cues] == ["Wetten worden vertaald.", "Elke keer", "opnieuw"]
    # Held until the next cue when the gap is short.
    assert cues[0]["end"] == 1.4
    cues = captions(words[:3], breaks=[0.42])
    assert [c["text"] for c in cues] == ["Wetten", "worden vertaald."]
    # Not held over the chapter break.
    assert cues[0]["end"] == 0.4


def test_pieces_start_and_end_on_frames():
    track = build_track([{"take": "a"}], {"a": [(1.01, 2.02)]}, {"a": 4}, fps=30)
    for p in track.pieces:
        assert abs(p.start * 30 - round(p.start * 30)) < 0.01
        assert abs(p.end * 30 - round(p.end * 30)) < 0.01
    assert [(p.start, p.end) for p in track.pieces] == [(0, 1.0), (2.0333, 4.0)]


def test_long_caption_wraps_on_two_lines():
    text = "Elke organisatie vertaalt de wet opnieuw naar haar eigen software"
    assert wrap(text).count("\n") == 1
    assert wrap("kort") == "kort"


def test_vtt_output():
    vtt = to_vtt([{"start": 1, "end": 3725.5, "text": "Hallo"}])
    assert "00:00:01.000 --> 01:02:05.500" in vtt
    assert vtt.startswith("WEBVTT")


def test_suggested_cuts_shorten_silences_but_not_when_the_presenter_acts_in_them():
    w = lambda text, s, e: {"word": text, "start": s, "end": e}
    words = [w("Hier", 0, 0.3), w("ziet", 3.0, 3.3), w("u", 10.0, 10.2)]
    events = [ev(3.6, "click")]
    cuts = suggest_cuts(words, events)
    assert cuts == [{"from": 0.7, "to": 2.6, "reason": "stilte van 2.7s"}]


def test_suggested_cuts_prefer_measured_silences():
    w = lambda text, s, e: {"word": text, "start": s, "end": e}
    # The aligner stretched "Hier" over the pause; the audio knows better.
    words = [w("Hier", 0, 2.9), w("ziet", 3.0, 3.3)]
    assert suggest_cuts(words, [], silences=[(0.4, 2.95)]) == [{"from": 0.8, "to": 2.55, "reason": "stilte van 2.6s"}]


def test_suggested_cut_for_a_flub_goes_back_to_the_sentence_start():
    w = lambda text, s, e: {"word": text, "start": s, "end": e}
    words = [w("Klaar.", 0, 0.4), w("Dit", 1.0, 1.2), w("is", 1.25, 1.4), w("de", 1.45, 1.5), w("Dit", 4.0, 4.2), w("is", 4.25, 4.4)]
    cuts = suggest_cuts(words, [ev(2.0, "flub")], long_pause=99)
    assert cuts == [{"from": 0.95, "to": 3.95, "reason": "verspreking (Shift+X)"}]
