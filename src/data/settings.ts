export type LocalSettings = {
  dataDirectory: string
  recordingMode: 'automatic' | 'confirm' | 'disabled'
  recordingNotice: boolean
  allowCloudAudio: boolean
  allowCloudText: boolean
  localOnlyMode: boolean
}

const settingsKey = 'neural-agent-os.settings'
const defaults: LocalSettings = {
  dataDirectory: 'Choose a local folder',
  recordingMode: 'confirm',
  recordingNotice: true,
  allowCloudAudio: false,
  allowCloudText: true,
  localOnlyMode: false,
}

export function loadSettings(): LocalSettings {
  if (typeof window === 'undefined') return defaults
  try {
    return { ...defaults, ...JSON.parse(window.localStorage.getItem(settingsKey) ?? '{}') }
  } catch {
    return defaults
  }
}

export function saveSettings(settings: LocalSettings) {
  window.localStorage.setItem(settingsKey, JSON.stringify(settings))
}
