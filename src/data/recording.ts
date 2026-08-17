import { invoke } from './invoke'

export type RecordingStatus = { active: boolean; path?: string; meeting_id?: string; queued?: boolean }
export type RecordingDecision = { allowed: boolean; action: string; reason: string }
export type RecordingContext = {
  workspaceId?: string
  eventTitle?: string
  meetingUrl?: string
  participants?: string[]
  confirmed?: boolean
}

export function startLocalRecording(path: string, source = 'default', context: RecordingContext = {}) {
  return invoke<RecordingStatus>('start_local_recording', {
    path,
    source,
    workspaceId: context.workspaceId,
    eventTitle: context.eventTitle,
    meetingUrl: context.meetingUrl,
    participants: context.participants ?? [],
    confirmed: context.confirmed ?? false,
  })
}
export function stopLocalRecording() { return invoke<RecordingStatus>('stop_local_recording') }
export function checkRecordingAllowed(workspaceId?: string, eventTitle?: string, meetingUrl?: string, participants: string[] = []) {
  return invoke<RecordingDecision>('check_recording_allowed', { workspaceId, eventTitle, meetingUrl, participants })
}
