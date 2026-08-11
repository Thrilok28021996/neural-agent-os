import { invoke } from '@tauri-apps/api/core'

export type MeetingSummary = { meeting_id: string; model: string; summary: string; actions: Array<{ id: string; title: string; owner?: string; due_at?: string; status: string }> }

export function summarizeMeeting(meetingId: string, model = 'qwen3:14b') {
  return invoke<MeetingSummary>('summarize_meeting', { meetingId, model })
}
