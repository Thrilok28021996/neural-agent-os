import { invoke } from './invoke'
import { DEFAULT_LMSTUDIO_MODEL } from './providers'
import type { Task } from './tasks'

export function extractEmailActions(emailId: string, workspaceId?: string, model?: string) {
  return invoke<string[]>('extract_email_actions', { emailId, workspaceId: workspaceId ?? 'personal', model: model ?? DEFAULT_LMSTUDIO_MODEL })
}

export function linkTranscriptToProfile(segmentId: string, profileId: string) {
  return invoke('link_transcript_to_profile', { segmentId, profileId })
}
