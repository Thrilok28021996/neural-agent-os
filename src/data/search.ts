import { invoke } from '@tauri-apps/api/core'

export type IndexResult = { sources_indexed: number; chunks_indexed: number }

export function indexSources(workspaceId: string) {
  return invoke<IndexResult>('index_sources', { workspaceId })
}
