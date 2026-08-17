import { invoke } from './invoke'
import type { SearchResult } from './retrieval'

export type HybridResult = SearchResult & { score: number }

/** Merge keyword (FTS5) and semantic results with normalized ranking. */
export function hybridSearch(workspaceId: string, query: string, model = 'nomic-embed-text') {
  return invoke<HybridResult[]>('hybrid_search', { workspaceId, query, model })
}
