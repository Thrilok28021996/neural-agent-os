import { Channel } from '@tauri-apps/api/core'
import { invoke } from './invoke'
import { DEFAULT_LMSTUDIO_MODEL } from './providers'

export function askAssistant(workspaceId: string, prompt: string, model = DEFAULT_LMSTUDIO_MODEL) {
  return invoke<{ conversation_id: string; response: string; model: string; citations: Array<{ title: string; uri?: string; chunk_id: string }> }>('ask_assistant', { workspaceId, prompt, model })
}

/**
 * Streaming voice reply: LM Studio tokens are delivered to `onToken` as they
 * are generated (via a Tauri IPC channel), so the avatar can start speaking
 * before the model finishes writing.
 */
export function streamVoiceReply(prompt: string, model: string, onToken: (token: string) => void) {
  const channel = new Channel<string>()
  channel.onmessage = onToken
  return invoke<void>('stream_voice_reply', { prompt, model, channel })
}
