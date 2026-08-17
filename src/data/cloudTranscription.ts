import { invoke } from './invoke'

export type CloudTranscriptionResult = {
  job_id: string
  meeting_id: string
  provider: string
  model: string
  status: string
  segments: number
}

export function cloudTranscribe(meetingId: string, recordingPath: string, provider: string, model: string, language?: string) {
  return invoke<CloudTranscriptionResult>('cloud_transcribe', { meetingId, recordingPath, provider, model, language })
}
