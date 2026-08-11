import { invoke } from '@tauri-apps/api/core'

export function launchMeetingBot(platform: string, meetingUrl: string, displayName?: string, durationMinutes?: number) {
  return invoke<string>('launch_meeting_bot', { platform, meetingUrl, displayName, durationMinutes })
}
