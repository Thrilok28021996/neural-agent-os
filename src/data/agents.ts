import { invoke } from './invoke'
import { DEFAULT_LMSTUDIO_MODEL } from './providers'

export type Agent = { id: string; workspace_id: string; name: string; prompt: string; schedule: string; permission_mode: string; status: string; last_run_at?: string }
export type AgentRun = { id: string; agent_id: string; status: string; output?: string; error?: string; retry_count: number }
export function createAgent(workspaceId: string, name: string, prompt: string, schedule: string, permissionMode: 'approval_required' | 'read_only' | 'full' = 'approval_required') { return invoke<Agent>('create_agent', { workspaceId, name, prompt, schedule, permissionMode }) }
/** Edit an existing agent's name, prompt, schedule, and permission mode. The agent status is preserved. */
export function updateAgent(agentId: string, name: string, prompt: string, schedule: string, permissionMode: 'approval_required' | 'read_only' | 'full') { return invoke<Agent>('update_agent', { agentId, name, prompt, schedule, permissionMode }) }
export function listAgents(workspaceId: string) { return invoke<Agent[]>('list_agents', { workspaceId }) }
export function updateAgentStatus(agentId: string, status: string) { return invoke('update_agent_status', { agentId, status }) }
export function runAgentNow(agentId: string, model = DEFAULT_LMSTUDIO_MODEL) { return invoke<AgentRun>('run_agent_now', { agentId, model }) }
export function listAgentRuns(agentId: string) { return invoke<AgentRun[]>('list_agent_runs', { agentId }) }
