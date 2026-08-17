import { invoke } from './invoke'

export type IndexResult = { sources_indexed: number; chunks_indexed: number }

export function indexSources(workspaceId: string) {
  return invoke<IndexResult>('index_sources', { workspaceId })
}
