import { invoke } from './invoke'

export type ExpandedEvent = { event_id: string; instance_date: string; title: string; starts_at: string; ends_at: string; meeting_url?: string; is_recurrence: boolean }
export type TimeZoneInfo = { iana_name: string; utc_offset_hours: number; display_name: string }

export function expandRecurringEvent(eventId: string, fromDate: string, toDate: string) { return invoke<ExpandedEvent[]>('expand_recurring_event', { eventId, fromDate, toDate }) }
export function commonTimezones() { return invoke<TimeZoneInfo[]>('common_timezones') }
export function evaluateRecordingRules(workspaceId: string, eventTitle: string, meetingUrl?: string, participants?: string[]) { return invoke<string>('evaluate_recording_rules', { workspaceId, eventTitle, meetingUrl, participants: participants ?? [] }) }
export type RecordingRule = { id: string; workspace_id: string; rule_type: string; pattern: string; action: string; priority: number }
export function setRecordingRule(workspaceId: string, ruleType: string, pattern: string, action: string, priority: number) { return invoke('set_recording_rule', { workspaceId, ruleType, pattern, action, priority }) }
export function listRecordingRules(workspaceId: string) { return invoke<RecordingRule[]>('list_recording_rules', { workspaceId }) }
export function deleteRecordingRule(ruleId: string) { return invoke('delete_recording_rule', { ruleId }) }
