import { invoke } from '@tauri-apps/api/core'

export type OAuthConfig = { provider: string; authorize_url: string; token_url: string; scopes: string[]; client_id: string; redirect_uri: string }
export type PKCEParams = { code_verifier: string; code_challenge: string; state: string }

export function getOAuthConfig(provider: string) { return invoke<OAuthConfig>('get_oauth_config', { provider }) }
export function generatePKCE() { return invoke<PKCEParams>('generate_pkce') }
export function buildAuthorizeUrl(provider: string, pkce: PKCEParams) { return invoke<string>('build_authorize_url', { provider, pkce }) }
export function exchangeCodeWithPKCE(provider: string, code: string, pkce: PKCEParams) { return invoke<[string, string]>('exchange_code_with_pkce', { provider, code, pkce }) }
export function refreshOAuthToken(provider: string) { return invoke<string>('refresh_oauth_token', { provider }) }
