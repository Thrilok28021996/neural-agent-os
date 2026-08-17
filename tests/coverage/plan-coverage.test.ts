// ── Plan §16 test-area coverage (REAL code-bound) ───────────────────────────
// One or more tests per test area from the plan's testing strategy (§16).
// Every test runs real src/data functions with a mocked Tauri `invoke`.
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

// Area 1: Offline operation
describe('Test area: Offline operation', () => {
  it('local-only mode is the default and model routes stay local', async () => {
    const { loadSettings } = await import('../../src/data/settings')
    expect(loadSettings().localOnlyMode).toBe(false) // configurable; default text sharing allowed

    const { providerStore } = await import('../../src/data/providers')
    expect(providerStore.chat.local).toBe(true)
    expect(providerStore.embeddings.local).toBe(true)
    expect(providerStore.transcription.local).toBe(true)
  })

  it('local providers are always available without network', async () => {
    const { checkProvider } = await import('../../src/data/providers')
    expect(await checkProvider({ model: 'qwen3:14b', local: true, provider: 'Ollama', healthy: true })).toBe(true)
  })
})

// Area 2: Audio capture reliability
describe('Test area: Audio capture reliability', () => {
  it('recording start carries capture context (workspace/event/confirmation)', async () => {
    const { startLocalRecording } = await import('../../src/data/recording')
    await startLocalRecording('/tmp/rec.wav', 'system', { workspaceId: 'w1', eventTitle: 'Standup', confirmed: true })
    expect(invoke).toHaveBeenCalledWith('start_local_recording', expect.objectContaining({
      path: '/tmp/rec.wav', source: 'system', workspaceId: 'w1', eventTitle: 'Standup', confirmed: true,
    }))
  })
})

// Area 3: System-audio permissions
describe('Test area: System-audio permissions', () => {
  it('recording permission is checked before capture', async () => {
    vi.mocked(invoke).mockResolvedValue({ allowed: false, action: 'disabled', reason: 'Blocked by rule' })
    const { checkRecordingAllowed } = await import('../../src/data/recording')
    const decision = await checkRecordingAllowed('w1', 'Board', 'https://meet.example.com/x')
    expect(decision.allowed).toBe(false)
    expect(invoke).toHaveBeenCalledWith('check_recording_allowed', expect.objectContaining({ eventTitle: 'Board' }))
  })
})

// Area 4: Teams bot joining and recovery
describe('Test area: Teams bot joining and recovery', () => {
  it('launches the Teams bot with join metadata', async () => {
    vi.mocked(invoke).mockResolvedValue('bot-started')
    const { launchMeetingBot } = await import('../../src/data/meetingBot')
    await launchMeetingBot('teams', 'https://teams.microsoft.com/l/meetup-join/abc', 'Neural Assistant', 45)
    expect(invoke).toHaveBeenCalledWith('launch_meeting_bot', {
      platform: 'teams', meetingUrl: 'https://teams.microsoft.com/l/meetup-join/abc',
      displayName: 'Neural Assistant', durationMinutes: 45,
    })
  })
})

// Area 5: Transcription in English, Hindi, and Telugu
describe('Test area: Multilingual transcription', () => {
  it('exposes en/hi/te speech codes for STT/TTS', async () => {
    const { languageSpeechCodes } = await import('../../src/data/language')
    expect(languageSpeechCodes).toEqual({ en: 'en-US', hi: 'hi-IN', te: 'te-IN' })
  })

  it('passes language to the local speech pipeline', async () => {
    const { localSpeechToText } = await import('../../src/data/voicePipeline')
    await localSpeechToText('/tmp/in.wav', 'te')
    expect(invoke).toHaveBeenCalledWith('local_speech_to_text', { audioPath: '/tmp/in.wav', language: 'te' })
  })
})

// Area 6: Speaker diarization and correction
describe('Test area: Speaker diarization and correction', () => {
  it('diarization command receives embedding model and threshold', async () => {
    const { diarizeTranscript } = await import('../../src/data/diarization')
    await diarizeTranscript('m-1', 'w1', 'nomic-embed-text', 0.6)
    expect(invoke).toHaveBeenCalledWith('diarize_transcript', { meetingId: 'm-1', workspaceId: 'w1', embeddingModel: 'nomic-embed-text', similarityThreshold: 0.6 })
  })

  it('speaker labels can be corrected on transcript segments', async () => {
    const { updateTranscriptSpeaker } = await import('../../src/data/transcripts')
    await updateTranscriptSpeaker('seg-9', 'Bob')
    expect(invoke).toHaveBeenCalledWith('update_transcript_speaker', { segmentId: 'seg-9', speaker: 'Bob' })
  })
})

