import { invoke } from '@tauri-apps/api/core'

export type MonitoredFolder = { id: string; workspace_id: string; path: string; enabled: boolean; last_checked_at?: string; files_found: number }

export function addMonitoredFolder(workspaceId: string, path: string) { return invoke<MonitoredFolder>('add_monitored_folder', { workspaceId, path }) }
export function listMonitoredFolders(workspaceId: string) { return invoke<MonitoredFolder[]>('list_monitored_folders', { workspaceId }) }
export function toggleMonitoredFolder(folderId: string, enabled: boolean) { return invoke('toggle_monitored_folder', { folderId, enabled }) }
export function removeMonitoredFolder(folderId: string) { return invoke('remove_monitored_folder', { folderId }) }
