import { invoke } from '@tauri-apps/api/core'

export type IngestionResult = { discovered: number; inserted: number; directory: string }

export function ingestDirectory(workspaceId: string, directory: string) {
  return invoke<IngestionResult>('ingest_directory', { workspaceId, directory })
}
