import { invoke } from '@tauri-apps/api/core'
import type { Workspace } from './workspaces'

export function createWorkspace(name: string) {
  return invoke<Workspace>('create_workspace', { name })
}

export function deleteWorkspace(workspaceId: string) {
  return invoke('delete_workspace', { workspaceId })
}

export function deleteWorkspaceComplete(workspaceId: string) {
  return invoke('delete_workspace_complete', { workspaceId })
}

export type WorkspaceDeletionPreview = {
  workspace_id: string
  workspace_name: string
  sources: number
  meetings: number
  transcript_segments: number
  tasks: number
  reminders: number
  notes: number
  emails: number
  agents: number
  recordings: string[]
}

export function previewWorkspaceDeletion(workspaceId: string) {
  return invoke<WorkspaceDeletionPreview>('preview_workspace_deletion', { workspaceId })
}
