import { invoke } from '@tauri-apps/api/core'

export function askAssistant(workspaceId: string, prompt: string, model = 'qwen3:14b') {
  return invoke<{ conversation_id: string; response: string; model: string; citations: Array<{ title: string; uri?: string; chunk_id: string }> }>('ask_assistant', { workspaceId, prompt, model })
}
