import { invoke } from '@tauri-apps/api/core'

export type EmailThread = { thread_id: string; subject: string; message_count: number; participants: string[]; oldest_at?: string; newest_at?: string; messages: { email_id: string; subject: string; sender?: string; body_preview: string; received_at?: string; position: number }[] }
export type EmailLabel = { id: string; account_id: string; name: string; provider_label_id?: string; color?: string }

export function rebuildThreads(workspaceId: string) { return invoke<EmailThread[]>('rebuild_threads', { workspaceId }) }
export function createLabel(accountId: string, name: string, color?: string) { return invoke<EmailLabel>('create_label', { accountId, name, color }) }
export function listLabels(accountId: string) { return invoke<EmailLabel[]>('list_labels', { accountId }) }
export function labelEmail(emailId: string, labelId: string) { return invoke('label_email', { emailId, labelId }) }
export function indexAttachment(emailId: string, filename: string, contentType: string, sizeBytes: number) { return invoke('index_attachment', { emailId, filename, contentType, sizeBytes }) }
export function generateEmailDraft(workspaceId: string, instructions: string, context: string, model?: string) { return invoke<string>('generate_email_draft', { workspaceId, instructions, context, model: model ?? 'qwen3:14b' }) }
export function summarizeEmailThread(threadId: string, workspaceId: string, model?: string) { return invoke<string>('summarize_email_thread', { threadId, workspaceId, model: model ?? 'qwen3:14b' }) }
export function extractEmailActionsFromEmail(emailId: string, workspaceId: string, model?: string) { return invoke<string[]>('extract_email_actions', { emailId, workspaceId, model: model ?? 'qwen3:14b' }) }
export function extractAttachmentText(path: string, filename: string) { return invoke<string>('extract_attachment_text', { path, filename }) }
export function suggestWorkspaceForEmail(emailId: string) { return invoke<string>('suggest_workspace_for_email', { emailId }) }
