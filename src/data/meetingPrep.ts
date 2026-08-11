import { invoke } from '@tauri-apps/api/core'

export type MeetingPrep = {
  meeting_id: string
  title: string
  starts_at: string
  participants: string[]
  recent_related_emails: { subject: string; sender?: string; snippet: string }[]
  open_action_items: string[]
  suggested_agenda: string
  knowledge_preview: { title: string; content: string }[]
}

export function prepareMeeting(eventId: string) {
  return invoke<MeetingPrep>('prepare_meeting', { eventId })
}
