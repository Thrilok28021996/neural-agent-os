import { invoke } from './invoke'

export type CostSummary = {
  total_tokens: number
  total_cost_cents: number
  by_provider: { provider: string; tokens: number; cost_cents: number }[]
  by_capability: { capability: string; tokens: number; cost_cents: number }[]
  daily_usage: { date: string; tokens: number; cost_cents: number }[]
}

export function getCostSummary(workspaceId: string) {
  return invoke<CostSummary>('get_cost_summary', { workspaceId })
}

export function setCostLimit(workspaceId: string, capability: string, limitCents: number) {
  return invoke('set_cost_limit', { workspaceId, capability, limitCents })
}

export function getCostLimit(workspaceId: string, capability: string) {
  return invoke<number | null>('get_cost_limit', { workspaceId, capability })
}
