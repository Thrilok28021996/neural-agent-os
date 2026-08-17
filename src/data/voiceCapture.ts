import { invoke } from './invoke'

export type VoiceSilenceStatus = {
  has_capture: boolean
  file_exists: boolean
  has_speech: boolean
  trailing_rms: number
  duration_secs: number
}

/** Start push-to-talk mic capture (desktop fallback when Web Speech API is unavailable). */
export function startVoiceCapture(language?: string) {
  return invoke<string>('start_voice_capture', { language })
}

/** Stop capture and transcribe the clip with local Whisper. */
export function stopVoiceCapture() {
  return invoke<string>('stop_voice_capture')
}

/**
 * Poll the in-progress mic capture for hands-free voice mode: reports whether
 * speech has been heard and how silent the trailing audio is, so the UI can
 * auto-answer ~2s after the user stops talking (no tap needed).
 */
export function checkVoiceSilence() {
  return invoke<VoiceSilenceStatus>('check_voice_silence')
}
