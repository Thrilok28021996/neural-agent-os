import { invoke } from './invoke'

export type SearchResult = { source_id: string; title: string; uri: string; chunk_id: string; content: string }

export function searchSources(workspaceId: string, query: string) {
  return invoke<SearchResult[]>('search_sources', { workspaceId, query })
}
