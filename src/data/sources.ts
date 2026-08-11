import { invoke } from '@tauri-apps/api/core'

export type Source = { id: string; workspace_id: string; kind: string; title: string; uri?: string; status: string }
export function listSources(workspaceId: string) { return invoke<Source[]>('list_sources', { workspaceId }) }
export function deleteSource(sourceId: string) { return invoke('delete_source', { sourceId }) }
