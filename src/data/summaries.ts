import { invoke } from './invoke'
import { DEFAULT_LMSTUDIO_MODEL } from './providers'

export type MeetingSummary = { meeting_id: string; model: string; summary: string; actions: Array<{ id: string; title: string; owner?: string; due_at?: string; status: string }> }

export function summarizeMeeting(meetingId: string, model = DEFAULT_LMSTUDIO_MODEL) {
  return invoke<MeetingSummary>('summarize_meeting', { meetingId, model })
}

/** Load a meeting's stored summary + actions (null when never summarized). */
export function getMeetingSummary(meetingId: string) {
  return invoke<MeetingSummary | null>('get_meeting_summary', { meetingId })
}
