import { invoke } from './invoke'

export type CalendarConnection = { id: string; workspace_id: string; provider: string; account_label: string; status: string; last_synced_at?: string }
export function listCalendarConnections(workspaceId: string) { return invoke<CalendarConnection[]>('list_calendar_connections', { workspaceId }) }
export function connectCalendarProvider(workspaceId: string, provider: 'google' | 'outlook', accountLabel: string) { return invoke<CalendarConnection>('connect_calendar_provider', { workspaceId, provider, accountLabel }) }
