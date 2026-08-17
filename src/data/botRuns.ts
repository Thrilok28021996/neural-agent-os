import { invoke } from './invoke'

export type BotRun = {
  id: string
  workspace_id: string
  bot_id: string
  status: string
  message?: string
  meeting_id?: string
  platform: string
  reported_at: string
}

export function listBotRuns(workspaceId: string) {
  return invoke<BotRun[]>('list_bot_runs', { workspaceId })
}
