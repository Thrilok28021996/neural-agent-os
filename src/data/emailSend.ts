import { invoke } from './invoke'

export type EmailSendResult = { sent: boolean; message: string }

export function sendEmail(accountId: string, to: string, subject: string, body: string, agentId?: string) {
  return invoke<EmailSendResult>('send_email', { accountId, to, subject, body, agentId })
}
