import { invoke } from '@tauri-apps/api/core'

export type WebPage = { id: string; workspace_id: string; url: string; title: string; fetched_at: string }

export function importWebpage(workspaceId: string, url: string) {
  return invoke<WebPage>('import_webpage', { workspaceId, url })
}
