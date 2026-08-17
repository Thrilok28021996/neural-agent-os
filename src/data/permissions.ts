import { invoke } from './invoke'

export type PermissionCheck = { capability: string; allowed: boolean; reason: string }
export type AgentPermission = { agent_id?: string; capability: string; granted: boolean; workspace_id: string }

export function checkPermission(agentId: string, workspaceId: string, capability: string) {
  return invoke<PermissionCheck>('check_permission', { agentId, workspaceId, capability })
}
export function checkAllPermissions(agentId: string, workspaceId: string) {
  return invoke<PermissionCheck[]>('check_all_permissions', { agentId, workspaceId })
}
export function grantPermission(agentId: string, workspaceId: string, capability: string) {
  return invoke('grant_permission', { agentId, workspaceId, capability })
}
export function revokePermission(agentId: string, workspaceId: string, capability: string) {
  return invoke('revoke_permission', { agentId, workspaceId, capability })
}
export function setWorkspaceAllowlist(workspaceId: string, capabilities: string[]) {
  return invoke('set_workspace_allowlist', { workspaceId, capabilities })
}
