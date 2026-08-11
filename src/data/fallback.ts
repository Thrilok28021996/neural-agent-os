import { invoke } from '@tauri-apps/api/core'

export type FallbackChain = {
  capability: string
  providers: { provider: string; model: string; priority: number; healthy: boolean }[]
}

export function getFallbackChain(workspaceId: string, capability: string) {
  return invoke<FallbackChain>('get_fallback_chain', { workspaceId, capability })
}

export function setFallbackProvider(workspaceId: string, capability: string, provider: string, model: string, priority: number) {
  return invoke('set_fallback_provider', { workspaceId, capability, provider, model, priority })
}

export function clearFallbackChain(workspaceId: string, capability: string) {
  return invoke('clear_fallback_chain', { workspaceId, capability })
}
