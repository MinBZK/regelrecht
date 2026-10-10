"""Transcribe one take with WhisperX, with word timestamps.

Run in its own environment (cli.py starts it with `uv run --with whisperx`),
because WhisperX pulls in a torch that the denoiser cannot share.

    python whisperx_run.py voice.wav words.json [--model large-v3] [--prompt "..."]

Writes `{"segments": [...], "words": [{word, start, end, score}]}`. Words that
the aligner could not place (numbers, sometimes) get the times of their
neighbours, so every word has a start and an end.
"""

import argparse
import json
import sys


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("audio")
    ap.add_argument("out")
    ap.add_argument("--model", default="large-v3")
    ap.add_argument("--language", default="nl")
    ap.add_argument("--prompt", default="")
    args = ap.parse_args()

    import whisperx

    device = "cpu"
    asr_options = {"initial_prompt": args.prompt} if args.prompt else {}
    model = whisperx.load_model(args.model, device, compute_type="int8", language=args.language, asr_options=asr_options)
    audio = whisperx.load_audio(args.audio)
    result = model.transcribe(audio, batch_size=8, language=args.language)
    align_model, metadata = whisperx.load_align_model(language_code=args.language, device=device)
    aligned = whisperx.align(result["segments"], align_model, metadata, audio, device, return_char_alignments=False)

    words = []
    for seg in aligned["segments"]:
        for w in seg.get("words", []):
            words.append({"word": w.get("word", "").strip(), "start": w.get("start"), "end": w.get("end"), "score": w.get("score")})
    # Fill gaps from the neighbours.
    for i, w in enumerate(words):
        if w["start"] is None:
            prev = next((x["end"] for x in reversed(words[:i]) if x["end"] is not None), 0.0)
            w["start"] = prev
        if w["end"] is None:
            nxt = next((x["start"] for x in words[i + 1 :] if x["start"] is not None), w["start"] + 0.3)
            w["end"] = max(w["start"], nxt)
    segments = [{"start": s["start"], "end": s["end"], "text": s["text"].strip()} for s in aligned["segments"]]
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump({"segments": segments, "words": [w for w in words if w["word"]]}, f, ensure_ascii=False, indent=1)
    print(f"{len(words)} woorden", file=sys.stderr)


if __name__ == "__main__":
    main()
