export type ModelCapability = 'chat' | 'transcription' | 'embeddings' | 'speech'
export type ProviderStatus = { model: string; local: boolean; provider: string; healthy: boolean }

export const providerStore: Record<ModelCapability, ProviderStatus> = {
  chat: { model: 'qwen3:14b', provider: 'Ollama', local: true, healthy: true },
  transcription: { model: 'whisper-large-v3', provider: 'Local runtime', local: true, healthy: true },
  embeddings: { model: 'nomic-embed-text', provider: 'Ollama', local: true, healthy: true },
  speech: { model: 'OpenAI TTS', provider: 'OpenAI', local: false, healthy: true },
}

export async function checkProvider(status: ProviderStatus): Promise<boolean> {
  if (status.local) return true
  return Boolean(status.provider && status.model)
}

// Plan §10: supported provider runtimes (local OpenAI-compatible runtimes and
// cloud providers). Local runtimes are reachable via model_routes and are
// never treated as cloud for cost/audit purposes.
export type ProviderOption = {
  id: string
  label: string
  kind: 'local' | 'cloud'
  baseUrlEnv?: string
  note: string
}

export const providerOptions: ProviderOption[] = [
  { id: 'ollama', label: 'Ollama', kind: 'local', baseUrlEnv: 'NEURAL_OLLAMA_URL', note: 'Local models via Ollama' },
  { id: 'local', label: 'Local', kind: 'local', note: 'Local runtime (Whisper, TTS, etc.)' },
  { id: 'lmstudio', label: 'LM Studio', kind: 'local', baseUrlEnv: 'NEURAL_OPENAI_COMPATIBLE_URL', note: 'Local OpenAI-compatible server' },
  { id: 'llamacpp', label: 'llama.cpp', kind: 'local', baseUrlEnv: 'NEURAL_OPENAI_COMPATIBLE_URL', note: 'Local OpenAI-compatible server' },
  { id: 'vllm', label: 'vLLM', kind: 'local', baseUrlEnv: 'NEURAL_OPENAI_COMPATIBLE_URL', note: 'Local OpenAI-compatible endpoint' },
  { id: 'opencode', label: 'OpenCode Go plan', kind: 'cloud', note: 'OpenCode Go plan (cloud)' },
  { id: 'openai_compatible', label: 'OpenAI-compatible endpoint', kind: 'local', baseUrlEnv: 'NEURAL_OPENAI_COMPATIBLE_URL', note: 'Any OpenAI-compatible /api/v1 endpoint' },
  { id: 'openai', label: 'OpenAI', kind: 'cloud', note: 'Cloud chat/embeddings (audited)' },
  { id: 'anthropic', label: 'Anthropic', kind: 'cloud', note: 'Cloud chat (audited)' },
  { id: 'google', label: 'Google', kind: 'cloud', note: 'Cloud chat/transcription (audited)' },
]

export function isCloudProviderOption(providerId: string): boolean {
  return providerOptions.find((p) => p.id === providerId)?.kind === 'cloud'
}

/** Map a provider id (or legacy label) to its display label. */
export function providerLabel(providerId: string): string {
  const id = providerId.toLowerCase().replace(/\s+/g, '_').replace(/[^a-z0-9_]/g, '')
  const option = providerOptions.find((p) => p.id === id)
  if (option) return option.label
  const byLabel = providerOptions.find((p) => p.label.toLowerCase() === providerId.toLowerCase())
  return byLabel?.label ?? providerId
}
