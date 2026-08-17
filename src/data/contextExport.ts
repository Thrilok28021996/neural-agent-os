import { invoke } from './invoke'

/** Export a markdown bundle of a workspace's context for coding agents. */
export function exportContext(workspaceId: string) {
  return invoke<string>('export_context', { workspaceId })
}
