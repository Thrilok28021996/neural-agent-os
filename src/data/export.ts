import { invoke } from './invoke'

export function exportWorkspace(workspaceId: string, path: string) {
  return invoke<string>('export_workspace', { workspaceId, path })
}

export function importWorkspace(path: string) {
  return invoke<string>('import_workspace', { path })
}

/** Validate a backup file's integrity (version/workspace_id/exported_at). */
export function verifyBackup(path: string) {
  return invoke<boolean>('verify_backup', { path })
}
