#!/usr/bin/env python3
"""Audio-based speaker diarization backend for Neural Agent OS.

Optional backend used by the Rust app (`diarization::diarize_audio`, resolved
via NEURAL_DIARIZE_BIN or PATH) to assign speakers to transcript segments by
time overlap. Requires `pyannote.audio` (and torch) plus a Hugging Face token
with access to the diarization model. When the dependencies or token are
missing the script exits non-zero and the app falls back to transcript-text
embedding diarization.

Contract (stdout):
    {"segments": [{"start": <s>, "end": <e>, "speaker": "SPEAKER_00"}, ...]}

Usage:
    python diarize.py --json <audio> [--hf-token TOKEN] [--model MODEL] [--stub]
"""

import argparse
import json
import os
import sys

DEFAULT_MODEL = "pyannote/speaker-diarization-3.1"


def duration_seconds(path: str) -> float:
    """Best-effort audio duration; WAV via stdlib, otherwise ffprobe."""
    if path.lower().endswith(".wav"):
        try:
            import wave
            with wave.open(path, "rb") as w:
                frames = w.getnframes()
                rate = w.getframerate()
                if rate:
                    return frames / rate
        except Exception:
            pass
    try:
        import subprocess
        out = subprocess.run(
            ["ffprobe", "-v", "error", "-show_entries", "format=duration",
             "-of", "default=noprint_wrappers=1:nokey=1", path],
            capture_output=True, text=True, timeout=30,
        )
        if out.returncode == 0 and out.stdout.strip():
            return float(out.stdout.strip())
    except Exception:
        pass
    return 60.0  # fallback: assume one minute


def stub_windows(duration: float, gap: float = 0.5) -> list:
    """Deterministic placeholder windows (two speakers) for local validation.

    Only used with `--stub`; never claim these are real diarization results.
    """
    half = max(duration / 2.0, 1.0)
    return [
        {"start": 0.0, "end": max(half - gap, 0.5), "speaker": "SPEAKER_00"},
        {"start": min(half, duration), "end": duration, "speaker": "SPEAKER_01"},
    ]


