import { invoke } from '@tauri-apps/api/core'

export type Meeting = { id: string; title: string; recording_path?: string; status: string }

export function importMeetingRecording(workspaceId: string, path: string, title: string) {
  return invoke<Meeting>('import_meeting_recording', { workspaceId, path, title })
}

export function listMeetings(workspaceId: string) {
  return invoke<Meeting[]>('list_meetings', { workspaceId })
}

/** Batch-import multiple recording files (plan §6: batch import). */
export function importMeetingRecordingsBatch(workspaceId: string, paths: string[]) {
  return invoke<Meeting[]>('import_meeting_recordings_batch', { workspaceId, paths })
}
