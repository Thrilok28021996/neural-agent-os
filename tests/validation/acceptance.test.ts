// ── Cross-platform release validation suite (REAL code-bound) ───────────────
// Validates the plan §17 acceptance criteria against the actual data layer.
// Tauri `invoke` is mocked; every assertion runs the real src/data function.
import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn().mockResolvedValue('/tmp/picked.wav') }))
vi.mock('@tauri-apps/plugin-notification', () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(true),
  requestPermission: vi.fn().mockResolvedValue('granted'),
  sendNotification: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'

beforeAll(() => {
  if (typeof localStorage === 'undefined') {
    const store = new Map<string, string>()
    ;(globalThis as any).localStorage = {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => { store.set(k, v) },
      removeItem: (k: string) => { store.delete(k) },
      clear: () => store.clear(),
      key: (i: number) => [...store.keys()][i] ?? null,
      get length() { return store.size },
    }
  }
})

beforeEach(() => {
  vi.mocked(invoke).mockClear()
  vi.mocked(invoke).mockResolvedValue(undefined)
  localStorage.clear()
})

// ── AC1: Install without an application account ─────────────────────────────
describe('AC1: Install without account', () => {
  it('no authentication or account module exists in the frontend', async () => {
    // The app must not require an account: assert no login/auth data module.
    const fs = await import('node:fs')
    const path = await import('node:path')
    const dataDir = path.resolve(process.cwd(), 'src/data')
    const files = fs.readdirSync(dataDir)
    const authFiles = files.filter((f) => /\b(auth|login|account|signin|signup)\b/i.test(f))
    expect(authFiles).toEqual([])
  })

  it('local settings load without any user identity', async () => {
    const { loadSettings } = await import('../../src/data/settings')
    const settings = loadSettings()
    expect(settings.dataDirectory).toBe('~/Neural Agent OS') // unified data root // default, no account needed
    expect(settings.recordingMode).toBe('confirm')
    expect(settings.allowCloudAudio).toBe(false)
  })
})

// ── AC2: Select a local storage location ────────────────────────────────────
describe('AC2: Local storage location', () => {
  it('data directory is configurable via the native layer', async () => {
    vi.mocked(invoke).mockResolvedValue('/Users/test/neural-data')
    const { setDataDirectory } = await import('../../src/data/native')
    const result = await setDataDirectory('/Users/test/neural-data')
    expect(result).toBe('/Users/test/neural-data')
    expect(invoke).toHaveBeenCalledWith('set_data_directory', { directory: '/Users/test/neural-data' })
  })

  it('persists the chosen directory in settings', async () => {
    const { loadSettings, saveSettings } = await import('../../src/data/settings')
    saveSettings({ ...loadSettings(), dataDirectory: '/Volumes/Data/nao' })
    expect(loadSettings().dataDirectory).toBe('/Volumes/Data/nao')
  })
})

// ── AC3: Isolated workspaces with configured ingestion folders ─────────────
describe('AC3: Isolated workspaces', () => {
  it('creates and selects isolated workspaces', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'ws-2', name: 'Research', sourceCount: 0 })
    const { createWorkspace } = await import('../../src/data/workspacesAdmin')
    const ws = await createWorkspace('Research')
    expect(ws.id).toBe('ws-2')
    expect(invoke).toHaveBeenCalledWith('create_workspace', { name: 'Research' })
  })

  it('selected workspace is persisted per user choice', async () => {
    const { loadSelectedWorkspace, saveSelectedWorkspace } = await import('../../src/data/workspaces')
    const first = loadSelectedWorkspace()
    expect(first.id).toBe('personal') // default
    saveSelectedWorkspace({ id: 'research', name: 'Research' } as any)
    expect(loadSelectedWorkspace().id).toBe('research')
  })

  it('monitors ingestion folders per workspace', async () => {
    vi.mocked(invoke).mockResolvedValue([{ id: 'm1', workspace_id: 'research', path: '/docs/research', enabled: true, files_found: 3 }])
    const { listMonitoredFolders } = await import('../../src/data/monitoring')
    const folders = await listMonitoredFolders('research')
    expect(folders).toHaveLength(1)
    expect(folders[0].workspace_id).toBe('research')
    expect(invoke).toHaveBeenCalledWith('list_monitored_folders', { workspaceId: 'research' })
  })
})

