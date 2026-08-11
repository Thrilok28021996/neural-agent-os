export type Workspace = { id: string; name: string; sourceCount: number }

export const workspaceStore: Workspace[] = [
  { id: 'personal', name: 'Personal', sourceCount: 8 },
  { id: 'neural-os', name: 'Neural Agent OS', sourceCount: 12 },
  { id: 'research', name: 'Research', sourceCount: 5 },
]

const selectedWorkspaceKey = 'neural-agent-os.selected-workspace'

export function loadSelectedWorkspace(): Workspace {
  if (typeof window === 'undefined') return workspaceStore[0]
  const savedId = window.localStorage.getItem(selectedWorkspaceKey)
  return workspaceStore.find((workspace) => workspace.id === savedId) ?? workspaceStore[0]
}

export function saveSelectedWorkspace(workspace: Workspace) {
  window.localStorage.setItem(selectedWorkspaceKey, workspace.id)
}
