import { invoke } from '@tauri-apps/api/core'

/** Speak text using a local TTS engine (macOS say, Linux espeak-ng, Windows SAPI) */
export function localTextToSpeech(text: string, language?: string) {
  return invoke<string>('local_text_to_speech', { text, language })
}

/** Transcribe audio using local Whisper */
export function localSpeechToText(audioPath: string, language?: string) {
  return invoke<string>('local_speech_to_text', { audioPath, language })
}
