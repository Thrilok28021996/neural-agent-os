import { invoke } from '@tauri-apps/api/core'

export type DetectedMeeting = { event_id: string; title: string; starts_at: string; ends_at: string; meeting_url: string | null; workspace_id: string }

export function detectUpcomingMeetings(workspaceId: string, hoursAhead?: number) {
  return invoke<DetectedMeeting[]>('detect_upcoming_meetings', { workspaceId, hoursAhead })
}
