import { invoke } from '@tauri-apps/api/core'
import type { SearchResult } from './retrieval'

export function semanticSearchSources(workspaceId: string, query: string, model = 'nomic-embed-text') {
  return invoke<SearchResult[]>('semantic_search_sources', { workspaceId, query, model })
}
