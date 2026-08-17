import { invoke } from './invoke'

export function embedSources(workspaceId: string, model = 'nomic-embed-text') {
  return invoke<{ chunks_embedded: number; model: string }>('embed_sources', { workspaceId, model })
}
