import { invoke } from '@tauri-apps/api/core'

export function embedSources(workspaceId: string, model = 'nomic-embed-text') {
  return invoke<{ chunks_embedded: number; model: string }>('embed_sources', { workspaceId, model })
}
