import { invoke } from '@tauri-apps/api/core'

export type TranscriptionJob = { id: string; meeting_id: string; provider: string; status: string; error?: string }

export function queueTranscription(meetingId: string, provider = 'local-whisper') {
  return invoke<TranscriptionJob>('queue_transcription', { meetingId, provider })
}

export function listTranscriptionJobs(meetingId: string) {
  return invoke<TranscriptionJob[]>('list_transcription_jobs', { meetingId })
}

export function processTranscriptionJob(jobId: string) {
  return invoke<TranscriptionJob>('process_transcription_job', { jobId })
}

/** Re-queue transcription for an existing meeting (plan §7: reprocessing). */
export function reprocessTranscription(meetingId: string, provider = 'local-whisper') {
  return invoke<TranscriptionJob>('reprocess_transcription', { meetingId, provider })
}
