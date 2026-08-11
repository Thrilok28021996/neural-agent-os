import { invoke } from '@tauri-apps/api/core'

/** Start push-to-talk mic capture (desktop fallback when Web Speech API is unavailable). */
export function startVoiceCapture(language?: string) {
  return invoke<string>('start_voice_capture', { language })
}

/** Stop capture and transcribe the clip with local Whisper. */
export function stopVoiceCapture() {
  return invoke<string>('stop_voice_capture')
}
