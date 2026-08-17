import { invoke } from './invoke'

export type ApprovalRequest = {
  id: string; agent_id: string; workspace_id: string; capability: string
  action_description: string; payload_json: string; status: string
  requested_at: string; resolved_at?: string; resolved_by?: string
}

export function requestApproval(agentId: string, workspaceId: string, capability: string, description: string, payload: Record<string, unknown>) {
  return invoke<ApprovalRequest>('request_approval', { agentId, workspaceId, capability, description, payloadJson: JSON.stringify(payload) })
}
export function approveRequest(requestId: string, approvedBy: string) { return invoke<ApprovalRequest>('approve_request', { requestId, approvedBy }) }
export function denyRequest(requestId: string, deniedBy: string, reason: string) { return invoke('deny_request', { requestId, deniedBy, reason }) }
export function listPendingApprovals(workspaceId: string) { return invoke<ApprovalRequest[]>('list_pending_approvals', { workspaceId }) }
