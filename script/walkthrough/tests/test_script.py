import pytest

from walkthrough.script import ScriptError, beats, even_alignment, layout, parse_line, words_of


def ev(t, type_, **kw):
    return {"t": int(t * 1000), "type": type_, **kw}


def test_markers_come_out_of_the_text_and_keep_their_place():
    line = parse_line("Ik open [1] de lijst en zoek [2] de huurtoeslag.")
    assert line.text == "Ik open de lijst en zoek de huurtoeslag."
    assert [(b, line.text[o:o + 3]) for b, o in line.marks] == [(1, "de "), (2, "de ")]


def test_actions_split_into_beats_at_pauses_and_keep_their_pace():
    events = [ev(0, "slide", index=5), ev(0.1, "route", path="/wetten"), ev(7, "click"), ev(10.3, "click"), ev(10.4, "input", value="h"), ev(10.6, "input", value="hu"), ev(15.9, "click"), ev(16.0, "route", path="/x")]
    runs = beats(events)
    assert [[e["type"] for e in r] for r in runs] == [["click"], ["click", "input", "input"], ["click", "route"]]
    assert [e["t"] for e in runs[1]] == [0, 0.1, 0.3]


def test_beats_land_on_their_word_and_the_voice_waits_for_a_long_one():
    lines = [parse_line("Ik open [1] de lijst."), parse_line("Klaar.")]
    audio = [{"duration": 2.0, "alignment": even_alignment(lines[0].text, 2.0)}, {"duration": 0.5, "alignment": even_alignment("Klaar.", 0.5)}]
    # The beat takes 4 seconds: the second line waits for it.
    runs = [[{"t": 0, "type": "click"}, {"t": 4.0, "type": "input", "value": "x"}]]
    out = layout(lines, audio, runs)
    click = out["events"][0]
    assert click["t"] == pytest.approx(2.0 * 8 / len(lines[0].text), abs=0.01)
    assert out["clips"][1]["at"] == pytest.approx(click["t"] + 4.0 + 0.6, abs=0.01)
    assert out["duration"] == pytest.approx(out["clips"][1]["at"] + 0.5, abs=0.01)


def test_every_beat_must_be_in_the_script_once():
    lines = [parse_line("Een [1] twee.")]
    audio = [{"duration": 1, "alignment": even_alignment(lines[0].text, 1)}]
    with pytest.raises(ScriptError, match="not in the script"):
        layout(lines, audio, [[{"t": 0, "type": "click"}], [{"t": 0, "type": "click"}]])
    with pytest.raises(ScriptError, match="does not exist"):
        layout([parse_line("Een [3].")], audio, [[{"t": 0, "type": "click"}]])


def test_say_as_changes_what_is_spoken_but_not_the_text():
    from walkthrough.voice import remap, speakable

    spoken, where = speakable("De Awb geldt.", {"Awb": "A-W-B"})
    assert spoken == "De A-W-B geldt."
    # "geldt" starts at 7 in the text and at 9 in what is spoken.
    assert where[7] == 9
    # A word that merely contains the key is left alone.
    assert speakable("Awbachtig", {"Awb": "A-W-B"})[0] == "Awbachtig"
    alignment = {"character_start_times_seconds": [i / 10 for i in range(len(spoken))]}
    assert remap(alignment, where)["character_start_times_seconds"][7] == 0.9


def test_words_for_captions_follow_the_alignment():
    text = "Hallo daar."
    words = words_of(text, even_alignment(text, 1.1))
    assert [w["word"] for w in words] == ["Hallo", "daar."]
    assert words[1]["start"] == pytest.approx(0.6, abs=0.01)