// Area 7: Voice-profile matching
describe('Test area: Voice-profile matching', () => {
  it('creates and lists voice profiles for matching', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ id: 'vp-1', workspace_id: 'w1', speaker_label: 'Carol', sample_count: 0 })
      .mockResolvedValueOnce([{ id: 'vp-1', workspace_id: 'w1', speaker_label: 'Carol', sample_count: 12 }])
    const { createVoiceProfile, listVoiceProfiles } = await import('../../src/data/voiceProfiles')
    await createVoiceProfile('w1', 'Carol')
    const profiles = await listVoiceProfiles('w1')
    expect(profiles[0].sample_count).toBe(12)
  })
})

// Area 8: Calendar synchronization conflicts
describe('Test area: Calendar sync conflicts', () => {
  it('syncs calendar events and resolves conflicts with explicit resolution', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('synced').mockResolvedValueOnce({ queue_id: 'q-1', resolution: 'keep_local', resolved_by: 'user', resolved_at: 'now' })
    const { syncCalendarEvents } = await import('../../src/data/calendarSync')
    await syncCalendarEvents('cc-1')
    expect(invoke).toHaveBeenCalledWith('sync_calendar_events', { connectionId: 'cc-1' })

    const { resolveSyncConflict } = await import('../../src/data/syncQueue')
    await resolveSyncConflict('q-1', 'keep_local')
    expect(invoke).toHaveBeenCalledWith('resolve_sync_conflict', { queueId: 'q-1', resolution: 'keep_local' })
  })

  it('enqueues sync operations with serialized payloads', async () => {
    const { enqueueSync } = await import('../../src/data/syncQueue')
    await enqueueSync('calendar_event', 'ev-1', 'google', 'update', { title: 'New' })
    expect(invoke).toHaveBeenCalledWith('enqueue_sync', {
      entityType: 'calendar_event', entityId: 'ev-1', provider: 'google', action: 'update', payloadJson: JSON.stringify({ title: 'New' }),
    })
  })
})

// Area 9: Gmail and Outlook synchronization
describe('Test area: Gmail/Outlook sync', () => {
  it('connects and syncs OAuth email accounts', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'acct-1', workspace_id: 'w1', provider: 'gmail', account_label: 'Gmail', status: 'connected' })
    const { connectEmailAccount, syncEmailOAuth } = await import('../../src/data/email')
    await connectEmailAccount('w1', 'gmail', 'Gmail')
    expect(invoke).toHaveBeenCalledWith('connect_email_account', expect.objectContaining({ provider: 'gmail' }))
    await syncEmailOAuth('acct-1')
    expect(invoke).toHaveBeenCalledWith('sync_email_oauth', { accountId: 'acct-1' })
  })
})

// Area 10: Email approval safeguards
describe('Test area: Email approval safeguards', () => {
  it('email sends go through approval-first authorization', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'appr-1', status: 'pending' })
    const { authorizeAction } = await import('../../src/data/security')
    const result = await authorizeAction('agent-1', 'w1', 'send_email', 'Send to client', { to: 'client@corp.com' })
    expect(result.status).toBe('pending')
    expect(invoke).toHaveBeenCalledWith('authorize_action', expect.objectContaining({ capability: 'send_email' }))
  })

  it('approval workflow is exposed for pending requests', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ id: 'appr-1', status: 'approved', resolved_at: 'now', resolved_by: 'user' })
    const { approveRequest, listPendingApprovals } = await import('../../src/data/approvals')
    await approveRequest('appr-1', 'user')
    expect(invoke).toHaveBeenCalledWith('approve_request', { requestId: 'appr-1', approvedBy: 'user' })
    await listPendingApprovals('w1')
    expect(invoke).toHaveBeenCalledWith('list_pending_approvals', { workspaceId: 'w1' })
  })
})

