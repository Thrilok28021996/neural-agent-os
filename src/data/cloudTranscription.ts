import { invoke } from '@tauri-apps/api/core'

export type CloudTranscriptionResult = {
  job_id: string
  meeting_id: string
  provider: string
  model: string
  status: string
  segments: number
}

export function cloudTranscribe(meetingId: string, recordingPath: string, provider: string, model: string) {
  return invoke<CloudTranscriptionResult>('cloud_transcribe', { meetingId, recordingPath, provider, model })
}
