import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'

// Ensure localStorage is available
beforeAll(() => {
  if (typeof localStorage === 'undefined') {
    global.localStorage = {
      _data: {} as Record<string, string>,
      setItem(key: string, value: string) { this._data[key] = value },
      getItem(key: string) { return this._data[key] ?? null },
      removeItem(key: string) { delete this._data[key] },
      clear() { this._data = {} },
      get length() { return Object.keys(this._data).length },
      key(index: number) { return Object.keys(this._data)[index] ?? null },
    } as unknown as Storage
  }
})

// ── Workspace tests ───────────────────────────────────────────────────────

describe('Workspaces', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('loads selected workspace from localStorage', async () => {
    const { loadSelectedWorkspace, workspaceStore } = await import('../../src/data/workspaces')
    localStorage.setItem('neural-agent-os.selected-workspace', 'research')
    const ws = loadSelectedWorkspace()
    expect(ws.id).toBe('research')
    expect(ws.name).toBe('Research')
  })

  it('falls back to first workspace when none saved', async () => {
    const { loadSelectedWorkspace } = await import('../../src/data/workspaces')
    localStorage.removeItem('neural-agent-os.selected-workspace')
    const ws = loadSelectedWorkspace()
    expect(ws.id).toBe('personal')
  })

  it('creates workspace via native invoke', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue({ id: 'ws-123', name: 'Test', sourceCount: 0 })
    const { createWorkspace } = await import('../../src/data/workspacesAdmin')
    const result = await createWorkspace('Test')
    expect(result.id).toBe('ws-123')
    expect(invoke).toHaveBeenCalledWith('create_workspace', { name: 'Test' })
  })

  it('previews deletion with related records count', async () => {
    const preview = {
      workspace_id: 'personal',
      workspace_name: 'Personal',
      sources: 5,
      meetings: 3,
      transcript_segments: 42,
      tasks: 2,
      reminders: 1,
      notes: 7,
      emails: 10,
      agents: 1,
      recordings: ['/tmp/rec1.m4a'],
    }
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue(preview)
    const { previewWorkspaceDeletion } = await import('../../src/data/workspacesAdmin')
    const result = await previewWorkspaceDeletion('personal')
    expect(result.sources).toBe(5)
    expect(result.recordings).toHaveLength(1)
  })
})

// ── Settings tests ────────────────────────────────────────────────────────

describe('Settings', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
  })

  it('loads default settings when nothing saved', async () => {
    const { loadSettings } = await import('../../src/data/settings')
    const settings = loadSettings()
    expect(settings.recordingMode).toBe('confirm')
    expect(settings.recordingNotice).toBe(true)
    expect(settings.allowCloudAudio).toBe(false)
    expect(settings.allowCloudText).toBe(true)
    expect(settings.dataDirectory).toBe('~/Neural Agent OS') // unified data root
  })

  it('saves and loads settings', async () => {
    const { loadSettings, saveSettings } = await import('../../src/data/settings')
    const settings = loadSettings()
    settings.recordingMode = 'automatic'
    settings.allowCloudAudio = true
    saveSettings(settings)

    const reloaded = loadSettings()
    expect(reloaded.recordingMode).toBe('automatic')
    expect(reloaded.allowCloudAudio).toBe(true)
  })

  it('handles corrupted localStorage gracefully', async () => {
    localStorage.setItem('neural-agent-os.settings', '{broken')
    const { loadSettings } = await import('../../src/data/settings')
    const settings = loadSettings()
    expect(settings.recordingMode).toBe('confirm') // falls back to defaults
  })
})

// ── Provider tests ────────────────────────────────────────────────────────

describe('Providers', () => {
  it('reports local providers as healthy', async () => {
    const { checkProvider } = await import('../../src/data/providers')
    const healthy = await checkProvider({ model: 'qwen3:14b', provider: 'Ollama', local: true, healthy: true })
    expect(healthy).toBe(true)
  })

  it('reports cloud providers based on configuration', async () => {
    const { checkProvider } = await import('../../src/data/providers')
    // Without API key set, cloud provider with model name still reports true
    const healthy = await checkProvider({ model: 'gpt-4o', provider: 'OpenAI', local: false, healthy: true })
    expect(healthy).toBe(true)
  })

  it('has expected capability keys', async () => {
    const { providerStore } = await import('../../src/data/providers')
    expect(Object.keys(providerStore)).toContain('chat')
    expect(Object.keys(providerStore)).toContain('transcription')
    expect(Object.keys(providerStore)).toContain('embeddings')
    expect(Object.keys(providerStore)).toContain('speech')
  })
})

// ── Language tests ────────────────────────────────────────────────────────

describe('Language support', () => {
  it('supports English, Hindi, and Telugu', async () => {
    const { languageLabels, languageSpeechCodes } = await import('../../src/data/language')
    expect(languageLabels.en).toBe('English')
    expect(languageLabels.hi).toBe('हिन्दी (Hindi)')
    expect(languageLabels.te).toBe('తెలుగు (Telugu)')
    expect(languageSpeechCodes.en).toBe('en-US')
    expect(languageSpeechCodes.hi).toBe('hi-IN')
    expect(languageSpeechCodes.te).toBe('te-IN')
  })
})

// ── Model routes tests ────────────────────────────────────────────────────