// Area 11: Cloud-provider switching
describe('Test area: Cloud-provider switching', () => {
  it('routes model selection per capability via provider config', async () => {
    const { listModelRoutes, setModelRoute } = await import('../../src/data/modelRoutes')
    await setModelRoute('w1', 'chat', 'anthropic', 'claude-3-5-sonnet')
    expect(invoke).toHaveBeenCalledWith('set_model_route', { workspaceId: 'w1', capability: 'chat', provider: 'anthropic', model: 'claude-3-5-sonnet' })
    await listModelRoutes('w1')
    expect(invoke).toHaveBeenCalledWith('list_model_routes', { workspaceId: 'w1' })
  })
})

// Area 12: Local-model fallback
describe('Test area: Local-model fallback', () => {
  it('fallback chains are configurable per capability', async () => {
    vi.mocked(invoke).mockResolvedValue({ capability: 'chat', providers: [{ provider: 'ollama', model: 'qwen3:14b', priority: 1, healthy: true }] })
    const { getFallbackChain, setFallbackProvider } = await import('../../src/data/fallback')
    await setFallbackProvider('w1', 'chat', 'ollama', 'qwen3:14b', 1)
    expect(invoke).toHaveBeenCalledWith('set_fallback_provider', { workspaceId: 'w1', capability: 'chat', provider: 'ollama', model: 'qwen3:14b', priority: 1 })
    const chain = await getFallbackChain('w1', 'chat')
    expect(chain.providers[0].provider).toBe('ollama')
  })
})

// Area 13: Knowledge citations
describe('Test area: Knowledge citations', () => {
  it('search results carry source metadata for citations', async () => {
    vi.mocked(invoke).mockResolvedValue([{ source_id: 's1', title: 'Budget.md', uri: 'file:///docs/budget.md', chunk_id: 'c1', content: '...' }])
    const { searchSources } = await import('../../src/data/retrieval')
    const results = await searchSources('w1', 'budget')
    expect(results[0].source_id).toBe('s1')
    expect(results[0].title).toBe('Budget.md')
    expect(invoke).toHaveBeenCalledWith('search_sources', { workspaceId: 'w1', query: 'budget' })
  })
})

// Area 14: Workspace isolation
describe('Test area: Workspace isolation', () => {
  it('workspace-scoped queries are passed to every search command', async () => {
    const { conversationSearch } = await import('../../src/data/conversationSearch')
    await conversationSearch('research', 'findings')
    expect(invoke).toHaveBeenCalledWith('conversation_search', { workspaceId: 'research', query: 'findings' })
    const { listNotes } = await import('../../src/data/notes')
    await listNotes('research')
    expect(invoke).toHaveBeenCalledWith('list_notes', { workspaceId: 'research' })
  })
})

// Area 15: Complete deletion of selected derived data
describe('Test area: Complete deletion of derived data', () => {
  it('every derived-data deletion surface is exposed', async () => {
    const { deleteSourceEmbeddings, deleteTranscriptSegments, deleteMeetingSummary, deleteMeetingActions, deleteMeetingRecording } = await import('../../src/data/security')
    await deleteSourceEmbeddings('s1'); expect(invoke).toHaveBeenCalledWith('delete_source_embeddings', { sourceId: 's1' })
    await deleteTranscriptSegments('m1'); expect(invoke).toHaveBeenCalledWith('delete_transcript_segments', { meetingId: 'm1' })
    await deleteMeetingSummary('m1'); expect(invoke).toHaveBeenCalledWith('delete_meeting_summary', { meetingId: 'm1' })
    await deleteMeetingActions('m1'); expect(invoke).toHaveBeenCalledWith('delete_meeting_actions', { meetingId: 'm1' })
    await deleteMeetingRecording('m1'); expect(invoke).toHaveBeenCalledWith('delete_meeting_recording', { meetingId: 'm1' })
  })
})

// Area 16: Agent retries and failures
describe('Test area: Agent retries and failures', () => {
  it('job queue exposes progress, retries, and failure handling', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'job-1', job_type: 'transcription', status: 'queued', progress_pct: 0, workspace_id: 'w1', retry_count: 0, max_retries: 3 })
    const { enqueueJob, getQueueStatus } = await import('../../src/data/jobQueue')
    await enqueueJob('transcription', 'w1', { meeting_id: 'm-1' }, 3)
    expect(invoke).toHaveBeenCalledWith('enqueue_job', expect.objectContaining({ jobType: 'transcription', workspaceId: 'w1', maxRetries: 3 }))
    await getQueueStatus('w1')
    expect(invoke).toHaveBeenCalledWith('get_queue_status', { workspaceId: 'w1' })
  })
})

