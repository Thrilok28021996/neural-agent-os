import { invoke } from './invoke'

export type ModelRoute = { workspace_id: string; capability: string; provider: string; model: string }
export function listModelRoutes(workspaceId: string) { return invoke<ModelRoute[]>('list_model_routes', { workspaceId }) }
export function setModelRoute(workspaceId: string, capability: string, provider: string, model: string) { return invoke<ModelRoute>('set_model_route', { workspaceId, capability, provider, model }) }
/** Download a model into a local runtime's model store ("ollama" pulls into
 *  Ollama; "huggingface" downloads into the HF cache). */
export function downloadModel(runtime: 'ollama' | 'huggingface', model: string) { return invoke<string>('download_model', { runtime, model }) }
