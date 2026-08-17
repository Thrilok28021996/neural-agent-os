import { invoke } from './invoke'

// ── Agent-controlled command/file execution (permission + approval gated) ──

export type CommandResult = { command: string; exit_code: number; stdout: string; stderr: string }
export type FileEditResult = { path: string; action: string; bytes_written: number }

export function executeCommand(agentId: string, workspaceId: string, command: string) {
  return invoke<CommandResult>('execute_command', { agentId, workspaceId, command })
}
export function modifyFile(agentId: string, workspaceId: string, path: string, content: string, append = false) {
  return invoke<FileEditResult>('modify_file', { agentId, workspaceId, path, content, append })
}

// ── Centralized action authorization ───────────────────────────────────────

export function authorizeAction(agentId: string, workspaceId: string, capability: string, description: string, payload: Record<string, unknown>) {
  return invoke<{ id: string; status: string } | null>('authorize_action', { agentId, workspaceId, capability, description, payloadJson: JSON.stringify(payload) })
}

// ── API key authentication for MCP/REST ────────────────────────────────────

export function generateApiKey(label: string) { return invoke<string>('generate_api_key', { label }) }
export function listApiKeys() { return invoke<string[]>('list_api_keys') }
export function revokeApiKey(label: string) { return invoke('revoke_api_key', { label }) }

// ── Selective derived-data deletion ────────────────────────────────────────

export function deleteSourceEmbeddings(sourceId: string) { return invoke<number>('delete_source_embeddings', { sourceId }) }
export function deleteTranscriptSegments(meetingId: string) { return invoke<number>('delete_transcript_segments', { meetingId }) }
export function deleteMeetingSummary(meetingId: string) { return invoke('delete_meeting_summary', { meetingId }) }
export function deleteMeetingActions(meetingId: string) { return invoke<number>('delete_meeting_actions', { meetingId }) }
export function deleteMeetingRecording(meetingId: string) { return invoke('delete_meeting_recording', { meetingId }) }

// ── Per-workspace recording retention ──────────────────────────────────────

export function getWorkspaceRetention(workspaceId: string) { return invoke<number | null>('get_workspace_retention', { workspaceId }) }
export function setWorkspaceRetention(workspaceId: string, retentionDays: number) { return invoke('set_workspace_retention', { workspaceId, retentionDays }) }