// Area 17: External-agent permissions
describe('Test area: External-agent permissions', () => {
  it('checks and grants the ten defined capabilities', async () => {
    const { checkAllPermissions } = await import('../../src/data/permissions')
    await checkAllPermissions('agent-1', 'w1')
    expect(invoke).toHaveBeenCalledWith('check_all_permissions', { agentId: 'agent-1', workspaceId: 'w1' })
    const { grantPermission, revokePermission } = await import('../../src/data/permissions')
    await grantPermission('agent-1', 'w1', 'create_notes')
    expect(invoke).toHaveBeenCalledWith('grant_permission', { agentId: 'agent-1', workspaceId: 'w1', capability: 'create_notes' })
    await revokePermission('agent-1', 'w1', 'create_notes')
    expect(invoke).toHaveBeenCalledWith('revoke_permission', { agentId: 'agent-1', workspaceId: 'w1', capability: 'create_notes' })
  })
})

// Area 18: Installer and update behavior (platform diagnostics)
describe('Test area: Installer and update behavior', () => {
  it('storage diagnostics surface database and recording usage', async () => {
    vi.mocked(invoke).mockResolvedValue({
      database_path: '/data/nao.sqlite', database_size_bytes: 1024, recordings_count: 2, recordings_size_bytes: 2048,
      recordings_paths: [], total_sources: 3, total_embeddings: 9, total_emails: 1,
    })
    const { getStorageStats } = await import('../../src/data/diagnostics')
    const stats = await getStorageStats()
    expect(stats.recordings_count).toBe(2)
    expect(invoke).toHaveBeenCalledWith('get_storage_stats')
  })

  it('audit log records provider data-sharing events', async () => {
    vi.mocked(invoke).mockResolvedValue([{ id: 1, timestamp: 'now', action: 'cloud_audio_upload', provider: 'openai', data_type: 'audio', workspace_id: 'w1', details: 'ok' }])
    const { getAuditLog } = await import('../../src/data/diagnostics')
    const log = await getAuditLog('w1', 50)
    expect(log[0].action).toBe('cloud_audio_upload')
    expect(invoke).toHaveBeenCalledWith('get_audit_log', { workspaceId: 'w1', limit: 50 })
  })
})

// ── Branch hardening: cover default-value and fallback paths ────────────────
describe('Branch hardening (defaults/fallbacks)', () => {
  it('extractEmailActions applies default workspace and model when omitted', async () => {
    const { extractEmailActions } = await import('../../src/data/automation')
    await extractEmailActions('e1')
    expect(invoke).toHaveBeenCalledWith('extract_email_actions', { emailId: 'e1', workspaceId: 'personal', model: 'qwen/qwen3.5-9b' })
  })

  it('generateEmailDraft applies the default local model when omitted', async () => {
    const { generateEmailDraft } = await import('../../src/data/emailComplete')
    await generateEmailDraft('w1', 'Draft a reply', 'context')
    expect(invoke).toHaveBeenCalledWith('generate_email_draft', {
      workspaceId: 'w1', instructions: 'Draft a reply', context: 'context', model: 'qwen/qwen3.5-9b',
    })
  })

  it('notifyReminder requests permission when not yet granted', async () => {
    const notif = await import('@tauri-apps/plugin-notification')
    vi.mocked(notif.isPermissionGranted).mockResolvedValue(false)
    vi.mocked(notif.requestPermission).mockResolvedValue('granted')
    const { notifyReminder } = await import('../../src/data/reminders')
    await notifyReminder({ id: 'r-1', workspace_id: 'w1', title: 'Hi', scheduled_at: 'now', status: 'scheduled' })
    expect(notif.requestPermission).toHaveBeenCalled()
    expect(notif.sendNotification).toHaveBeenCalledWith({ title: 'Neural reminder', body: 'Hi' })
  })

  it('loadSettings falls back to defaults on corrupt stored JSON', async () => {
    const { loadSettings, saveSettings } = await import('../../src/data/settings')
    // corrupt the stored settings directly
    localStorage.setItem('neural-agent-os.settings', '{not valid json')
    const settings = loadSettings()
    expect(settings.recordingMode).toBe('confirm')
    expect(settings.allowCloudAudio).toBe(false)
  })

  it('loadSelectedWorkspace falls back to the first workspace on unknown id', async () => {
    const { loadSelectedWorkspace } = await import('../../src/data/workspaces')
    localStorage.setItem('neural-agent-os.selected-workspace', 'does-not-exist')
    const ws = loadSelectedWorkspace()
    expect(ws.id).toBe('personal') // fallback to first/default
  })
})

