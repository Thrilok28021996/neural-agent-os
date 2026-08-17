import { invoke } from './invoke'

export type Task = { id: string; workspace_id: string; title: string; due_at?: string; status: string }

/** Create a task directly (not from a meeting action). dueAt must be an ISO-8601 / RFC3339 string if provided. */
export function createTask(workspaceId: string, title: string, dueAt?: string) { return invoke<Task>('create_task', { workspaceId, title, dueAt }) }
export function deleteTask(taskId: string) { return invoke<void>('delete_task', { taskId }) }
export function promoteAction(actionId: string, workspaceId: string) { return invoke<Task>('promote_action_to_task', { actionId, workspaceId }) }
export function listTasks(workspaceId: string) { return invoke<Task[]>('list_tasks', { workspaceId }) }
export function updateTaskStatus(taskId: string, status: string) { return invoke('update_task_status', { taskId, status }) }