def diarize(path: str, model: str, hf_token: str, backend: str) -> list:
    """Run a real audio diarization model and return speaker windows.

    Default backend is `simple` (simple-diarizer: speechbrain ECAPA embeddings
    + spectral clustering; models are ungated). The `pyannote` backend needs a
    Hugging Face token accepted for the gated pyannote models.
    """
    if backend == "pyannote":
        try:
            from pyannote.audio import Pipeline
        except ImportError as exc:
            raise RuntimeError(
                "pyannote.audio is not installed in the diarizer environment "
                "(`pip install pyannote.audio`)."
            ) from exc
        if not hf_token:
            raise RuntimeError(
                "A Hugging Face token is required to load the pyannote model "
                "(set NEURAL_DIARIZE_HF_TOKEN or pass --hf-token)."
            )
        # pyannote.audio 4.x: pass the token explicitly (use_auth_token was removed).
        pipeline = Pipeline.from_pretrained(model, token=hf_token)
        diarization = pipeline(path)
        windows = []
        for turn, _, speaker in diarization.itertracks(yield_label=True):
            windows.append({"start": turn.start, "end": turn.end, "speaker": speaker})
        windows.sort(key=lambda w: w["start"])
        return windows

    # simple-diarizer backend (default): silero VAD + speechbrain ECAPA
    # embeddings, then our own silhouette-selected spectral clustering (the
    # library's built-in auto-clustering over-segments on real audio).
    try:
        import contextlib
        import io as _io

        import numpy as np
        import torch
        import torchaudio
        from simple_diarizer.diarizer import Diarizer
    except ImportError as exc:
        raise RuntimeError(
            "the simple diarizer backend is not installed in the diarizer "
            "environment (`pip install simple-diarizer speechbrain scikit-learn`)."
        ) from exc

    # torchcodec's native AudioDecoder dylib can fail to load in some
    # environments even though `import torchaudio` succeeds; route every
    # torchaudio.load through the stdlib `wave` module instead (recordings are
    # WAV at this point; anything else is converted to WAV by ffmpeg first).
    def _load_wav(wav_path):
        import wave as _wave

        with _wave.open(str(wav_path), "rb") as w:
            rate = w.getframerate()
            sampwidth = w.getsampwidth()
            frames = w.readframes(w.getnframes())
        if sampwidth == 2:
            raw = np.frombuffer(frames, dtype=np.int16)
        elif sampwidth == 4:
            raw = np.frombuffer(frames, dtype=np.int32)
        else:
            raw = np.frombuffer(frames, dtype=np.uint8)
        signal = raw.astype(np.float32) / 32768.0
        return torch.from_numpy(signal).unsqueeze(0), rate

    torchaudio.load = _load_wav

    diarizer = Diarizer(embed_model="ecapa", cluster_method="sc")
    with contextlib.redirect_stdout(_io.StringIO()):
        signal, fs = torchaudio.load(path)
        speech_ts = diarizer.vad(signal[0])
        embeds, window_samples = diarizer.recording_embeds(signal, fs, speech_ts)

    if len(embeds) == 0:
        return []
    if len(embeds) == 1:
        labels = [0]
    else:
        # Agglomerative clustering on cosine distance with a fixed threshold:
        # same-speaker windows merge until the cosine distance between clusters
        # exceeds the threshold. ECAPA same-speaker distances are typically
        # < 0.4 while different speakers are > 0.5.
        from sklearn.cluster import AgglomerativeClustering

        model = AgglomerativeClustering(
            n_clusters=None,
            metric="cosine",
            linkage="average",
            distance_threshold=0.45,
        )
        labels = model.fit_predict(embeds).tolist()

    # Map clustered windows (start,end sample indices) to speaker segments,
    # merging consecutive windows that share a label.
    windows = []
    for idx, (start_sample, end_sample) in enumerate(window_samples):
        windows.append({
            "start": float(start_sample) / float(fs),
            "end": float(end_sample) / float(fs),
            "speaker": f"SPEAKER_{labels[idx]:02d}",
        })
    windows.sort(key=lambda w: w["start"])

    merged = []
    for w in windows:
        if merged and merged[-1]["speaker"] == w["speaker"] and w["start"] <= merged[-1]["end"] + 0.35:
            merged[-1]["end"] = max(merged[-1]["end"], w["end"])
        else:
            merged.append(dict(w))

    # Reassign very short fragments (< 0.5s) to the temporally nearest window:
    # they are usually boundary artifacts of the sliding embedding windows.
    for i, w in enumerate(merged):
        if w["end"] - w["start"] >= 0.5 or len(merged) == 1:
            continue
        before = merged[i - 1] if i > 0 else None
        after = merged[i + 1] if i + 1 < len(merged) else None
        if before and after:
            w["speaker"] = before["speaker"] if (w["start"] - before["end"]) <= (after["start"] - w["end"]) else after["speaker"]
        elif before:
            w["speaker"] = before["speaker"]
        elif after:
            w["speaker"] = after["speaker"]
    # Merge windows that now share a speaker and touch/overlap.
    final = []
    for w in merged:
        if final and final[-1]["speaker"] == w["speaker"] and w["start"] <= final[-1]["end"] + 0.01:
            final[-1]["end"] = max(final[-1]["end"], w["end"])
        else:
            final.append(dict(w))
    return final


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit JSON to stdout")
    parser.add_argument("audio", help="audio file to diarize")
    parser.add_argument("--hf-token", default=os.environ.get("NEURAL_DIARIZE_HF_TOKEN") or os.environ.get("HF_TOKEN", ""))
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--stub", action="store_true",
                        help="emit deterministic placeholder windows (testing only)")
    parser.add_argument("--backend", choices=["simple", "pyannote"], default="simple",
                        help="audio diarization backend (default: simple-diarizer)")
    args = parser.parse_args(argv)

    try:
        if args.stub:
            windows = stub_windows(duration_seconds(args.audio))
        else:
            windows = diarize(args.audio, args.model, args.hf_token, args.backend)
    except Exception as exc:  # noqa: BLE001 - surface any backend failure to the app
        print(f"diarize: {exc}", file=sys.stderr)
        return 1

    if args.json:
        json.dump({"segments": windows}, sys.stdout)
    else:
        for w in windows:
            print(f"{w['start']:8.2f} {w['end']:8.2f} {w['speaker']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
