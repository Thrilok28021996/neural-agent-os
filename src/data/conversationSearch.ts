import { invoke } from '@tauri-apps/api/core'
import type { SearchResult } from './retrieval'

export function conversationSearch(workspaceId: string, query: string) {
  return invoke<SearchResult[]>('conversation_search', { workspaceId, query })
}
