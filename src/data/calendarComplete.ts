import { invoke } from '@tauri-apps/api/core'

export type ExpandedEvent = { event_id: string; instance_date: string; title: string; starts_at: string; ends_at: string; meeting_url?: string; is_recurrence: boolean }
export type TimeZoneInfo = { iana_name: string; utc_offset_hours: number; display_name: string }

export function expandRecurringEvent(eventId: string, fromDate: string, toDate: string) { return invoke<ExpandedEvent[]>('expand_recurring_event', { eventId, fromDate, toDate }) }
export function commonTimezones() { return invoke<TimeZoneInfo[]>('common_timezones') }
export function evaluateRecordingRules(workspaceId: string, eventTitle: string, meetingUrl?: string, participants?: string[]) { return invoke<string>('evaluate_recording_rules', { workspaceId, eventTitle, meetingUrl, participants: participants ?? [] }) }
export function setRecordingRule(workspaceId: string, ruleType: string, pattern: string, action: string, priority: number) { return invoke('set_recording_rule', { workspaceId, ruleType, pattern, action, priority }) }
