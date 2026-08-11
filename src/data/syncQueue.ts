import { invoke } from '@tauri-apps/api/core'

export type SyncQueueItem = {
  id: string; entity_type: string; entity_id: string; provider: string; action: string
  payload_json: string; status: string; retry_count: number; error?: string
  conflict_details?: string; created_at: string; synced_at?: string
}
export type SyncStatus = { provider: string; pending: number; synced: number; failed: number; conflicts: number; last_sync?: string }
export type ConflictResolution = { queue_id: string; resolution: string; resolved_by: string; resolved_at: string }

export function enqueueSync(entityType: string, entityId: string, provider: string, action: string, payload: Record<string, unknown>) {
  return invoke<SyncQueueItem>('enqueue_sync', { entityType, entityId, provider, action, payloadJson: JSON.stringify(payload) })
}
export function getSyncStatus(workspaceId: string) { return invoke<SyncStatus[]>('get_sync_status', { workspaceId }) }
export function resolveSyncConflict(queueId: string, resolution: string) { return invoke<ConflictResolution>('resolve_sync_conflict', { queueId, resolution }) }
