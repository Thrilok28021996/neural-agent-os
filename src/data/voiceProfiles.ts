import { invoke } from './invoke'

export type VoiceProfile = { id: string; workspace_id: string; speaker_label: string; sample_count: number }

export function createVoiceProfile(workspaceId: string, speakerLabel: string) {
  return invoke<VoiceProfile>('create_voice_profile', { workspaceId, speakerLabel })
}

export function listVoiceProfiles(workspaceId: string) {
  return invoke<VoiceProfile[]>('list_voice_profiles', { workspaceId })
}

/** Permanently delete a voice profile and all associated embeddings. */
export function deleteVoiceProfile(profileId: string) {
  return invoke<void>('delete_voice_profile', { profileId })
}