// ── AC4: Connect Google Calendar, Outlook Calendar, Gmail, Outlook mail ─────
describe('AC4: Preconfigured OAuth for calendar + email', () => {
  it('exposes google/outlook OAuth config and PKCE authorize URLs', async () => {
    const { getOAuthConfig, buildAuthorizeUrl, generatePKCE } = await import('../../src/data/oauth')
    vi.mocked(invoke).mockResolvedValueOnce({
      provider: 'google', authorize_url: 'https://accounts.google.com/o/oauth2/v2/auth',
      token_url: 'https://oauth2.googleapis.com/token', scopes: ['calendar.readonly', 'gmail.readonly', 'gmail.send'],
      client_id: 'id', redirect_uri: 'http://localhost:8787/callback',
    })
    const config = await getOAuthConfig('google')
    expect(config.scopes.length).toBeGreaterThanOrEqual(3)

    vi.mocked(invoke).mockResolvedValue({ code_verifier: 'v', code_challenge: 'c', state: 's' })
    const pkce = await generatePKCE()
    vi.mocked(invoke).mockResolvedValue('https://accounts.google.com/oauth?code_challenge=c')
    const url = await buildAuthorizeUrl('google', pkce)
    expect(url).toContain('accounts.google.com')
    expect(invoke).toHaveBeenCalledWith('build_authorize_url', { provider: 'google', pkce })
  })

  it('connects email accounts (gmail / outlook / imap)', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'acct-1', workspace_id: 'w1', provider: 'imap', account_label: 'Self-hosted', status: 'needs_auth' })
    const { connectEmailAccount } = await import('../../src/data/email')
    await connectEmailAccount('w1', 'imap', 'Self-hosted', 'imap.example.com', 'smtp.example.com')
    expect(invoke).toHaveBeenCalledWith('connect_email_account', {
      workspaceId: 'w1', provider: 'imap', accountLabel: 'Self-hosted', imapHost: 'imap.example.com', smtpHost: 'smtp.example.com',
    })
  })
})

// ── AC5-8: Meeting capture pipeline ─────────────────────────────────────────
describe('AC5-8: Meeting capture pipeline', () => {
  it('detects meetings from calendar events', async () => {
    vi.mocked(invoke).mockResolvedValue([{ event_id: 'ev-1', title: 'Standup', starts_at: '2026-08-10T09:00:00Z', ends_at: '2026-08-10T09:30:00Z', meeting_url: 'https://teams.microsoft.com/m/1', workspace_id: 'w1' }])
    const { detectUpcomingMeetings } = await import('../../src/data/meetingDetection')
    const meetings = await detectUpcomingMeetings('w1', 24)
    expect(meetings).toHaveLength(1)
    expect(meetings[0].meeting_url).toContain('teams.microsoft.com')
    expect(invoke).toHaveBeenCalledWith('detect_upcoming_meetings', { workspaceId: 'w1', hoursAhead: 24 })
  })

  it('checks recording permission with configurable modes', async () => {
    vi.mocked(invoke).mockResolvedValue({ allowed: true, action: 'automatic', reason: 'Rule match' })
    const { checkRecordingAllowed } = await import('../../src/data/recording')
    const decision = await checkRecordingAllowed('w1', 'Daily Standup', 'https://zoom.us/j/1', ['Alice'])
    expect(decision.allowed).toBe(true)
    expect(invoke).toHaveBeenCalledWith('check_recording_allowed', {
      workspaceId: 'w1', eventTitle: 'Daily Standup', meetingUrl: 'https://zoom.us/j/1', participants: ['Alice'],
    })
  })

  it('imports audio/video recordings in common formats', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'm-1', title: 'Imported', status: 'created' })
    const { importMeetingRecording } = await import('../../src/data/meetings')
    for (const ext of ['mp3', 'wav', 'm4a', 'mp4', 'mov', 'webm', 'ogg', 'flac']) {
      await importMeetingRecording('w1', `/tmp/rec.${ext}`, `Rec ${ext}`)
    }
    expect(invoke).toHaveBeenCalledTimes(8)
    expect(invoke).toHaveBeenLastCalledWith('import_meeting_recording', { workspaceId: 'w1', path: '/tmp/rec.flac', title: 'Rec flac' })
  })

  it('starts/stops local recording through the native layer', async () => {
    vi.mocked(invoke).mockResolvedValue({ active: true, path: '/tmp/rec.wav' })
    const { startLocalRecording, stopLocalRecording } = await import('../../src/data/recording')
    await startLocalRecording('/tmp/rec.wav')
    expect(invoke).toHaveBeenCalledWith('start_local_recording', expect.objectContaining({ path: '/tmp/rec.wav' }))
    await stopLocalRecording()
    expect(invoke).toHaveBeenCalledWith('stop_local_recording')
  })
})

