import { invoke } from '@tauri-apps/api/core'

export type TokenLimit = { capability: string; limit_tokens: number }

export function setTokenLimit(workspaceId: string, capability: string, limitTokens: number) {
  return invoke<void>('set_token_limit', { workspaceId, capability, limitTokens })
}

export function getTokenLimit(workspaceId: string, capability: string) {
  return invoke<number | null>('get_token_limit', { workspaceId, capability })
}

export function listTokenLimits(workspaceId: string) {
  return invoke<TokenLimit[]>('list_token_limits', { workspaceId })
}
