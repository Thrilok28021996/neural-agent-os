import { invoke } from '@tauri-apps/api/core'

export type EmailHit = {
  id: string
  account_id: string
  subject: string
  sender?: string
  recipients?: string
  body?: string
  received_at?: string
  score?: number
}

export function searchEmails(workspaceId: string, query: string) {
  return invoke<EmailHit[]>('search_emails', { workspaceId, query })
}

export function semanticSearchEmails(workspaceId: string, query: string, model = 'nomic-embed-text') {
  return invoke<EmailHit[]>('semantic_search_emails', { workspaceId, query, model })
}
