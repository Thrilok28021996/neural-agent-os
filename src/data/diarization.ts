import { invoke } from '@tauri-apps/api/core'

export type DiarizedSegment = {
  segment_id: string
  start_seconds: number
  end_seconds: number
  text: string
  speaker_label: string
  confidence: number
  matched_profile_id?: string
}

export type DiarizationResult = {
  segments: DiarizedSegment[]
  speaker_count: number
  embedding_model: string
}

export type SpeakerEmbedding = {
  profile_id: string
  speaker_label: string
  embedding: number[]
  sample_count: number
}

export function diarizeTranscript(meetingId: string, workspaceId: string, embeddingModel?: string, similarityThreshold?: number) {
  return invoke<DiarizationResult>('diarize_transcript', { meetingId, workspaceId, embeddingModel, similarityThreshold })
}

export function buildSpeakerEmbedding(profileId: string, model?: string) {
  return invoke<SpeakerEmbedding>('build_speaker_embedding', { profileId, model })
}
