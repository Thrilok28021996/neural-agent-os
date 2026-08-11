import { invoke } from '@tauri-apps/api/core'

export type TranscriptSegment = { id: string; speaker?: string; start_seconds: number; end_seconds: number; text: string; confidence?: number }

export function listTranscriptSegments(meetingId: string) {
  return invoke<TranscriptSegment[]>('list_transcript_segments', { meetingId })
}

export function updateTranscriptSpeaker(segmentId: string, speaker: string) {
  return invoke('update_transcript_speaker', { segmentId, speaker })
}

/** Correct transcript text (plan §7: transcript editing). */
export function updateTranscriptSegmentText(segmentId: string, text: string) {
  return invoke<void>('update_transcript_segment_text', { segmentId, text })
}
