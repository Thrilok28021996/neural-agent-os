import { invoke } from './invoke'

export type EmailAccount = { id: string; workspace_id: string; provider: string; account_label: string; imap_host?: string; smtp_host?: string; status: string }
export function connectEmailAccount(workspaceId: string, provider: 'gmail' | 'outlook' | 'imap', accountLabel: string, imapHost?: string, smtpHost?: string) { return invoke<EmailAccount>('connect_email_account', { workspaceId, provider, accountLabel, imapHost, smtpHost }) }
export function listEmailAccounts(workspaceId: string) { return invoke<EmailAccount[]>('list_email_accounts', { workspaceId }) }
export function storeEmailSecret(accountId: string, secretKind: 'username' | 'password' | 'oauth_refresh_token', value: string) { return invoke('store_email_secret', { accountId, secretKind, value }) }
export function syncEmailAccount(accountId: string) { return invoke<{ account_id: string; fetched: number; stored: number }>('sync_email_account', { accountId }) }
export function syncEmailOAuth(accountId: string) { return invoke<{ account_id: string; fetched: number; stored: number }>('sync_email_oauth', { accountId }) }
export type Email = { id: string; account_id: string; subject: string; sender?: string; recipients?: string; body?: string; received_at?: string }
export function listEmails(workspaceId: string, query = '') { return invoke<Email[]>('list_emails', { workspaceId, query }) }
