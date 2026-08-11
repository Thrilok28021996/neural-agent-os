import { invoke } from '@tauri-apps/api/core'

export type OpenAction = { id: string; meeting_id: string; title: string; status: string }
export function listOpenActions(workspaceId: string) { return invoke<OpenAction[]>('list_open_actions', { workspaceId }) }
