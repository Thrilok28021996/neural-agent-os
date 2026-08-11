import { invoke } from '@tauri-apps/api/core'

export type Note = { id: string; workspace_id: string; title: string; content: string; created_at: string; updated_at: string }

export function createNote(workspaceId: string, title: string, content: string) {
  return invoke<Note>('create_note', { workspaceId, title, content })
}

export function updateNote(noteId: string, title: string, content: string) {
  return invoke<Note>('update_note', { noteId, title, content })
}

export function listNotes(workspaceId: string) {
  return invoke<Note[]>('list_notes', { workspaceId })
}

export function deleteNote(noteId: string) {
  return invoke('delete_note', { noteId })
}

export function searchNotes(workspaceId: string, query: string) {
  return invoke<Note[]>('search_notes', { workspaceId, query })
}
