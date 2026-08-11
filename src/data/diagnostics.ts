import { invoke } from '@tauri-apps/api/core'

export type StorageStats = {
  database_path: string
  database_size_bytes: number
  recordings_count: number
  recordings_size_bytes: number
  recordings_paths: { meeting_id: string; title: string; path: string; size_bytes: number; created_at?: string; status: string }[]
  total_sources: number
  total_embeddings: number
  total_emails: number
  oldest_recording_date?: string
  storage_warning?: string
}

export type AuditEntry = {
  id: string
  timestamp: string
  action: string
  provider?: string
  data_type?: string
  workspace_id?: string
  details: string
}

export function getStorageStats() { return invoke<StorageStats>('get_storage_stats') }
export function cleanupOldRecordings(olderThanDays: number, dryRun?: boolean) { return invoke<string[]>('cleanup_old_recordings', { olderThanDays, dryRun }) }
export function getAuditLog(workspaceId?: string, limit?: number) { return invoke<AuditEntry[]>('get_audit_log', { workspaceId, limit }) }
