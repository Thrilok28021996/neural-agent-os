import { invoke } from './invoke'

export type Memory = {
  id: string
  workspace_id: string
  content: string
  source_kind?: string
  source_id?: string
  created_at: string
  updated_at: string
}

export function createMemory(workspaceId: string, content: string, sourceKind?: string, sourceId?: string) {
  return invoke<Memory>('create_memory', { workspaceId, content, sourceKind, sourceId })
}

export function listMemories(workspaceId: string) {
  return invoke<Memory[]>('list_memories', { workspaceId })
}

export function searchMemories(workspaceId: string, query: string) {
  return invoke<Memory[]>('search_memories', { workspaceId, query })
}

export function deleteMemory(memoryId: string) {
  return invoke<void>('delete_memory', { memoryId })
}
