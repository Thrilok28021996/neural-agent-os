import { invoke } from './invoke'

export type TriageItem = {
  email_id: string
  subject: string
  sender: string
  received_at: string
  reason: string
}

export function emailTriage(workspaceId: string) {
  return invoke<TriageItem[]>('email_triage', { workspaceId })
}

export function researchBrief(workspaceId: string, query: string, model?: string) {
  return invoke<string>('research_brief', { workspaceId, query, model })
}
