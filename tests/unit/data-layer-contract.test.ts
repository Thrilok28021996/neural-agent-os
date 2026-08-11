// AUTO-GENERATED contract tests binding every src/data module to its real code.
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

async function callNoThrow(fn: () => unknown) {
  try {
    const result = fn()
    if (result && typeof (result as Promise<unknown>).then === 'function') await result
    return { threw: false }
  } catch (error) {
    return { threw: true, error }
  }
}

beforeEach(() => {
  vi.mocked(invoke).mockClear()
  vi.mocked(invoke).mockResolvedValue(undefined)
})

describe('data/actions', () => {
  it('listOpenActions -> list_open_actions', async () => {
    const mod = await import('../../src/data/actions')
    await mod.listOpenActions('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_open_actions')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/agentActivity', () => {
  it('getAgentActivity -> get_agent_activity', async () => {
    const mod = await import('../../src/data/agentActivity')
    await mod.getAgentActivity('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_agent_activity')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/agents', () => {
  it('createAgent -> create_agent', async () => {
    const mod = await import('../../src/data/agents')
    await mod.createAgent('ws-1', 'Test', 'test', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('create_agent')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('name')
    expect(invoke.mock.calls[0][1]).toHaveProperty('prompt')
    expect(invoke.mock.calls[0][1]).toHaveProperty('schedule')
    expect(invoke.mock.calls[0][1]).toHaveProperty('permissionMode')
  })
  it('updateAgent -> update_agent', async () => {
    const mod = await import('../../src/data/agents')
    await mod.updateAgent('agent-1', 'Test', 'test', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('update_agent')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('name')
    expect(invoke.mock.calls[0][1]).toHaveProperty('prompt')
    expect(invoke.mock.calls[0][1]).toHaveProperty('schedule')
    expect(invoke.mock.calls[0][1]).toHaveProperty('permissionMode')
  })
  it('listAgents -> list_agents', async () => {
    const mod = await import('../../src/data/agents')
    await mod.listAgents('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_agents')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('updateAgentStatus -> update_agent_status', async () => {
    const mod = await import('../../src/data/agents')
    await mod.updateAgentStatus('agent-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('update_agent_status')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('status')
  })
  it('runAgentNow -> run_agent_now', async () => {
    const mod = await import('../../src/data/agents')
    await mod.runAgentNow('agent-1', 'qwen3:14b')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('run_agent_now')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
  it('listAgentRuns -> list_agent_runs', async () => {
    const mod = await import('../../src/data/agents')
    await mod.listAgentRuns('agent-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_agent_runs')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
  })
})

describe('data/approvals', () => {
  it('requestApproval -> request_approval', async () => {
    const mod = await import('../../src/data/approvals')
    await mod.requestApproval('agent-1', 'ws-1', 'read_knowledge', 'test', {}, 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('request_approval')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
    expect(invoke.mock.calls[0][1]).toHaveProperty('description')
    expect(invoke.mock.calls[0][1]).toHaveProperty('payloadJson')
  })
  it('approveRequest -> approve_request', async () => {
    const mod = await import('../../src/data/approvals')
    await mod.approveRequest('id-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('approve_request')
    expect(invoke.mock.calls[0][1]).toHaveProperty('requestId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('approvedBy')
  })
  it('denyRequest -> deny_request', async () => {
    const mod = await import('../../src/data/approvals')
    await mod.denyRequest('id-1', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('deny_request')
    expect(invoke.mock.calls[0][1]).toHaveProperty('requestId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('deniedBy')
    expect(invoke.mock.calls[0][1]).toHaveProperty('reason')
  })
  it('listPendingApprovals -> list_pending_approvals', async () => {
    const mod = await import('../../src/data/approvals')
    await mod.listPendingApprovals('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_pending_approvals')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/assistant', () => {
  it('askAssistant is callable', async () => {
    const mod = await import('../../src/data/assistant')
    const result = await callNoThrow(() => mod.askAssistant('ws-1', 'test', 'qwen3:14b'))
    expect(result.threw).toBe(false)
  })
})

describe('data/automation', () => {
  it('extractEmailActions -> extract_email_actions', async () => {
    const mod = await import('../../src/data/automation')
    await mod.extractEmailActions('id-1', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('extract_email_actions')
    expect(invoke.mock.calls[0][1]).toHaveProperty('emailId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
  it('linkTranscriptToProfile -> link_transcript_to_profile', async () => {
    const mod = await import('../../src/data/automation')
    await mod.linkTranscriptToProfile('id-1', 'id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('link_transcript_to_profile')
    expect(invoke.mock.calls[0][1]).toHaveProperty('segmentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('profileId')
  })
})

describe('data/botRuns', () => {
  it('listBotRuns -> list_bot_runs', async () => {
    const mod = await import('../../src/data/botRuns')
    await mod.listBotRuns('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_bot_runs')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/calendar', () => {
  it('listCalendarEvents -> list_calendar_events', async () => {
    const mod = await import('../../src/data/calendar')
    await mod.listCalendarEvents('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_calendar_events')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('createCalendarEvent -> create_calendar_event', async () => {
    const mod = await import('../../src/data/calendar')
    await mod.createCalendarEvent('ws-1', 'Test', '2026-08-10T09:00:00Z', '2026-08-10T09:00:00Z', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('create_calendar_event')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('title')
    expect(invoke.mock.calls[0][1]).toHaveProperty('startsAt')
    expect(invoke.mock.calls[0][1]).toHaveProperty('endsAt')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingUrl')
    expect(invoke.mock.calls[0][1]).toHaveProperty('recurrence')
  })
  it('updateCalendarEvent -> update_calendar_event', async () => {
    const mod = await import('../../src/data/calendar')
    await mod.updateCalendarEvent('ev-1', 'Test', '2026-08-10T09:00:00Z', '2026-08-10T09:00:00Z', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('update_calendar_event')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('title')
    expect(invoke.mock.calls[0][1]).toHaveProperty('startsAt')
    expect(invoke.mock.calls[0][1]).toHaveProperty('endsAt')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingUrl')
    expect(invoke.mock.calls[0][1]).toHaveProperty('recurrence')
  })
  it('deleteCalendarEvent -> delete_calendar_event', async () => {
    const mod = await import('../../src/data/calendar')
    await mod.deleteCalendarEvent('ev-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_calendar_event')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
  })
  it('pullRemoteCalendar -> pull_remote_calendar', async () => {
    const mod = await import('../../src/data/calendar')
    await mod.pullRemoteCalendar('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('pull_remote_calendar')
    expect(invoke.mock.calls[0][1]).toHaveProperty('connectionId')
  })
  it('scheduleEventReminder -> schedule_event_reminder', async () => {
    const mod = await import('../../src/data/calendar')
    await mod.scheduleEventReminder('ws-1', 'ev-1', 'Test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('schedule_event_reminder')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('title')
    expect(invoke.mock.calls[0][1]).toHaveProperty('scheduledAt')
  })
})

describe('data/calendarComplete', () => {
  it('expandRecurringEvent -> expand_recurring_event', async () => {
    const mod = await import('../../src/data/calendarComplete')
    await mod.expandRecurringEvent('ev-1', '2026-08-10T09:00:00Z', '2026-08-10T09:00:00Z')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('expand_recurring_event')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('fromDate')
    expect(invoke.mock.calls[0][1]).toHaveProperty('toDate')
  })
  it('commonTimezones -> common_timezones', async () => {
    const mod = await import('../../src/data/calendarComplete')
    await mod.commonTimezones()
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('common_timezones')
  })
  it('evaluateRecordingRules -> evaluate_recording_rules', async () => {
    const mod = await import('../../src/data/calendarComplete')
    await mod.evaluateRecordingRules('ws-1', 'test', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('evaluate_recording_rules')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventTitle')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingUrl')
    expect(invoke.mock.calls[0][1]).toHaveProperty('participants')
  })
  it('setRecordingRule -> set_recording_rule', async () => {
    const mod = await import('../../src/data/calendarComplete')
    await mod.setRecordingRule('ws-1', 'test', 'test', 'test', 1)
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('set_recording_rule')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('ruleType')
    expect(invoke.mock.calls[0][1]).toHaveProperty('pattern')
    expect(invoke.mock.calls[0][1]).toHaveProperty('action')
    expect(invoke.mock.calls[0][1]).toHaveProperty('priority')
  })
})

describe('data/calendarConnections', () => {
  it('listCalendarConnections -> list_calendar_connections', async () => {
    const mod = await import('../../src/data/calendarConnections')
    await mod.listCalendarConnections('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_calendar_connections')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('connectCalendarProvider -> connect_calendar_provider', async () => {
    const mod = await import('../../src/data/calendarConnections')
    await mod.connectCalendarProvider('ws-1', 'openai', 'Test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('connect_calendar_provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('accountLabel')
  })
})

describe('data/calendarSync', () => {
  it('generateOAuthUrl -> generate_oauth_url', async () => {
    const mod = await import('../../src/data/calendarSync')
    await mod.generateOAuthUrl('openai')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('generate_oauth_url')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
  })
  it('exchangeOAuthCode -> exchange_oauth_code', async () => {
    const mod = await import('../../src/data/calendarSync')
    await mod.exchangeOAuthCode('openai', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('exchange_oauth_code')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('code')
  })
  it('syncCalendarEvents -> sync_calendar_events', async () => {
    const mod = await import('../../src/data/calendarSync')
    await mod.syncCalendarEvents('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('sync_calendar_events')
    expect(invoke.mock.calls[0][1]).toHaveProperty('connectionId')
  })
})

describe('data/cloudTranscription', () => {
  it('cloudTranscribe -> cloud_transcribe', async () => {
    const mod = await import('../../src/data/cloudTranscription')
    await mod.cloudTranscribe('m-1', '/tmp/test-file.wav', 'openai', 'qwen3:14b')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('cloud_transcribe')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('recordingPath')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/contextExport', () => {
  it('exportContext -> export_context', async () => {
    const mod = await import('../../src/data/contextExport')
    await mod.exportContext('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('export_context')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/conversationSearch', () => {
  it('conversationSearch -> conversation_search', async () => {
    const mod = await import('../../src/data/conversationSearch')
    await mod.conversationSearch('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('conversation_search')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
  })
})

describe('data/costTracking', () => {
  it('getCostSummary -> get_cost_summary', async () => {
    const mod = await import('../../src/data/costTracking')
    await mod.getCostSummary('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_cost_summary')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('setCostLimit -> set_cost_limit', async () => {
    const mod = await import('../../src/data/costTracking')
    await mod.setCostLimit('ws-1', 'read_knowledge', 1)
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('set_cost_limit')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
    expect(invoke.mock.calls[0][1]).toHaveProperty('limitCents')
  })
  it('getCostLimit -> get_cost_limit', async () => {
    const mod = await import('../../src/data/costTracking')
    await mod.getCostLimit('ws-1', 'read_knowledge')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_cost_limit')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
  })
})

describe('data/deletion', () => {
  it('previewSourceDeletion -> preview_source_deletion', async () => {
    const mod = await import('../../src/data/deletion')
    await mod.previewSourceDeletion('s-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('preview_source_deletion')
    expect(invoke.mock.calls[0][1]).toHaveProperty('sourceId')
  })
  it('previewMeetingDeletion -> preview_meeting_deletion', async () => {
    const mod = await import('../../src/data/deletion')
    await mod.previewMeetingDeletion('m-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('preview_meeting_deletion')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
  })
  it('fullDeletionPreview -> full_deletion_preview', async () => {
    const mod = await import('../../src/data/deletion')
    await mod.fullDeletionPreview('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('full_deletion_preview')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/diagnostics', () => {
  it('getStorageStats -> get_storage_stats', async () => {
    const mod = await import('../../src/data/diagnostics')
    await mod.getStorageStats()
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_storage_stats')
  })
  it('cleanupOldRecordings -> cleanup_old_recordings', async () => {
    const mod = await import('../../src/data/diagnostics')
    await mod.cleanupOldRecordings('test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('cleanup_old_recordings')
    expect(invoke.mock.calls[0][1]).toHaveProperty('olderThanDays')
    expect(invoke.mock.calls[0][1]).toHaveProperty('dryRun')
  })
  it('getAuditLog -> get_audit_log', async () => {
    const mod = await import('../../src/data/diagnostics')
    await mod.getAuditLog('test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_audit_log')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('limit')
  })
})

describe('data/diarization', () => {
  it('diarizeTranscript -> diarize_transcript', async () => {
    const mod = await import('../../src/data/diarization')
    await mod.diarizeTranscript('m-1', 'ws-1', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('diarize_transcript')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('embeddingModel')
    expect(invoke.mock.calls[0][1]).toHaveProperty('similarityThreshold')
  })
  it('buildSpeakerEmbedding -> build_speaker_embedding', async () => {
    const mod = await import('../../src/data/diarization')
    await mod.buildSpeakerEmbedding('id-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('build_speaker_embedding')
    expect(invoke.mock.calls[0][1]).toHaveProperty('profileId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/duplicates', () => {
  it('findDuplicates -> find_duplicates', async () => {
    const mod = await import('../../src/data/duplicates')
    await mod.findDuplicates('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('find_duplicates')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/email', () => {
  it('connectEmailAccount -> connect_email_account', async () => {
    const mod = await import('../../src/data/email')
    await mod.connectEmailAccount('ws-1', 'openai', 'Test', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('connect_email_account')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('accountLabel')
    expect(invoke.mock.calls[0][1]).toHaveProperty('imapHost')
    expect(invoke.mock.calls[0][1]).toHaveProperty('smtpHost')
  })
  it('listEmailAccounts -> list_email_accounts', async () => {
    const mod = await import('../../src/data/email')
    await mod.listEmailAccounts('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_email_accounts')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('storeEmailSecret -> store_email_secret', async () => {
    const mod = await import('../../src/data/email')
    await mod.storeEmailSecret('id-1', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('store_email_secret')
    expect(invoke.mock.calls[0][1]).toHaveProperty('accountId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('secretKind')
    expect(invoke.mock.calls[0][1]).toHaveProperty('value')
  })
  it('syncEmailAccount -> sync_email_account', async () => {
    const mod = await import('../../src/data/email')
    await mod.syncEmailAccount('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('sync_email_account')
    expect(invoke.mock.calls[0][1]).toHaveProperty('accountId')
  })
  it('syncEmailOAuth -> sync_email_oauth', async () => {
    const mod = await import('../../src/data/email')
    await mod.syncEmailOAuth('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('sync_email_oauth')
    expect(invoke.mock.calls[0][1]).toHaveProperty('accountId')
  })
  it('listEmails -> list_emails', async () => {
    const mod = await import('../../src/data/email')
    await mod.listEmails('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_emails')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
  })
})

describe('data/emailComplete', () => {
  it('rebuildThreads -> rebuild_threads', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.rebuildThreads('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('rebuild_threads')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('createLabel -> create_label', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.createLabel('id-1', 'Test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('create_label')
    expect(invoke.mock.calls[0][1]).toHaveProperty('accountId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('name')
    expect(invoke.mock.calls[0][1]).toHaveProperty('color')
  })
  it('listLabels -> list_labels', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.listLabels('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_labels')
    expect(invoke.mock.calls[0][1]).toHaveProperty('accountId')
  })
  it('labelEmail -> label_email', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.labelEmail('id-1', 'id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('label_email')
    expect(invoke.mock.calls[0][1]).toHaveProperty('emailId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('labelId')
  })
  it('indexAttachment -> index_attachment', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.indexAttachment('id-1', 'test', 'test', 1)
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('index_attachment')
    expect(invoke.mock.calls[0][1]).toHaveProperty('emailId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('filename')
    expect(invoke.mock.calls[0][1]).toHaveProperty('contentType')
    expect(invoke.mock.calls[0][1]).toHaveProperty('sizeBytes')
  })
  it('generateEmailDraft -> generate_email_draft', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.generateEmailDraft('ws-1', 'test', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('generate_email_draft')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('instructions')
    expect(invoke.mock.calls[0][1]).toHaveProperty('context')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
  it('summarizeEmailThread -> summarize_email_thread', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.summarizeEmailThread('id-1', 'ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('summarize_email_thread')
    expect(invoke.mock.calls[0][1]).toHaveProperty('threadId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
  it('extractEmailActionsFromEmail -> extract_email_actions', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.extractEmailActionsFromEmail('id-1', 'ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('extract_email_actions')
    expect(invoke.mock.calls[0][1]).toHaveProperty('emailId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
  it('extractAttachmentText -> extract_attachment_text', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.extractAttachmentText('/tmp/test-file.wav', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('extract_attachment_text')
    expect(invoke.mock.calls[0][1]).toHaveProperty('path')
    expect(invoke.mock.calls[0][1]).toHaveProperty('filename')
  })
  it('suggestWorkspaceForEmail -> suggest_workspace_for_email', async () => {
    const mod = await import('../../src/data/emailComplete')
    await mod.suggestWorkspaceForEmail('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('suggest_workspace_for_email')
    expect(invoke.mock.calls[0][1]).toHaveProperty('emailId')
  })
})

describe('data/emailSearch', () => {
  it('searchEmails -> search_emails', async () => {
    const mod = await import('../../src/data/emailSearch')
    await mod.searchEmails('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('search_emails')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
  })
  it('semanticSearchEmails -> semantic_search_emails', async () => {
    const mod = await import('../../src/data/emailSearch')
    await mod.semanticSearchEmails('ws-1', 'test', 'qwen3:14b')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('semantic_search_emails')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/emailSend', () => {
  it('sendEmail -> send_email', async () => {
    const mod = await import('../../src/data/emailSend')
    await mod.sendEmail('id-1', 'test', 'test', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('send_email')
    expect(invoke.mock.calls[0][1]).toHaveProperty('accountId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('to')
    expect(invoke.mock.calls[0][1]).toHaveProperty('subject')
    expect(invoke.mock.calls[0][1]).toHaveProperty('body')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
  })
})

describe('data/embeddings', () => {
  it('embedSources -> embed_sources', async () => {
    const mod = await import('../../src/data/embeddings')
    await mod.embedSources('ws-1', 'qwen3:14b')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('embed_sources')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/export', () => {
  it('exportWorkspace -> export_workspace', async () => {
    const mod = await import('../../src/data/export')
    await mod.exportWorkspace('ws-1', '/tmp/test-file.wav')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('export_workspace')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('path')
  })
  it('importWorkspace -> import_workspace', async () => {
    const mod = await import('../../src/data/export')
    await mod.importWorkspace('/tmp/test-file.wav')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('import_workspace')
    expect(invoke.mock.calls[0][1]).toHaveProperty('path')
  })
})

describe('data/fallback', () => {
  it('getFallbackChain -> get_fallback_chain', async () => {
    const mod = await import('../../src/data/fallback')
    await mod.getFallbackChain('ws-1', 'read_knowledge')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_fallback_chain')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
  })
  it('setFallbackProvider -> set_fallback_provider', async () => {
    const mod = await import('../../src/data/fallback')
    await mod.setFallbackProvider('ws-1', 'read_knowledge', 'openai', 'qwen3:14b', 1)
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('set_fallback_provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
    expect(invoke.mock.calls[0][1]).toHaveProperty('priority')
  })
  it('clearFallbackChain -> clear_fallback_chain', async () => {
    const mod = await import('../../src/data/fallback')
    await mod.clearFallbackChain('ws-1', 'read_knowledge')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('clear_fallback_chain')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
  })
})

describe('data/health', () => {
  it('refreshProviderHealth -> refresh_provider_health', async () => {
    const mod = await import('../../src/data/health')
    await mod.refreshProviderHealth('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('refresh_provider_health')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('getProviderHealth -> get_provider_health', async () => {
    const mod = await import('../../src/data/health')
    await mod.getProviderHealth('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_provider_health')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/hybridSearch', () => {
  it('hybridSearch -> hybrid_search', async () => {
    const mod = await import('../../src/data/hybridSearch')
    await mod.hybridSearch('ws-1', 'test', 'qwen3:14b')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('hybrid_search')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/ingestion', () => {
  it('ingestDirectory -> ingest_directory', async () => {
    const mod = await import('../../src/data/ingestion')
    await mod.ingestDirectory('ws-1', '/tmp/test-file.wav')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('ingest_directory')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('directory')
  })
})

describe('data/jobQueue', () => {
  it('enqueueJob -> enqueue_job', async () => {
    const mod = await import('../../src/data/jobQueue')
    await mod.enqueueJob('test', 'ws-1', {}, 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('enqueue_job')
    expect(invoke.mock.calls[0][1]).toHaveProperty('jobType')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('payloadJson')
    expect(invoke.mock.calls[0][1]).toHaveProperty('maxRetries')
  })
  it('listJobs -> list_jobs', async () => {
    const mod = await import('../../src/data/jobQueue')
    await mod.listJobs('ws-1', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_jobs')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('statusFilter')
    expect(invoke.mock.calls[0][1]).toHaveProperty('limit')
  })
  it('cancelJob -> cancel_job', async () => {
    const mod = await import('../../src/data/jobQueue')
    await mod.cancelJob('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('cancel_job')
    expect(invoke.mock.calls[0][1]).toHaveProperty('jobId')
  })
  it('getQueueStatus -> get_queue_status', async () => {
    const mod = await import('../../src/data/jobQueue')
    await mod.getQueueStatus('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_queue_status')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/meetingBot', () => {
  it('launchMeetingBot -> launch_meeting_bot', async () => {
    const mod = await import('../../src/data/meetingBot')
    await mod.launchMeetingBot('test', 'test', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('launch_meeting_bot')
    expect(invoke.mock.calls[0][1]).toHaveProperty('platform')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingUrl')
    expect(invoke.mock.calls[0][1]).toHaveProperty('displayName')
    expect(invoke.mock.calls[0][1]).toHaveProperty('durationMinutes')
  })
})

describe('data/meetingDetection', () => {
  it('detectUpcomingMeetings -> detect_upcoming_meetings', async () => {
    const mod = await import('../../src/data/meetingDetection')
    await mod.detectUpcomingMeetings('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('detect_upcoming_meetings')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('hoursAhead')
  })
})

describe('data/meetingPrep', () => {
  it('prepareMeeting -> prepare_meeting', async () => {
    const mod = await import('../../src/data/meetingPrep')
    await mod.prepareMeeting('ev-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('prepare_meeting')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
  })
})

describe('data/meetings', () => {
  it('importMeetingRecording -> import_meeting_recording', async () => {
    const mod = await import('../../src/data/meetings')
    await mod.importMeetingRecording('ws-1', '/tmp/test-file.wav', 'Test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('import_meeting_recording')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('path')
    expect(invoke.mock.calls[0][1]).toHaveProperty('title')
  })
  it('listMeetings -> list_meetings', async () => {
    const mod = await import('../../src/data/meetings')
    await mod.listMeetings('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_meetings')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('importMeetingRecordingsBatch -> import_meeting_recordings_batch', async () => {
    const mod = await import('../../src/data/meetings')
    await mod.importMeetingRecordingsBatch('ws-1', '/tmp/test-file.wav')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('import_meeting_recordings_batch')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('paths')
  })
})

describe('data/memories', () => {
  it('createMemory -> create_memory', async () => {
    const mod = await import('../../src/data/memories')
    await mod.createMemory('ws-1', 'test', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('create_memory')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('content')
    expect(invoke.mock.calls[0][1]).toHaveProperty('sourceKind')
    expect(invoke.mock.calls[0][1]).toHaveProperty('sourceId')
  })
  it('listMemories -> list_memories', async () => {
    const mod = await import('../../src/data/memories')
    await mod.listMemories('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_memories')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('searchMemories -> search_memories', async () => {
    const mod = await import('../../src/data/memories')
    await mod.searchMemories('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('search_memories')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
  })
  it('deleteMemory -> delete_memory', async () => {
    const mod = await import('../../src/data/memories')
    await mod.deleteMemory('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_memory')
    expect(invoke.mock.calls[0][1]).toHaveProperty('memoryId')
  })
})

describe('data/modelRoutes', () => {
  it('listModelRoutes -> list_model_routes', async () => {
    const mod = await import('../../src/data/modelRoutes')
    await mod.listModelRoutes('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_model_routes')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('setModelRoute -> set_model_route', async () => {
    const mod = await import('../../src/data/modelRoutes')
    await mod.setModelRoute('ws-1', 'read_knowledge', 'openai', 'qwen3:14b')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('set_model_route')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/monitoring', () => {
  it('addMonitoredFolder -> add_monitored_folder', async () => {
    const mod = await import('../../src/data/monitoring')
    await mod.addMonitoredFolder('ws-1', '/tmp/test-file.wav')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('add_monitored_folder')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('path')
  })
  it('listMonitoredFolders -> list_monitored_folders', async () => {
    const mod = await import('../../src/data/monitoring')
    await mod.listMonitoredFolders('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_monitored_folders')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('toggleMonitoredFolder -> toggle_monitored_folder', async () => {
    const mod = await import('../../src/data/monitoring')
    await mod.toggleMonitoredFolder('id-1', false)
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('toggle_monitored_folder')
    expect(invoke.mock.calls[0][1]).toHaveProperty('folderId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('enabled')
  })
  it('removeMonitoredFolder -> remove_monitored_folder', async () => {
    const mod = await import('../../src/data/monitoring')
    await mod.removeMonitoredFolder('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('remove_monitored_folder')
    expect(invoke.mock.calls[0][1]).toHaveProperty('folderId')
  })
})

describe('data/native', () => {
  it('loadNativeWorkspaces -> list_workspaces', async () => {
    const mod = await import('../../src/data/native')
    await mod.loadNativeWorkspaces()
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_workspaces')
  })
  it('saveNativeSetting -> save_setting', async () => {
    const mod = await import('../../src/data/native')
    await mod.saveNativeSetting('test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('save_setting')
    expect(invoke.mock.calls[0][1]).toHaveProperty('key')
    expect(invoke.mock.calls[0][1]).toHaveProperty('value')
  })
  it('loadNativeSetting -> load_setting', async () => {
    const mod = await import('../../src/data/native')
    await mod.loadNativeSetting('test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('load_setting')
    expect(invoke.mock.calls[0][1]).toHaveProperty('key')
  })
  it('setDataDirectory -> set_data_directory', async () => {
    const mod = await import('../../src/data/native')
    await mod.setDataDirectory('/tmp/test-file.wav')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('set_data_directory')
    expect(invoke.mock.calls[0][1]).toHaveProperty('directory')
  })
  it('pickDirectory is callable', async () => {
    const mod = await import('../../src/data/native')
    const result = await callNoThrow(() => mod.pickDirectory())
    expect(result.threw).toBe(false)
  })
  it('pickRecordingFile is callable', async () => {
    const mod = await import('../../src/data/native')
    const result = await callNoThrow(() => mod.pickRecordingFile())
    expect(result.threw).toBe(false)
  })
})

describe('data/notes', () => {
  it('createNote -> create_note', async () => {
    const mod = await import('../../src/data/notes')
    await mod.createNote('ws-1', 'Test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('create_note')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('title')
    expect(invoke.mock.calls[0][1]).toHaveProperty('content')
  })
  it('updateNote -> update_note', async () => {
    const mod = await import('../../src/data/notes')
    await mod.updateNote('id-1', 'Test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('update_note')
    expect(invoke.mock.calls[0][1]).toHaveProperty('noteId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('title')
    expect(invoke.mock.calls[0][1]).toHaveProperty('content')
  })
  it('listNotes -> list_notes', async () => {
    const mod = await import('../../src/data/notes')
    await mod.listNotes('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_notes')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('deleteNote -> delete_note', async () => {
    const mod = await import('../../src/data/notes')
    await mod.deleteNote('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_note')
    expect(invoke.mock.calls[0][1]).toHaveProperty('noteId')
  })
  it('searchNotes -> search_notes', async () => {
    const mod = await import('../../src/data/notes')
    await mod.searchNotes('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('search_notes')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
  })
})

describe('data/oauth', () => {
  it('getOAuthConfig -> get_oauth_config', async () => {
    const mod = await import('../../src/data/oauth')
    await mod.getOAuthConfig('openai')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_oauth_config')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
  })
  it('generatePKCE -> generate_pkce', async () => {
    const mod = await import('../../src/data/oauth')
    await mod.generatePKCE()
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('generate_pkce')
  })
  it('buildAuthorizeUrl -> build_authorize_url', async () => {
    const mod = await import('../../src/data/oauth')
    await mod.buildAuthorizeUrl('openai', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('build_authorize_url')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('pkce')
  })
  it('exchangeCodeWithPKCE -> exchange_code_with_pkce', async () => {
    const mod = await import('../../src/data/oauth')
    await mod.exchangeCodeWithPKCE('openai', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('exchange_code_with_pkce')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('code')
    expect(invoke.mock.calls[0][1]).toHaveProperty('pkce')
  })
  it('refreshOAuthToken -> refresh_oauth_token', async () => {
    const mod = await import('../../src/data/oauth')
    await mod.refreshOAuthToken('openai')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('refresh_oauth_token')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
  })
})

describe('data/participants', () => {
  it('addParticipant -> add_participant', async () => {
    const mod = await import('../../src/data/participants')
    await mod.addParticipant('ev-1', 'Test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('add_participant')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('name')
    expect(invoke.mock.calls[0][1]).toHaveProperty('email')
  })
  it('listParticipants -> list_participants', async () => {
    const mod = await import('../../src/data/participants')
    await mod.listParticipants('ev-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_participants')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
  })
  it('updateParticipantStatus -> update_participant_status', async () => {
    const mod = await import('../../src/data/participants')
    await mod.updateParticipantStatus('id-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('update_participant_status')
    expect(invoke.mock.calls[0][1]).toHaveProperty('participantId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('status')
  })
  it('removeParticipant -> remove_participant', async () => {
    const mod = await import('../../src/data/participants')
    await mod.removeParticipant('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('remove_participant')
    expect(invoke.mock.calls[0][1]).toHaveProperty('participantId')
  })
  it('detectConflicts -> detect_conflicts', async () => {
    const mod = await import('../../src/data/participants')
    await mod.detectConflicts('ws-1', '2026-08-10T09:00:00Z', '2026-08-10T09:00:00Z', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('detect_conflicts')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('startsAt')
    expect(invoke.mock.calls[0][1]).toHaveProperty('endsAt')
    expect(invoke.mock.calls[0][1]).toHaveProperty('excludeEventId')
  })
  it('checkConflictsForEvent -> check_conflicts_for_event', async () => {
    const mod = await import('../../src/data/participants')
    await mod.checkConflictsForEvent('ws-1', 'ev-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('check_conflicts_for_event')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
  })
  it('sendCalendarInvitation -> send_calendar_invitation', async () => {
    const mod = await import('../../src/data/participants')
    await mod.sendCalendarInvitation('ev-1', 'id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('send_calendar_invitation')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('participantId')
  })
  it('processRsvpResponse -> process_rsvp_response', async () => {
    const mod = await import('../../src/data/participants')
    await mod.processRsvpResponse('id-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('process_rsvp_response')
    expect(invoke.mock.calls[0][1]).toHaveProperty('participantId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('response')
  })
  it('generateIcs -> generate_ics', async () => {
    const mod = await import('../../src/data/participants')
    await mod.generateIcs('ev-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('generate_ics')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventId')
  })
})

describe('data/permissions', () => {
  it('checkPermission -> check_permission', async () => {
    const mod = await import('../../src/data/permissions')
    await mod.checkPermission('agent-1', 'ws-1', 'read_knowledge')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('check_permission')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
  })
  it('checkAllPermissions -> check_all_permissions', async () => {
    const mod = await import('../../src/data/permissions')
    await mod.checkAllPermissions('agent-1', 'ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('check_all_permissions')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('grantPermission -> grant_permission', async () => {
    const mod = await import('../../src/data/permissions')
    await mod.grantPermission('agent-1', 'ws-1', 'read_knowledge')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('grant_permission')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
  })
  it('revokePermission -> revoke_permission', async () => {
    const mod = await import('../../src/data/permissions')
    await mod.revokePermission('agent-1', 'ws-1', 'read_knowledge')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('revoke_permission')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
  })
  it('setWorkspaceAllowlist -> set_workspace_allowlist', async () => {
    const mod = await import('../../src/data/permissions')
    await mod.setWorkspaceAllowlist('ws-1', 'read_knowledge')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('set_workspace_allowlist')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capabilities')
  })
})

describe('data/providerSecrets', () => {
  it('storeProviderSecret -> store_provider_secret', async () => {
    const mod = await import('../../src/data/providerSecrets')
    await mod.storeProviderSecret('openai', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('store_provider_secret')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('secretKind')
    expect(invoke.mock.calls[0][1]).toHaveProperty('value')
  })
  it('deleteProviderSecret -> delete_provider_secret', async () => {
    const mod = await import('../../src/data/providerSecrets')
    await mod.deleteProviderSecret('openai', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_provider_secret')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('secretKind')
  })
})

describe('data/providers', () => {
  it('checkProvider is callable', async () => {
    const mod = await import('../../src/data/providers')
    const result = await callNoThrow(() => mod.checkProvider('test'))
    expect(result.threw).toBe(false)
  })
  it('isCloudProviderOption is callable', async () => {
    const mod = await import('../../src/data/providers')
    const result = await callNoThrow(() => mod.isCloudProviderOption('id-1'))
    expect(result.threw).toBe(false)
  })
  it('providerLabel is callable', async () => {
    const mod = await import('../../src/data/providers')
    const result = await callNoThrow(() => mod.providerLabel('id-1'))
    expect(result.threw).toBe(false)
  })
})

describe('data/recording', () => {
  it('startLocalRecording -> start_local_recording', async () => {
    const mod = await import('../../src/data/recording')
    await mod.startLocalRecording('/tmp/test-file.wav', 'default', {})
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('start_local_recording')
    expect(invoke.mock.calls[0][1]).toHaveProperty('path')
    expect(invoke.mock.calls[0][1]).toHaveProperty('source')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventTitle')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingUrl')
    expect(invoke.mock.calls[0][1]).toHaveProperty('participants')
    expect(invoke.mock.calls[0][1]).toHaveProperty('confirmed')
  })
  it('stopLocalRecording -> stop_local_recording', async () => {
    const mod = await import('../../src/data/recording')
    await mod.stopLocalRecording()
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('stop_local_recording')
  })
  it('checkRecordingAllowed -> check_recording_allowed', async () => {
    const mod = await import('../../src/data/recording')
    await mod.checkRecordingAllowed('test', 'test', 'test', [])
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('check_recording_allowed')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('eventTitle')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingUrl')
    expect(invoke.mock.calls[0][1]).toHaveProperty('participants')
  })
})

describe('data/reminders', () => {
  it('scheduleTaskReminder -> schedule_task_reminder', async () => {
    const mod = await import('../../src/data/reminders')
    await mod.scheduleTaskReminder('ws-1', 'id-1', 'Test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('schedule_task_reminder')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('taskId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('title')
    expect(invoke.mock.calls[0][1]).toHaveProperty('scheduledAt')
  })
  it('listReminders -> list_reminders', async () => {
    const mod = await import('../../src/data/reminders')
    await mod.listReminders('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_reminders')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('cancelReminder -> cancel_reminder', async () => {
    const mod = await import('../../src/data/reminders')
    await mod.cancelReminder('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('cancel_reminder')
    expect(invoke.mock.calls[0][1]).toHaveProperty('reminderId')
  })
  it('deleteReminder -> delete_reminder', async () => {
    const mod = await import('../../src/data/reminders')
    await mod.deleteReminder('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_reminder')
    expect(invoke.mock.calls[0][1]).toHaveProperty('reminderId')
  })
  it('notifyReminder is callable', async () => {
    const mod = await import('../../src/data/reminders')
    const result = await callNoThrow(() => mod.notifyReminder('test'))
    expect(result.threw).toBe(false)
  })
})

describe('data/reranking', () => {
  it('rerankSearch -> rerank_search', async () => {
    const mod = await import('../../src/data/reranking')
    await mod.rerankSearch('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('rerank_search')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
  })
})

describe('data/retrieval', () => {
  it('searchSources -> search_sources', async () => {
    const mod = await import('../../src/data/retrieval')
    await mod.searchSources('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('search_sources')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
  })
})

describe('data/search', () => {
  it('indexSources -> index_sources', async () => {
    const mod = await import('../../src/data/search')
    await mod.indexSources('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('index_sources')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/security', () => {
  it('executeCommand -> execute_command', async () => {
    const mod = await import('../../src/data/security')
    await mod.executeCommand('agent-1', 'ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('execute_command')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('command')
  })
  it('modifyFile -> modify_file', async () => {
    const mod = await import('../../src/data/security')
    await mod.modifyFile('agent-1', 'ws-1', '/tmp/test-file.wav', 'test', false)
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('modify_file')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('path')
    expect(invoke.mock.calls[0][1]).toHaveProperty('content')
    expect(invoke.mock.calls[0][1]).toHaveProperty('append')
  })
  it('authorizeAction -> authorize_action', async () => {
    const mod = await import('../../src/data/security')
    await mod.authorizeAction('agent-1', 'ws-1', 'read_knowledge', 'test', {}, 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('authorize_action')
    expect(invoke.mock.calls[0][1]).toHaveProperty('agentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
    expect(invoke.mock.calls[0][1]).toHaveProperty('description')
    expect(invoke.mock.calls[0][1]).toHaveProperty('payloadJson')
  })
  it('generateApiKey -> generate_api_key', async () => {
    const mod = await import('../../src/data/security')
    await mod.generateApiKey('Test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('generate_api_key')
    expect(invoke.mock.calls[0][1]).toHaveProperty('label')
  })
  it('listApiKeys -> list_api_keys', async () => {
    const mod = await import('../../src/data/security')
    await mod.listApiKeys()
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_api_keys')
  })
  it('revokeApiKey -> revoke_api_key', async () => {
    const mod = await import('../../src/data/security')
    await mod.revokeApiKey('Test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('revoke_api_key')
    expect(invoke.mock.calls[0][1]).toHaveProperty('label')
  })
  it('deleteSourceEmbeddings -> delete_source_embeddings', async () => {
    const mod = await import('../../src/data/security')
    await mod.deleteSourceEmbeddings('s-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_source_embeddings')
    expect(invoke.mock.calls[0][1]).toHaveProperty('sourceId')
  })
  it('deleteTranscriptSegments -> delete_transcript_segments', async () => {
    const mod = await import('../../src/data/security')
    await mod.deleteTranscriptSegments('m-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_transcript_segments')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
  })
  it('deleteMeetingSummary -> delete_meeting_summary', async () => {
    const mod = await import('../../src/data/security')
    await mod.deleteMeetingSummary('m-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_meeting_summary')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
  })
  it('deleteMeetingActions -> delete_meeting_actions', async () => {
    const mod = await import('../../src/data/security')
    await mod.deleteMeetingActions('m-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_meeting_actions')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
  })
  it('deleteMeetingRecording -> delete_meeting_recording', async () => {
    const mod = await import('../../src/data/security')
    await mod.deleteMeetingRecording('m-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_meeting_recording')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
  })
  it('getWorkspaceRetention -> get_workspace_retention', async () => {
    const mod = await import('../../src/data/security')
    await mod.getWorkspaceRetention('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_workspace_retention')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('setWorkspaceRetention -> set_workspace_retention', async () => {
    const mod = await import('../../src/data/security')
    await mod.setWorkspaceRetention('ws-1', 1)
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('set_workspace_retention')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('retentionDays')
  })
})

describe('data/semanticSearch', () => {
  it('semanticSearchSources -> semantic_search_sources', async () => {
    const mod = await import('../../src/data/semanticSearch')
    await mod.semanticSearchSources('ws-1', 'test', 'qwen3:14b')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('semantic_search_sources')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/settings', () => {
  it('loadSettings is callable', async () => {
    const mod = await import('../../src/data/settings')
    const result = await callNoThrow(() => mod.loadSettings())
    expect(result.threw).toBe(false)
  })
  it('saveSettings is callable', async () => {
    const mod = await import('../../src/data/settings')
    const result = await callNoThrow(() => mod.saveSettings({}))
    expect(result.threw).toBe(false)
  })
})

describe('data/sources', () => {
  it('listSources -> list_sources', async () => {
    const mod = await import('../../src/data/sources')
    await mod.listSources('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_sources')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('deleteSource -> delete_source', async () => {
    const mod = await import('../../src/data/sources')
    await mod.deleteSource('s-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_source')
    expect(invoke.mock.calls[0][1]).toHaveProperty('sourceId')
  })
})

describe('data/summaries', () => {
  it('summarizeMeeting -> summarize_meeting', async () => {
    const mod = await import('../../src/data/summaries')
    await mod.summarizeMeeting('m-1', 'qwen3:14b')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('summarize_meeting')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/syncQueue', () => {
  it('enqueueSync -> enqueue_sync', async () => {
    const mod = await import('../../src/data/syncQueue')
    await mod.enqueueSync('test', 'id-1', 'openai', 'test', {}, 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('enqueue_sync')
    expect(invoke.mock.calls[0][1]).toHaveProperty('entityType')
    expect(invoke.mock.calls[0][1]).toHaveProperty('entityId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
    expect(invoke.mock.calls[0][1]).toHaveProperty('action')
    expect(invoke.mock.calls[0][1]).toHaveProperty('payloadJson')
  })
  it('getSyncStatus -> get_sync_status', async () => {
    const mod = await import('../../src/data/syncQueue')
    await mod.getSyncStatus('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_sync_status')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('resolveSyncConflict -> resolve_sync_conflict', async () => {
    const mod = await import('../../src/data/syncQueue')
    await mod.resolveSyncConflict('id-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('resolve_sync_conflict')
    expect(invoke.mock.calls[0][1]).toHaveProperty('queueId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('resolution')
  })
})

describe('data/tasks', () => {
  it('createTask -> create_task', async () => {
    const mod = await import('../../src/data/tasks')
    await mod.createTask('ws-1', 'Test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('create_task')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('title')
    expect(invoke.mock.calls[0][1]).toHaveProperty('dueAt')
  })
  it('deleteTask -> delete_task', async () => {
    const mod = await import('../../src/data/tasks')
    await mod.deleteTask('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_task')
    expect(invoke.mock.calls[0][1]).toHaveProperty('taskId')
  })
  it('promoteAction -> promote_action_to_task', async () => {
    const mod = await import('../../src/data/tasks')
    await mod.promoteAction('id-1', 'ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('promote_action_to_task')
    expect(invoke.mock.calls[0][1]).toHaveProperty('actionId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('listTasks -> list_tasks', async () => {
    const mod = await import('../../src/data/tasks')
    await mod.listTasks('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_tasks')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('updateTaskStatus -> update_task_status', async () => {
    const mod = await import('../../src/data/tasks')
    await mod.updateTaskStatus('id-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('update_task_status')
    expect(invoke.mock.calls[0][1]).toHaveProperty('taskId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('status')
  })
})

describe('data/tokenLimits', () => {
  it('setTokenLimit -> set_token_limit', async () => {
    const mod = await import('../../src/data/tokenLimits')
    await mod.setTokenLimit('ws-1', 'read_knowledge', 1)
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('set_token_limit')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
    expect(invoke.mock.calls[0][1]).toHaveProperty('limitTokens')
  })
  it('getTokenLimit -> get_token_limit', async () => {
    const mod = await import('../../src/data/tokenLimits')
    await mod.getTokenLimit('ws-1', 'read_knowledge')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('get_token_limit')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('capability')
  })
  it('listTokenLimits -> list_token_limits', async () => {
    const mod = await import('../../src/data/tokenLimits')
    await mod.listTokenLimits('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_token_limits')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

describe('data/transcription', () => {
  it('queueTranscription -> queue_transcription', async () => {
    const mod = await import('../../src/data/transcription')
    await mod.queueTranscription('m-1', 'openai')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('queue_transcription')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
  })
  it('listTranscriptionJobs -> list_transcription_jobs', async () => {
    const mod = await import('../../src/data/transcription')
    await mod.listTranscriptionJobs('m-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_transcription_jobs')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
  })
  it('processTranscriptionJob -> process_transcription_job', async () => {
    const mod = await import('../../src/data/transcription')
    await mod.processTranscriptionJob('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('process_transcription_job')
    expect(invoke.mock.calls[0][1]).toHaveProperty('jobId')
  })
  it('reprocessTranscription -> reprocess_transcription', async () => {
    const mod = await import('../../src/data/transcription')
    await mod.reprocessTranscription('m-1', 'openai')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('reprocess_transcription')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('provider')
  })
})

describe('data/transcripts', () => {
  it('listTranscriptSegments -> list_transcript_segments', async () => {
    const mod = await import('../../src/data/transcripts')
    await mod.listTranscriptSegments('m-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_transcript_segments')
    expect(invoke.mock.calls[0][1]).toHaveProperty('meetingId')
  })
  it('updateTranscriptSpeaker -> update_transcript_speaker', async () => {
    const mod = await import('../../src/data/transcripts')
    await mod.updateTranscriptSpeaker('id-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('update_transcript_speaker')
    expect(invoke.mock.calls[0][1]).toHaveProperty('segmentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('speaker')
  })
  it('updateTranscriptSegmentText -> update_transcript_segment_text', async () => {
    const mod = await import('../../src/data/transcripts')
    await mod.updateTranscriptSegmentText('id-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('update_transcript_segment_text')
    expect(invoke.mock.calls[0][1]).toHaveProperty('segmentId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('text')
  })
})

describe('data/triage', () => {
  it('emailTriage -> email_triage', async () => {
    const mod = await import('../../src/data/triage')
    await mod.emailTriage('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('email_triage')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('researchBrief -> research_brief', async () => {
    const mod = await import('../../src/data/triage')
    await mod.researchBrief('ws-1', 'test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('research_brief')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('query')
    expect(invoke.mock.calls[0][1]).toHaveProperty('model')
  })
})

describe('data/voiceCapture', () => {
  it('startVoiceCapture -> start_voice_capture', async () => {
    const mod = await import('../../src/data/voiceCapture')
    await mod.startVoiceCapture('test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('start_voice_capture')
    expect(invoke.mock.calls[0][1]).toHaveProperty('language')
  })
  it('stopVoiceCapture -> stop_voice_capture', async () => {
    const mod = await import('../../src/data/voiceCapture')
    await mod.stopVoiceCapture()
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('stop_voice_capture')
  })
})

describe('data/voicePipeline', () => {
  it('localTextToSpeech -> local_text_to_speech', async () => {
    const mod = await import('../../src/data/voicePipeline')
    await mod.localTextToSpeech('test', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('local_text_to_speech')
    expect(invoke.mock.calls[0][1]).toHaveProperty('text')
    expect(invoke.mock.calls[0][1]).toHaveProperty('language')
  })
  it('localSpeechToText -> local_speech_to_text', async () => {
    const mod = await import('../../src/data/voicePipeline')
    await mod.localSpeechToText('/tmp/test-file.wav', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('local_speech_to_text')
    expect(invoke.mock.calls[0][1]).toHaveProperty('audioPath')
    expect(invoke.mock.calls[0][1]).toHaveProperty('language')
  })
})

describe('data/voiceProfiles', () => {
  it('createVoiceProfile -> create_voice_profile', async () => {
    const mod = await import('../../src/data/voiceProfiles')
    await mod.createVoiceProfile('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('create_voice_profile')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('speakerLabel')
  })
  it('listVoiceProfiles -> list_voice_profiles', async () => {
    const mod = await import('../../src/data/voiceProfiles')
    await mod.listVoiceProfiles('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('list_voice_profiles')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('deleteVoiceProfile -> delete_voice_profile', async () => {
    const mod = await import('../../src/data/voiceProfiles')
    await mod.deleteVoiceProfile('id-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_voice_profile')
    expect(invoke.mock.calls[0][1]).toHaveProperty('profileId')
  })
})

describe('data/webpage', () => {
  it('importWebpage -> import_webpage', async () => {
    const mod = await import('../../src/data/webpage')
    await mod.importWebpage('ws-1', 'test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('import_webpage')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
    expect(invoke.mock.calls[0][1]).toHaveProperty('url')
  })
})

describe('data/workspaces', () => {
  it('loadSelectedWorkspace is callable', async () => {
    const mod = await import('../../src/data/workspaces')
    const result = await callNoThrow(() => mod.loadSelectedWorkspace())
    expect(result.threw).toBe(false)
  })
  it('saveSelectedWorkspace is callable', async () => {
    const mod = await import('../../src/data/workspaces')
    const result = await callNoThrow(() => mod.saveSelectedWorkspace('test'))
    expect(result.threw).toBe(false)
  })
})

describe('data/workspacesAdmin', () => {
  it('createWorkspace -> create_workspace', async () => {
    const mod = await import('../../src/data/workspacesAdmin')
    await mod.createWorkspace('Test')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('create_workspace')
    expect(invoke.mock.calls[0][1]).toHaveProperty('name')
  })
  it('deleteWorkspace -> delete_workspace', async () => {
    const mod = await import('../../src/data/workspacesAdmin')
    await mod.deleteWorkspace('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_workspace')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('deleteWorkspaceComplete -> delete_workspace_complete', async () => {
    const mod = await import('../../src/data/workspacesAdmin')
    await mod.deleteWorkspaceComplete('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('delete_workspace_complete')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
  it('previewWorkspaceDeletion -> preview_workspace_deletion', async () => {
    const mod = await import('../../src/data/workspacesAdmin')
    await mod.previewWorkspaceDeletion('ws-1')
    expect(invoke).toHaveBeenCalled()
    expect(invoke.mock.calls[0][0]).toBe('preview_workspace_deletion')
    expect(invoke.mock.calls[0][1]).toHaveProperty('workspaceId')
  })
})