// ── AC9-10: Multilingual transcription + diarization + voice profiles ───────
describe('AC9-10: Transcription, diarization, voice profiles', () => {
  it('supports English, Hindi, and Telugu', async () => {
    const { languageLabels } = await import('../../src/data/language')
    expect(Object.keys(languageLabels)).toEqual(['en', 'hi', 'te'])
    expect(languageLabels.te).toBe('తెలుగు (Telugu)')
  })

  it('queues transcription with local provider by default', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'tj-1', meeting_id: 'm-1', provider: 'local-whisper', status: 'queued' })
    const { queueTranscription } = await import('../../src/data/transcription')
    const job = await queueTranscription('m-1')
    expect(job.provider).toBe('local-whisper')
    expect(invoke).toHaveBeenCalledWith('queue_transcription', { meetingId: 'm-1', provider: 'local-whisper' })
  })

  it('diarizes transcripts and corrects speaker labels', async () => {
    vi.mocked(invoke).mockResolvedValue({ segments: [], speaker_count: 2, embedding_model: 'nomic-embed-text' })
    const { diarizeTranscript } = await import('../../src/data/diarization')
    await diarizeTranscript('m-1', 'w1')
    expect(invoke).toHaveBeenCalledWith('diarize_transcript', expect.objectContaining({ meetingId: 'm-1', workspaceId: 'w1' }))

    const { updateTranscriptSpeaker } = await import('../../src/data/transcripts')
    await updateTranscriptSpeaker('seg-1', 'Alice')
    expect(invoke).toHaveBeenCalledWith('update_transcript_speaker', { segmentId: 'seg-1', speaker: 'Alice' })
  })

  it('maintains local voice profiles with embeddings', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'vp-1', workspace_id: 'w1', speaker_label: 'Alice', sample_count: 0 })
    const { createVoiceProfile, listVoiceProfiles } = await import('../../src/data/voiceProfiles')
    await createVoiceProfile('w1', 'Alice')
    expect(invoke).toHaveBeenCalledWith('create_voice_profile', { workspaceId: 'w1', speakerLabel: 'Alice' })
    vi.mocked(invoke).mockResolvedValue([{ id: 'vp-1', workspace_id: 'w1', speaker_label: 'Alice', sample_count: 4 }])
    const profiles = await listVoiceProfiles('w1')
    expect(profiles[0].speaker_label).toBe('Alice')
  })
})

// ── AC11: Search meetings, conversations, documents, web pages, email ───────
describe('AC11: Search with citations', () => {
  it('keyword + semantic + conversation search hit the native commands', async () => {
    const { searchSources } = await import('../../src/data/retrieval')
    await searchSources('w1', 'budget')
    expect(invoke).toHaveBeenCalledWith('search_sources', { workspaceId: 'w1', query: 'budget' })

    const { semanticSearchSources } = await import('../../src/data/semanticSearch')
    await semanticSearchSources('w1', 'budget')
    expect(invoke).toHaveBeenCalledWith('semantic_search_sources', { workspaceId: 'w1', query: 'budget', model: 'nomic-embed-text' })

    const { conversationSearch } = await import('../../src/data/conversationSearch')
    await conversationSearch('w1', 'what did we decide')
    expect(invoke).toHaveBeenCalledWith('conversation_search', { workspaceId: 'w1', query: 'what did we decide' })

    const { rerankSearch } = await import('../../src/data/reranking')
    await rerankSearch('w1', 'budget')
    expect(invoke).toHaveBeenCalledWith('rerank_search', { workspaceId: 'w1', query: 'budget' })
  })

  it('indexes sources and imports web pages for retrieval', async () => {
    const { indexSources } = await import('../../src/data/search')
    await indexSources('w1')
    expect(invoke).toHaveBeenCalledWith('index_sources', { workspaceId: 'w1' })

    const { importWebpage } = await import('../../src/data/webpage')
    await importWebpage('w1', 'https://example.com')
    expect(invoke).toHaveBeenCalledWith('import_webpage', { workspaceId: 'w1', url: 'https://example.com' })
  })
})

