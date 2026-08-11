import { invoke } from '@tauri-apps/api/core'

export function exportWorkspace(workspaceId: string, path: string) {
  return invoke<string>('export_workspace', { workspaceId, path })
}

export function importWorkspace(path: string) {
  return invoke<string>('import_workspace', { path })
}
