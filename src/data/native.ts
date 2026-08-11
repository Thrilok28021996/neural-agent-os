import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type { Workspace } from './workspaces'

export async function loadNativeWorkspaces(): Promise<Workspace[]> {
  return invoke<Workspace[]>('list_workspaces')
}

export async function saveNativeSetting(key: string, value: string) {
  return invoke('save_setting', { key, value })
}

export async function loadNativeSetting(key: string) {
  return invoke<string | null>('load_setting', { key })
}

export function setDataDirectory(directory: string) {
  return invoke<string>('set_data_directory', { directory })
}

/** Open native directory picker dialog */
export async function pickDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title: 'Choose Data Directory' })
  return selected as string | null
}

/** Open native file picker for audio/video imports */
export async function pickRecordingFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    title: 'Select Recording',
    filters: [{ name: 'Audio/Video', extensions: ['mp3', 'wav', 'm4a', 'mp4', 'mov', 'webm', 'ogg', 'flac'] }],
  })
  return selected as string | null
}