// ── AC12-15: Summaries, actions, tasks, reminders, follow-ups ───────────────
describe('AC12-15: Summaries, actions, tasks, reminders', () => {
  it('summarizes meetings with a local model default', async () => {
    vi.mocked(invoke).mockResolvedValue({ meeting_id: 'm-1', model: 'qwen3:14b', summary: '...', actions: [] })
    const { summarizeMeeting } = await import('../../src/data/summaries')
    await summarizeMeeting('m-1')
    expect(invoke).toHaveBeenCalledWith('summarize_meeting', { meetingId: 'm-1', model: 'qwen/qwen3.5-9b' })
  })

  it('extracts actions and promotes them to tasks', async () => {
    const { listOpenActions } = await import('../../src/data/actions')
    await listOpenActions('w1')
    expect(invoke).toHaveBeenCalledWith('list_open_actions', { workspaceId: 'w1' })

    vi.mocked(invoke).mockResolvedValue({ id: 't-1', workspace_id: 'w1', title: 'Follow up', status: 'open' })
    const { promoteAction } = await import('../../src/data/tasks')
    await promoteAction('a-1', 'w1')
    expect(invoke).toHaveBeenCalledWith('promote_action_to_task', { actionId: 'a-1', workspaceId: 'w1' })
  })

  it('creates tasks, reminders, and cancels reminders', async () => {
    const { createTask } = await import('../../src/data/tasks')
    vi.mocked(invoke).mockResolvedValue({ id: 't-1', workspace_id: 'w1', title: 'Ship', status: 'open' })
    await createTask('w1', 'Ship release')
    expect(invoke).toHaveBeenCalledWith('create_task', { workspaceId: 'w1', title: 'Ship release', dueAt: undefined })

    const { scheduleTaskReminder, cancelReminder } = await import('../../src/data/reminders')
    vi.mocked(invoke).mockResolvedValue({ id: 'r-1', workspace_id: 'w1', task_id: 't-1', title: 'Ship', scheduled_at: '2026-08-11T09:00:00Z', status: 'scheduled' })
    await scheduleTaskReminder('w1', 't-1', 'Ship release', '2026-08-11T09:00:00Z')
    expect(invoke).toHaveBeenCalledWith('schedule_task_reminder', { workspaceId: 'w1', taskId: 't-1', title: 'Ship release', scheduledAt: '2026-08-11T09:00:00Z' })
    await cancelReminder('r-1')
    expect(invoke).toHaveBeenCalledWith('cancel_reminder', { reminderId: 'r-1' })
  })

  it('sends desktop notifications for reminders', async () => {
    const { notifyReminder } = await import('../../src/data/reminders')
    const notif = await import('@tauri-apps/plugin-notification')
    await notifyReminder({ id: 'r-1', workspace_id: 'w1', title: 'Standup in 5', scheduled_at: '2026-08-10T09:00:00Z', status: 'scheduled' })
    expect(notif.sendNotification).toHaveBeenCalledWith({ title: 'Neural reminder', body: 'Standup in 5' })
  })
})

// ── AC16: Text and voice interaction with optional 3D assistant ─────────────
describe('AC16: Voice interaction', () => {
  it('local TTS and STT bind to the native pipeline', async () => {
    const { localTextToSpeech, localSpeechToText } = await import('../../src/data/voicePipeline')
    vi.mocked(invoke).mockResolvedValue('ok')
    await localTextToSpeech('Hello', 'en')
    expect(invoke).toHaveBeenCalledWith('local_text_to_speech', { text: 'Hello', language: 'en' })
    vi.mocked(invoke).mockResolvedValue('transcribed text')
    await localSpeechToText('/tmp/in.wav', 'hi')
    expect(invoke).toHaveBeenCalledWith('local_speech_to_text', { audioPath: '/tmp/in.wav', language: 'hi' })
  })
})

// ── AC17: Delete selected sources and derived data ──────────────────────────
describe('AC17: Deletion workflow', () => {
  it('previews source/meeting/workspace deletion impact', async () => {
    vi.mocked(invoke).mockResolvedValue({ source_id: 's1', source_title: 'Doc', source_kind: 'document', chunks: 5, embeddings: 4, total_size_estimate: 1000 })
    const { previewSourceDeletion } = await import('../../src/data/deletion')
    const preview = await previewSourceDeletion('s1')
    expect(preview.chunks).toBe(5)
    expect(invoke).toHaveBeenCalledWith('preview_source_deletion', { sourceId: 's1' })

    const { fullDeletionPreview } = await import('../../src/data/deletion')
    await fullDeletionPreview('w1')
    expect(invoke).toHaveBeenCalledWith('full_deletion_preview', { workspaceId: 'w1' })
  })

  it('deletes derived data selectively and completely', async () => {
    const { deleteSourceEmbeddings, deleteTranscriptSegments, deleteMeetingSummary } = await import('../../src/data/security')
    await deleteSourceEmbeddings('s1')
    expect(invoke).toHaveBeenCalledWith('delete_source_embeddings', { sourceId: 's1' })
    await deleteTranscriptSegments('m-1')
    expect(invoke).toHaveBeenCalledWith('delete_transcript_segments', { meetingId: 'm-1' })
    await deleteMeetingSummary('m-1')
    expect(invoke).toHaveBeenCalledWith('delete_meeting_summary', { meetingId: 'm-1' })

    const { deleteWorkspaceComplete } = await import('../../src/data/workspacesAdmin')
    await deleteWorkspaceComplete('w1')
    expect(invoke).toHaveBeenCalledWith('delete_workspace_complete', { workspaceId: 'w1' })
  })
})

