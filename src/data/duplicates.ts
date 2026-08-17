import { invoke } from './invoke'

export type DuplicateSource = { id: string; title: string; uri?: string; created_at: string }
export type DuplicateGroup = { content_hash: string; source_count: number; sources: DuplicateSource[] }

export function findDuplicates(workspaceId: string) {
  return invoke<DuplicateGroup[]>('find_duplicates', { workspaceId })
}
