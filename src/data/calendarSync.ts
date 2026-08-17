import { invoke } from './invoke'

export type OAuthUrl = { url: string; state: string }

export function generateOAuthUrl(provider: 'google' | 'outlook') {
  return invoke<OAuthUrl>('generate_oauth_url', { provider })
}

export function exchangeOAuthCode(provider: 'google' | 'outlook', code: string) {
  return invoke<string>('exchange_oauth_code', { provider, code })
}

export function syncCalendarEvents(connectionId: string) {
  return invoke<string>('sync_calendar_events', { connectionId })
}