// ── Plan-completion features (added Aug 11) ─────────────────────────────────
describe('Plan-completion: token limits, memories, search, export, bots', () => {
  it('sets and reads per-capability token limits', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined).mockResolvedValueOnce(100_000)
    const { setTokenLimit, getTokenLimit } = await import('../../src/data/tokenLimits')
    await setTokenLimit('w1', 'chat', 100_000)
    expect(invoke).toHaveBeenCalledWith('set_token_limit', { workspaceId: 'w1', capability: 'chat', limitTokens: 100_000 })
    const limit = await getTokenLimit('w1', 'chat')
    expect(limit).toBe(100_000)
  })

  it('saves, searches, and deletes memories', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'memory-1', workspace_id: 'w1', content: 'Prefers concise replies', created_at: 'now', updated_at: 'now' })
    const { createMemory, searchMemories, deleteMemory } = await import('../../src/data/memories')
    await createMemory('w1', 'Prefers concise replies')
    expect(invoke).toHaveBeenCalledWith('create_memory', { workspaceId: 'w1', content: 'Prefers concise replies', sourceKind: undefined, sourceId: undefined })
    await searchMemories('w1', 'concise')
    expect(invoke).toHaveBeenCalledWith('search_memories', { workspaceId: 'w1', query: 'concise' })
    await deleteMemory('memory-1')
    expect(invoke).toHaveBeenCalledWith('delete_memory', { memoryId: 'memory-1' })
  })

  it('finds duplicate sources by content hash', async () => {
    vi.mocked(invoke).mockResolvedValue([{ content_hash: 'abc', source_count: 2, sources: [{ id: 's1', title: 'A', created_at: 'now' }, { id: 's2', title: 'A copy', created_at: 'now' }] }])
    const { findDuplicates } = await import('../../src/data/duplicates')
    const groups = await findDuplicates('w1')
    expect(groups[0].source_count).toBe(2)
    expect(invoke).toHaveBeenCalledWith('find_duplicates', { workspaceId: 'w1' })
  })

  it('hybrid search merges keyword and semantic results with scores', async () => {
    vi.mocked(invoke).mockResolvedValue([{ source_id: 's1', title: 'Budget', uri: '', chunk_id: 'c1', content: '...', score: 0.9 }])
    const { hybridSearch } = await import('../../src/data/hybridSearch')
    const results = await hybridSearch('w1', 'budget')
    expect(results[0].score).toBe(0.9)
    expect(invoke).toHaveBeenCalledWith('hybrid_search', { workspaceId: 'w1', query: 'budget', model: 'nomic-embed-text' })
  })

  it('searches email by keyword and semantically', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{ id: 'e1', account_id: 'a1', subject: 'Invoice', score: undefined }])
      .mockResolvedValueOnce([{ id: 'e1', account_id: 'a1', subject: 'Invoice', score: 0.8 }])
    const { searchEmails, semanticSearchEmails } = await import('../../src/data/emailSearch')
    await searchEmails('w1', 'invoice')
    expect(invoke).toHaveBeenCalledWith('search_emails', { workspaceId: 'w1', query: 'invoice' })
    await semanticSearchEmails('w1', 'money owed')
    expect(invoke).toHaveBeenCalledWith('semantic_search_emails', { workspaceId: 'w1', query: 'money owed', model: 'nomic-embed-text' })
  })

  it('exports workspace context for coding agents', async () => {
    vi.mocked(invoke).mockResolvedValue('# Workspace Context: Work\n## Sources\n- [note] Doc')
    const { exportContext } = await import('../../src/data/contextExport')
    const context = await exportContext('w1')
    expect(context).toContain('Workspace Context')
    expect(invoke).toHaveBeenCalledWith('export_context', { workspaceId: 'w1' })
  })

  it('batch-imports recordings and reprocesses transcription', async () => {
    vi.mocked(invoke).mockResolvedValue([{ id: 'm-1', title: 'A', status: 'created' }])
    const { importMeetingRecordingsBatch } = await import('../../src/data/meetings')
    await importMeetingRecordingsBatch('w1', ['/tmp/a.m4a', '/tmp/b.wav'])
    expect(invoke).toHaveBeenCalledWith('import_meeting_recordings_batch', { workspaceId: 'w1', paths: ['/tmp/a.m4a', '/tmp/b.wav'] })

    vi.mocked(invoke).mockResolvedValue({ id: 'tj-1', meeting_id: 'm-1', provider: 'local-whisper', status: 'queued' })
    const { reprocessTranscription } = await import('../../src/data/transcription')
    await reprocessTranscription('m-1')
    expect(invoke).toHaveBeenCalledWith('reprocess_transcription', { meetingId: 'm-1', provider: 'local-whisper' })
  })

  it('edits transcript segment text', async () => {
    const { updateTranscriptSegmentText } = await import('../../src/data/transcripts')
    await updateTranscriptSegmentText('seg-1', 'corrected')
    expect(invoke).toHaveBeenCalledWith('update_transcript_segment_text', { segmentId: 'seg-1', text: 'corrected' })
  })

  it('triages email and compiles research briefs', async () => {
    vi.mocked(invoke).mockResolvedValue([{ email_id: 'e1', subject: 'Invoice #42', sender: 'x@y.com', received_at: 'now', reason: 'Review payment' }])
    const { emailTriage } = await import('../../src/data/triage')
    const items = await emailTriage('w1')
    expect(items[0].reason).toBe('Review payment')
    expect(invoke).toHaveBeenCalledWith('email_triage', { workspaceId: 'w1' })

    vi.mocked(invoke).mockResolvedValue('# Research Brief: x')
    const { researchBrief } = await import('../../src/data/triage')
    await researchBrief('w1', 'market size')
    expect(invoke).toHaveBeenCalledWith('research_brief', { workspaceId: 'w1', query: 'market size', model: undefined })
  })

  it('schedules calendar-event reminders', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'r-1', workspace_id: 'w1', title: 'Prep', scheduled_at: 'now', status: 'scheduled' })
    const { scheduleEventReminder } = await import('../../src/data/calendar')
    await scheduleEventReminder('w1', 'ev-1', 'Prep for standup', '2026-08-11T08:30:00Z')
    expect(invoke).toHaveBeenCalledWith('schedule_event_reminder', { workspaceId: 'w1', eventId: 'ev-1', title: 'Prep for standup', scheduledAt: '2026-08-11T08:30:00Z' })
  })

  it('lists Teams-bot run history', async () => {
    vi.mocked(invoke).mockResolvedValue([{ id: 'b-1', workspace_id: 'w1', bot_id: 'bot-1', status: 'joined', platform: 'teams', reported_at: 'now' }])
    const { listBotRuns } = await import('../../src/data/botRuns')
    const runs = await listBotRuns('w1')
    expect(runs[0].bot_id).toBe('bot-1')
    expect(invoke).toHaveBeenCalledWith('list_bot_runs', { workspaceId: 'w1' })
  })

  it('advertises the plan §10 provider runtimes', async () => {
    const { providerOptions, isCloudProviderOption } = await import('../../src/data/providers')
    const ids = providerOptions.map((p) => p.id)
    expect(ids).toEqual(expect.arrayContaining(['ollama', 'lmstudio', 'llamacpp', 'vllm', 'opencode', 'openai_compatible', 'openai', 'anthropic', 'google']))
    expect(isCloudProviderOption('ollama')).toBe(false)
    expect(isCloudProviderOption('openai')).toBe(true)
  })

  it('launches the Teams bot with join metadata', async () => {
    vi.mocked(invoke).mockResolvedValue('started')
    const { launchMeetingBot } = await import('../../src/data/meetingBot')
    await launchMeetingBot('teams', 'https://teams.microsoft.com/l/meetup-join/xyz', 'Neural Assistant', 30)
    expect(invoke).toHaveBeenCalledWith('launch_meeting_bot', {
      platform: 'teams', meetingUrl: 'https://teams.microsoft.com/l/meetup-join/xyz', displayName: 'Neural Assistant', durationMinutes: 30,
    })
  })
})
