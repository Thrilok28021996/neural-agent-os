import { invoke } from './invoke'

export type AgentActivitySummary = {
  active_agents: number
  pending_approvals: number
  recent_runs: { run_id: string; agent_name: string; status: string; output?: string; error?: string; started_at: string }[]
  next_scheduled: { agent_id: string; name: string; schedule: string; permission_mode: string }[]
}

export function getAgentActivity(workspaceId: string) {
  return invoke<AgentActivitySummary>('get_agent_activity', { workspaceId })
}
