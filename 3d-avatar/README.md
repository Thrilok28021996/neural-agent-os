# 🤖 Ava — Your 3D Human AI Avatar

A fully interactive **human-proportioned 3D avatar** that talks, reacts, and
responds with real AI — including **hands-free voice mode**. Also integrated
into the **neural-agent-os** app (Sidebar → "3D View").

## What Ava does
- 🧍 **Human structure** — head with hair + expressive face (eyes, brows, nose,
  lips), neck, shoulders, torso, and two-joint arms with hands. No robot parts.
- 🎙️ **Voice mode (hands-free)** — one click, then just *talk*: Ava listens,
  answers aloud, and keeps listening for your next sentence. No push-to-talk.
- 💬 **Chats with you** — replies come from your local **LM Studio** model
  (`qwen/qwen3.5-9b` at `localhost:1234`), with built-in witty fallbacks if it's off.
- 🔊 **Voice output** — speaks replies aloud (browser TTS in the demo; native
  macOS `say` / espeak-ng / SAPI in the desktop app, incl. Hindi/Telugu voices).
- 😊 **Reacts emotionally** — happy (smile + blush), thinking (brow furrow,
  hand at chin, eyes up), surprised, sad, listening (attentive lean-in);
  talking mouth animates at speech cadence.
- 👋 **Gestures** — waves, nods, shakes head, shrugs, bounces when excited.
- 🖱️ **Click her** — she reacts ("Boop! You activated me!").
- 🌀 Drag to orbit, scroll to zoom (standalone demo).

## Run the standalone demo
```bash
cd 3d-avatar
python3 server.py            # serves the page AND proxies /api/* to LM Studio
# open http://localhost:8000
```
- **Voice mode** works in Chrome/Edge (Web Speech API). Click **🎙️ Voice mode**,
  then just talk — she replies aloud and listens again.
- The small 🎤 mic in the chat is classic push-to-talk.

## In the neural-agent-os app
- `src/components/HumanAvatar.tsx` — the human avatar component
  (three.js, same `state`/`size`/`speech`/`onStateChange` API as the old orb).
- `src/App.tsx` → `ThreeDView` (Sidebar → **3D View**):
  - **🎙️ Voice mode** button → hands-free loop using the desktop mic
    (`start_voice_capture` + local Whisper) and native TTS
    (`local_text_to_speech`); tap the avatar to end your turn, she auto-re-listens.
  - Falls back to the browser's Web Speech API when running the web build.
  - Chat goes through the existing `ask_assistant` Tauri command (Rust → LM Studio).

## Files
- `3d-avatar/index.html` — standalone demo (human avatar, chat UI, LLM
  proxy-first brain, TTS/STT, hands-free voice mode).
- `3d-avatar/server.py` — zero-dependency Python server: static files +
  `/api/*` proxy to `http://localhost:1234/*` (removes browser CORS).
- `src/components/HumanAvatar.tsx` — the in-app React component.


## ⚡ Speed (voice mode)
| step | before | after |
|---|---|---|
| Whisper transcription | ~75s (default `small` model) | **~1.2s** (`base`, `--beam_size 1`; override with `NEURAL_WHISPER_MODEL`) |
| Reply | waited for full generation | **streamed** — Ava starts speaking while the model writes (first audio ~3-4s) |
| Auto-answer pause | ~2s | ~1.2-1.6s |

- App: new `stream_voice_reply` Tauri command (LM Studio SSE → IPC channel → incremental TTS).
- Demo: fetch SSE through the proxy with the same incremental TTS.

## Tested
Headless Chromium (with a mocked SpeechRecognition): voice mode on → simulated
utterance → real LLM reply via LM Studio → spoken reply → **auto re-listen**,
no runaway restart loop, clean toggle off — **zero console errors**. App:
`tsc -b` + `vite build` clean, **532 unit/integration tests pass**.
