import { invoke } from './invoke'
import type { SearchResult } from './retrieval'

export type RerankedResult = SearchResult & { relevance_score: number }

export function rerankSearch(workspaceId: string, query: string) {
  return invoke<RerankedResult[]>('rerank_search', { workspaceId, query })
}
