import { invoke } from '@tauri-apps/api/core'

export type ModelRoute = { workspace_id: string; capability: string; provider: string; model: string }
export function listModelRoutes(workspaceId: string) { return invoke<ModelRoute[]>('list_model_routes', { workspaceId }) }
export function setModelRoute(workspaceId: string, capability: string, provider: string, model: string) { return invoke<ModelRoute>('set_model_route', { workspaceId, capability, provider, model }) }
