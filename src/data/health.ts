import { invoke } from './invoke'

export type ProviderHealth = { provider: string; capability: string; model: string; healthy: boolean; last_checked_at?: string; response_time_ms?: number; error_message?: string; consecutive_failures: number }

export function refreshProviderHealth(workspaceId: string) { return invoke<ProviderHealth[]>('refresh_provider_health', { workspaceId }) }
export function getProviderHealth(workspaceId: string) { return invoke<ProviderHealth[]>('get_provider_health', { workspaceId }) }
