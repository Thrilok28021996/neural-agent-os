# Diarization test fixtures

Real-speech WAV fixtures used by the gated end-to-end test
(`NAO_E2E_DIARIZE=1 cargo test --lib e2e_multilingual_transcription_and_audio_diarization`)
and for validating `scripts/diarize.py`.

| File | Content | Ground truth |
|---|---|---|
| `hi_conversation.wav` | Hindi speech, macOS `say` voice **Lekha** (hi_IN), two sentences | 1 speaker |
| `te_conversation.wav` | Telugu speech, macOS `say` voice **Geeta** (te_IN), two sentences | 1 speaker |
| `mixed_hi_te.wav` | Lekha ↔ Geeta alternating (4 turns, 0.6 s gaps) | 2 speakers |

`expected_speaker_windows.json` records the exact turn windows of
`mixed_hi_te.wav` (speaker, start, end in seconds) used to check the
diarizer's output.

Regenerate with:

```bash
say -v Lekha -o hi1.wav --data-format=LEI16@16000 '<Hindi sentence>'
say -v Geeta -o te1.wav --data-format=LEI16@16000 '<Telugu sentence>'
# then stitch at 16 kHz mono (see the E2E fixture builder in the session log)
```