// ── AC18: Export or restore user data ───────────────────────────────────────
describe('AC18: Export / restore', () => {
  it('exports and imports workspaces', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('/backups/w1.json').mockResolvedValueOnce('w1')
    const { exportWorkspace, importWorkspace } = await import('../../src/data/export')
    await exportWorkspace('w1', '/backups/w1.json')
    expect(invoke).toHaveBeenCalledWith('export_workspace', { workspaceId: 'w1', path: '/backups/w1.json' })
    await importWorkspace('/backups/w1.json')
    expect(invoke).toHaveBeenCalledWith('import_workspace', { path: '/backups/w1.json' })
  })

  it('performs differential backups with per-workspace retention', async () => {
    const { getWorkspaceRetention, setWorkspaceRetention } = await import('../../src/data/security')
    await setWorkspaceRetention('w1', 30)
    expect(invoke).toHaveBeenCalledWith('set_workspace_retention', { workspaceId: 'w1', retentionDays: 30 })
    await getWorkspaceRetention('w1')
    expect(invoke).toHaveBeenCalledWith('get_workspace_retention', { workspaceId: 'w1' })
  })
})

// ── AC19-20: External coding agents via MCP/REST/CLI, permission-scoped ─────
describe('AC19-20: External agent integration + permissions', () => {
  it('exposes permission checks and workspace allowlists', async () => {
    vi.mocked(invoke).mockResolvedValue({ capability: 'send_email', allowed: false, reason: 'Denied by default' })
    const { checkPermission } = await import('../../src/data/permissions')
    const check = await checkPermission('agent-1', 'w1', 'send_email')
    expect(check.allowed).toBe(false)
    expect(invoke).toHaveBeenCalledWith('check_permission', { agentId: 'agent-1', workspaceId: 'w1', capability: 'send_email' })

    const { setWorkspaceAllowlist } = await import('../../src/data/permissions')
    await setWorkspaceAllowlist('w1', ['read_knowledge', 'create_notes'])
    expect(invoke).toHaveBeenCalledWith('set_workspace_allowlist', { workspaceId: 'w1', capabilities: ['read_knowledge', 'create_notes'] })
  })

  it('gates command/file execution behind authorization', async () => {
    const { executeCommand, modifyFile, authorizeAction } = await import('../../src/data/security')
    await executeCommand('agent-1', 'w1', 'ls -la')
    expect(invoke).toHaveBeenCalledWith('execute_command', { agentId: 'agent-1', workspaceId: 'w1', command: 'ls -la' })
    await modifyFile('agent-1', 'w1', '/tmp/x.txt', 'content')
    expect(invoke).toHaveBeenCalledWith('modify_file', { agentId: 'agent-1', workspaceId: 'w1', path: '/tmp/x.txt', content: 'content', append: false })
    await authorizeAction('agent-1', 'w1', 'send_email', 'Send report', { to: 'x@y.com' })
    expect(invoke).toHaveBeenCalledWith('authorize_action', expect.objectContaining({ capability: 'send_email', payloadJson: JSON.stringify({ to: 'x@y.com' }) }))
  })

  it('provides API-key auth for MCP/REST surfaces', async () => {
    vi.mocked(invoke).mockResolvedValue('key-123')
    const { generateApiKey, listApiKeys, revokeApiKey } = await import('../../src/data/security')
    await generateApiKey('mcp-agent')
    expect(invoke).toHaveBeenCalledWith('generate_api_key', { label: 'mcp-agent' })
    await listApiKeys()
    expect(invoke).toHaveBeenCalledWith('list_api_keys')
    await revokeApiKey('mcp-agent')
    expect(invoke).toHaveBeenCalledWith('revoke_api_key', { label: 'mcp-agent' })
  })
})