describe('Model routes', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('lists model routes for a workspace', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue([
      { workspace_id: 'personal', capability: 'chat', provider: 'Ollama', model: 'qwen3:14b' },
    ])
    const { listModelRoutes } = await import('../../src/data/modelRoutes')
    const routes = await listModelRoutes('personal')
    expect(routes).toHaveLength(1)
    expect(routes[0].model).toBe('qwen3:14b')
  })

  it('sets a model route', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      workspace_id: 'personal', capability: 'chat', provider: 'OpenAI', model: 'gpt-4o',
    })
    const { setModelRoute } = await import('../../src/data/modelRoutes')
    const route = await setModelRoute('personal', 'chat', 'OpenAI', 'gpt-4o')
    expect(route.provider).toBe('OpenAI')
    expect(route.model).toBe('gpt-4o')
  })
})

// ── Cost tracking tests ───────────────────────────────────────────────────

describe('Cost tracking', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('gets cost summary', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      total_tokens: 15000,
      total_cost_cents: 5,
      by_provider: [{ provider: 'openai', tokens: 15000, cost_cents: 5 }],
      by_capability: [{ capability: 'chat', tokens: 15000, cost_cents: 5 }],
      daily_usage: [{ date: '2026-08-09', tokens: 15000, cost_cents: 5 }],
    })
    const { getCostSummary } = await import('../../src/data/costTracking')
    const summary = await getCostSummary('personal')
    expect(summary.total_tokens).toBe(15000)
    expect(summary.by_provider).toHaveLength(1)
  })

  it('sets cost limit', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined)
    const { setCostLimit } = await import('../../src/data/costTracking')
    await setCostLimit('personal', 'chat', 500)
    expect(invoke).toHaveBeenCalledWith('set_cost_limit', {
      workspaceId: 'personal', capability: 'chat', limitCents: 500,
    })
  })
})

// ── Fallback tests ────────────────────────────────────────────────────────

describe('Provider fallback', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('gets fallback chain', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      capability: 'chat',
      providers: [
        { provider: 'Ollama', model: 'qwen3:14b', priority: 0, healthy: true },
        { provider: 'OpenAI', model: 'gpt-4o', priority: 1, healthy: true },
      ],
    })
    const { getFallbackChain } = await import('../../src/data/fallback')
    const chain = await getFallbackChain('personal', 'chat')
    expect(chain.providers).toHaveLength(2)
    expect(chain.providers[0].provider).toBe('Ollama')
  })

  it('sets fallback provider', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined)
    const { setFallbackProvider } = await import('../../src/data/fallback')
    await setFallbackProvider('personal', 'chat', 'OpenAI', 'gpt-4o', 1)
    expect(invoke).toHaveBeenCalledWith('set_fallback_provider', {
      workspaceId: 'personal', capability: 'chat', provider: 'OpenAI', model: 'gpt-4o', priority: 1,
    })
  })
})

// ── Calendar participants tests ────────────────────────────────────────────

describe('Calendar participants', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('adds a participant', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: 'p-1', event_id: 'ev-1', name: 'Jane', email: 'jane@example.com', status: 'invited',
    })
    const { addParticipant } = await import('../../src/data/participants')
    const p = await addParticipant('ev-1', 'Jane', 'jane@example.com')
    expect(p.name).toBe('Jane')
    expect(p.status).toBe('invited')
  })

  it('detects conflicts', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue([{
      event_id: 'ev-1', title: 'Standup', starts_at: '...', ends_at: '...',
      overlapping_event_id: 'ev-2', overlapping_title: '1:1',
      overlapping_starts_at: '...', overlapping_ends_at: '...',
    }])
    const { detectConflicts } = await import('../../src/data/participants')
    const conflicts = await detectConflicts('personal', '2026-01-01T09:00:00Z', '2026-01-01T10:00:00Z')
    expect(conflicts).toHaveLength(1)
    expect(conflicts[0].overlapping_title).toBe('1:1')
  })
})

// ── Meeting preparation tests ─────────────────────────────────────────────

describe('Meeting preparation', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('prepares meeting briefing', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      meeting_id: 'ev-1',
      title: 'Product Review',
      starts_at: '2026-08-10T14:00:00Z',
      participants: ['Alice', 'Bob'],
      recent_related_emails: [],
      open_action_items: ['Update roadmap'],
      suggested_agenda: '# Product Review\n\n1. Check-in...',
      knowledge_preview: [],
    })
    const { prepareMeeting } = await import('../../src/data/meetingPrep')
    const prep = await prepareMeeting('ev-1')
    expect(prep.title).toBe('Product Review')
    expect(prep.participants).toContain('Alice')
    expect(prep.open_action_items).toContain('Update roadmap')
    expect(prep.suggested_agenda).toContain('Product Review')
  })
})

// ── Monitoring tests ──────────────────────────────────────────────────────

describe('Folder monitoring', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('adds monitored folder', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: 'mf-1', workspace_id: 'personal', path: '/Users/test/docs',
      enabled: true, last_checked_at: null, files_found: 0,
    })
    const { addMonitoredFolder } = await import('../../src/data/monitoring')
    const folder = await addMonitoredFolder('personal', '/Users/test/docs')
    expect(folder.path).toBe('/Users/test/docs')
    expect(folder.enabled).toBe(true)
  })
})

// ── Cloud transcription tests ─────────────────────────────────────────────

describe('Cloud transcription', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('calls cloud transcribe', async () => {
    ;(invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      job_id: 'tj-1', meeting_id: 'm-1', provider: 'openai',
      model: 'whisper-1', status: 'completed', segments: 15,
    })
    const { cloudTranscribe } = await import('../../src/data/cloudTranscription')
    const result = await cloudTranscribe('m-1', '/tmp/audio.mp3', 'openai', 'whisper-1')
    expect(result.provider).toBe('openai')
    expect(result.segments).toBe(15)
  })
})
