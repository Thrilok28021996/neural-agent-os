import { invoke } from './invoke'
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification'

export type Reminder = { id: string; workspace_id: string; task_id?: string; calendar_event_id?: string; title: string; scheduled_at: string; status: string }

export function scheduleTaskReminder(workspaceId: string, taskId: string, title: string, scheduledAt: string) {
  return invoke<Reminder>('schedule_task_reminder', { workspaceId, taskId, title, scheduledAt })
}

export function listReminders(workspaceId: string) {
  return invoke<Reminder[]>('list_reminders', { workspaceId })
}

/** Mark a scheduled reminder as cancelled (soft-cancel — keeps the DB row). */
export function cancelReminder(reminderId: string) {
  return invoke<void>('cancel_reminder', { reminderId })
}

/** Permanently delete a reminder row from the database. */
export function deleteReminder(reminderId: string) {
  return invoke<void>('delete_reminder', { reminderId })
}

export async function notifyReminder(reminder: Reminder) {
  let granted = await isPermissionGranted()
  if (!granted) granted = (await requestPermission()) === 'granted'
  if (granted) sendNotification({ title: 'Neural reminder', body: reminder.title })
}
