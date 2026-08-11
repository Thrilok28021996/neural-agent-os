import { invoke } from '@tauri-apps/api/core'

export type CalendarEvent = { id: string; workspace_id: string; title: string; starts_at: string; ends_at: string; meeting_url?: string; provider: string; recurrence?: string }
export function listCalendarEvents(workspaceId: string) { return invoke<CalendarEvent[]>('list_calendar_events', { workspaceId }) }
export function createCalendarEvent(workspaceId: string, title: string, startsAt: string, endsAt: string, meetingUrl?: string, recurrence?: string) { return invoke<CalendarEvent>('create_calendar_event', { workspaceId, title, startsAt, endsAt, meetingUrl, recurrence }) }
export function updateCalendarEvent(eventId: string, title: string, startsAt: string, endsAt: string, meetingUrl?: string, recurrence?: string) { return invoke('update_calendar_event', { eventId, title, startsAt, endsAt, meetingUrl, recurrence }) }
export function deleteCalendarEvent(eventId: string, agentId?: string) { return invoke('delete_calendar_event', { eventId, agentId }) }
export function pullRemoteCalendar(connectionId: string) { return invoke<[number, number, number]>('pull_remote_calendar', { connectionId }) }

/** Schedule a reminder tied to a calendar event (plan §8). */
export function scheduleEventReminder(workspaceId: string, eventId: string, title: string, scheduledAt: string) {
  return invoke<import('./reminders').Reminder>('schedule_event_reminder', { workspaceId, eventId, title, scheduledAt })
}
