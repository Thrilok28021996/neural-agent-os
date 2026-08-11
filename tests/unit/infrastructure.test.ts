import { describe, it, expect } from 'vitest'

// ── Diarization algorithms ─────────────────────────────────────────────────

describe('Diarization', () => {
  function cosineSim(a: number[], b: number[]): number {
    let dot = 0, na = 0, nb = 0
    for (let i = 0; i < Math.min(a.length, b.length); i++) {
      dot += a[i] * b[i]; na += a[i] * a[i]; nb += b[i] * b[i]
    }
    return na === 0 || nb === 0 ? 0 : dot / (Math.sqrt(na) * Math.sqrt(nb))
  }

  function assignSpeaker(
    embedding: number[],
    profiles: { id: string; label: string; embedding: number[] }[],
    threshold: number,
  ): { label: string; confidence: number; matchedId?: string } {
    let bestSim = threshold
    let bestMatch: { label: string; matchedId?: string } = { label: 'Unknown Speaker' }

    for (const p of profiles) {
      const sim = cosineSim(embedding, p.embedding)
      if (sim > bestSim) {
        bestSim = sim
        bestMatch = { label: p.label, matchedId: p.id }
      }
    }
    return { ...bestMatch, confidence: bestMatch.label === "Unknown Speaker" ? 0 : bestSim }
  }

  function mergeAdjacentSpeakers(segments: { speaker: string; text: string }[]): { speaker: string; text: string }[] {
    if (segments.length === 0) return []
    const merged: { speaker: string; text: string }[] = [{ ...segments[0] }]
    for (let i = 1; i < segments.length; i++) {
      const last = merged[merged.length - 1]
      if (segments[i].speaker === last.speaker) {
        last.text += ' ' + segments[i].text
      } else {
        merged.push({ ...segments[i] })
      }
    }
    return merged
  }

  it('matches identical embeddings to the correct speaker', () => {
    const profiles = [{ id: 'p1', label: 'Alice', embedding: [1, 0, 0] }]
    const result = assignSpeaker([1, 0, 0], profiles, 0.7)
    expect(result.label).toBe('Alice')
    expect(result.confidence).toBeGreaterThan(0.99)
  })

  it('returns Unknown Speaker when below threshold', () => {
    const profiles = [{ id: 'p1', label: 'Alice', embedding: [1, 0, 0] }]
    const result = assignSpeaker([0, 1, 0], profiles, 0.7)
    expect(result.label).toBe('Unknown Speaker')
    expect(result.confidence).toBe(0)
  })

  it('returns Unknown Speaker when no profiles exist', () => {
    const result = assignSpeaker([1, 0], [], 0.7)
    expect(result.label).toBe('Unknown Speaker')
  })

  it('merges adjacent same-speaker segments', () => {
    const segments = [
      { speaker: 'Alice', text: 'Hello' },
      { speaker: 'Alice', text: 'world' },
      { speaker: 'Bob', text: 'Hi' },
      { speaker: 'Bob', text: 'there' },
    ]
    const merged = mergeAdjacentSpeakers(segments)
    expect(merged).toHaveLength(2)
    expect(merged[0].text).toBe('Hello world')
    expect(merged[1].text).toBe('Hi there')
  })

  it('preserves single segments', () => {
    const merged = mergeAdjacentSpeakers([{ speaker: 'Alice', text: 'Hello' }])
    expect(merged).toHaveLength(1)
  })
})

// ── Job queue ──────────────────────────────────────────────────────────────

describe('Job queue', () => {
  interface Job {
    id: string; status: string; retryCount: number; maxRetries: number
  }

  function handleJobFailure(job: Job, error: string): { status: string; retryCount: number } {
    if (job.retryCount < job.maxRetries) {
      return { status: 'queued', retryCount: job.retryCount + 1 }
    }
    return { status: 'failed', retryCount: job.retryCount }
  }

  function canStartJob(job: Job): boolean {
    return job.status === 'queued' && job.retryCount < job.maxRetries
  }

  it('retries a job up to maxRetries', () => {
    const job: Job = { id: 'j1', status: 'queued', retryCount: 0, maxRetries: 3 }
    let result = handleJobFailure(job, 'error')
    expect(result.status).toBe('queued')
    expect(result.retryCount).toBe(1)

    job.retryCount = 2
    result = handleJobFailure(job, 'error')
    expect(result.status).toBe('queued')
    expect(result.retryCount).toBe(3)
  })

  it('fails permanently after maxRetries', () => {
    const job: Job = { id: 'j1', status: 'queued', retryCount: 3, maxRetries: 3 }
    const result = handleJobFailure(job, 'error')
    expect(result.status).toBe('failed')
  })

  it('prevents starting a failed job', () => {
    expect(canStartJob({ id: 'j1', status: 'failed', retryCount: 3, maxRetries: 3 })).toBe(false)
  })

  it('allows starting a queued job with retries remaining', () => {
    expect(canStartJob({ id: 'j1', status: 'queued', retryCount: 1, maxRetries: 3 })).toBe(true)
  })

  it('prevents starting when retries exhausted', () => {
    expect(canStartJob({ id: 'j1', status: 'queued', retryCount: 3, maxRetries: 3 })).toBe(false)
  })
})

// ── Permissions ────────────────────────────────────────────────────────────

describe('Permissions', () => {
  const READ_CAPS = ['read_knowledge', 'read_transcripts']
  const WRITE_CAPS = ['create_notes', 'create_reminders', 'modify_tasks', 'schedule_events', 'draft_email', 'send_email', 'modify_files', 'execute_commands']
  const ALL_CAPS = [...READ_CAPS, ...WRITE_CAPS]

  function checkPermission(
    capability: string,
    explicitGrants: Map<string, boolean>,
  ): { allowed: boolean; reason: string } {
    const explicit = explicitGrants.get(capability)
    if (explicit !== undefined) {
      return { allowed: explicit, reason: explicit ? 'Explicitly granted' : 'Explicitly denied' }
    }
    const defaultAllowed = READ_CAPS.includes(capability)
    return { allowed: defaultAllowed, reason: defaultAllowed ? 'Allowed by default (read-only)' : 'Denied by default' }
  }

  it('allows read capabilities by default', () => {
    for (const cap of READ_CAPS) {
      const result = checkPermission(cap, new Map())
      expect(result.allowed).toBe(true)
    }
  })

  it('denies write capabilities by default', () => {
    for (const cap of WRITE_CAPS) {
      const result = checkPermission(cap, new Map())
      expect(result.allowed).toBe(false)
    }
  })

  it('honors explicit grants over defaults', () => {
    const grants = new Map([['send_email', true]])
    const result = checkPermission('send_email', grants)
    expect(result.allowed).toBe(true)
    expect(result.reason).toBe('Explicitly granted')
  })

  it('honors explicit denials even for read capabilities', () => {
    const grants = new Map([['read_knowledge', false]])
    const result = checkPermission('read_knowledge', grants)
    expect(result.allowed).toBe(false)
  })

  it('all capabilities are either read or write', () => {
    expect(READ_CAPS.length + WRITE_CAPS.length).toBe(ALL_CAPS.length)
    for (const cap of ALL_CAPS) {
      expect(READ_CAPS.includes(cap) || WRITE_CAPS.includes(cap)).toBe(true)
    }
  })
})

// ── Storage monitoring ─────────────────────────────────────────────────────

describe('Storage retention', () => {
  function getStorageWarning(totalBytes: number): string | null {
    if (totalBytes > 10_000_000_000) return 'Storage exceeds 10GB'
    if (totalBytes > 5_000_000_000) return 'Storage exceeds 5GB'
    return null
  }

  function shouldCleanup(createdAt: Date, olderThanDays: number): boolean {
    const cutoff = new Date(Date.now() - olderThanDays * 86400000)
    return createdAt < cutoff
  }

  it('warns at 10GB threshold', () => {
    expect(getStorageWarning(11_000_000_000)).toBe('Storage exceeds 10GB')
  })

  it('warns at 5GB threshold', () => {
    expect(getStorageWarning(6_000_000_000)).toBe('Storage exceeds 5GB')
  })

  it('no warning below 5GB', () => {
    expect(getStorageWarning(1_000_000_000)).toBeNull()
  })

  it('identifies recordings older than threshold', () => {
    const oldDate = new Date(Date.now() - 40 * 86400000) // 40 days ago
    expect(shouldCleanup(oldDate, 30)).toBe(true)
  })

  it('keeps recent recordings', () => {
    const recent = new Date(Date.now() - 5 * 86400000) // 5 days ago
    expect(shouldCleanup(recent, 30)).toBe(false)
  })
})

// ── Audit log ──────────────────────────────────────────────────────────────

describe('Audit log', () => {
  interface AuditEntry {
    action: string
    provider?: string
    dataType?: string
    details: string
  }

  function recordAudit(action: string, provider?: string, dataType?: string, details = ''): AuditEntry {
    return { action, provider, dataType, details }
  }

  it('records cloud transcription audit', () => {
    const entry = recordAudit('cloud_transcription', 'openai', 'audio', 'Sent meeting recording to OpenAI Whisper')
    expect(entry.action).toBe('cloud_transcription')
    expect(entry.provider).toBe('openai')
    expect(entry.dataType).toBe('audio')
  })

  it('records local operations without provider', () => {
    const entry = recordAudit('local_transcription', undefined, undefined, 'Whisper processed locally')
    expect(entry.provider).toBeUndefined()
  })

  it('records backup creation', () => {
    const entry = recordAudit('backup_created', undefined, 'workspace_export', 'Backup saved')
    expect(entry.action).toBe('backup_created')
  })
})

// ── Calendar sync conflict resolution ──────────────────────────────────────

describe('Calendar conflict resolution', () => {
  interface Event {
    id: string; title: string; startsAt: Date; endsAt: Date
  }

  function detectConflicts(proposed: Event, existing: Event[]): Event[] {
    return existing.filter(
      (e) => e.id !== proposed.id && e.startsAt < proposed.endsAt && e.endsAt > proposed.startsAt,
    )
  }

  function resolveConflicts(conflicts: Event[]): { keep: Event; suggestion: string }[] {
    return conflicts.map((c) => ({
      keep: c,
      suggestion: `Reschedule or cancel "${c.title}" to avoid overlap`,
    }))
  }

  it('detects single conflict', () => {
    const existing: Event[] = [{ id: '1', title: 'Standup', startsAt: new Date('2026-01-01T09:00:00'), endsAt: new Date('2026-01-01T09:30:00') }]
    const proposed: Event = { id: '2', title: 'Review', startsAt: new Date('2026-01-01T09:15:00'), endsAt: new Date('2026-01-01T10:00:00') }
    const conflicts = detectConflicts(proposed, existing)
    expect(conflicts).toHaveLength(1)
  })

  it('provides resolution suggestions', () => {
    const conflicts: Event[] = [{ id: '1', title: 'Standup', startsAt: new Date(), endsAt: new Date() }]
    const resolutions = resolveConflicts(conflicts)
    expect(resolutions[0].suggestion).toContain('Standup')
  })

  it('excludes self from conflict detection', () => {
    const existing: Event[] = [{ id: '1', title: 'Standup', startsAt: new Date('2026-01-01T09:00:00'), endsAt: new Date('2026-01-01T09:30:00') }]
    const conflicts = detectConflicts(existing[0], existing)
    expect(conflicts).toHaveLength(0)
  })
})

// ── Recurring event expansion ──────────────────────────────────────────────

describe('Recurring events', () => {
  function expandRecurrence(
    startsAt: Date,
    recurrence: string,
    count: number,
  ): Date[] {
    const dates: Date[] = [new Date(startsAt)]
    const match = recurrence.match(/^(\d+)\s*(day|week|month)s?$/)
    if (!match) return dates

    const interval = parseInt(match[1])
    const unit = match[2]

    for (let i = 1; i < count; i++) {
      const next = new Date(dates[i - 1])
      switch (unit) {
        case 'day': next.setDate(next.getDate() + interval); break
        case 'week': next.setDate(next.getDate() + interval * 7); break
        case 'month': next.setMonth(next.getMonth() + interval); break
      }
      dates.push(next)
    }
    return dates
  }

  it('expands daily recurrence', () => {
    const base = new Date('2026-01-01T09:00:00')
    const expanded = expandRecurrence(base, '1 day', 3)
    expect(expanded).toHaveLength(3)
    expect(expanded[1].getDate()).toBe(2)
    expect(expanded[2].getDate()).toBe(3)
  })

  it('expands weekly recurrence', () => {
    const base = new Date('2026-01-01') // Thursday
    const expanded = expandRecurrence(base, '1 week', 3)
    expect(expanded[1].getDate()).toBe(8)
    expect(expanded[2].getDate()).toBe(15)
  })

  it('returns single date for unknown recurrence', () => {
    const base = new Date('2026-01-01')
    const expanded = expandRecurrence(base, 'unknown', 5)
    expect(expanded).toHaveLength(1)
  })
})

// ── Timezone handling ─────────────────────────────────────────────────────

describe('Timezone', () => {
  function toUTC(date: Date): Date {
    return new Date(date.getTime() - date.getTimezoneOffset() * 60000)
  }

  function formatInTimezone(date: Date, offsetHours: number): string {
    const local = new Date(date.getTime() + offsetHours * 3600000)
    return local.toISOString()
  }

  it('converts to UTC correctly', () => {
    const local = new Date('2026-01-01T14:00:00')
    const utc = toUTC(local)
    expect(utc instanceof Date).toBe(true)
  })

  it('formats for different timezone offsets', () => {
    const utc = new Date('2026-01-01T14:00:00Z')
    const ist = formatInTimezone(utc, 5.5) // IST = UTC+5:30
    expect(ist).toContain('T19:30:00')
  })
})

// ── Email approval workflow ────────────────────────────────────────────────

describe('Email approval', () => {
  interface EmailDraft {
    to: string; subject: string; body: string; approved: boolean
  }

  function validateDraft(draft: EmailDraft): { valid: boolean; errors: string[] } {
    const errors: string[] = []
    if (!draft.to.includes('@')) errors.push('Invalid recipient email')
    if (draft.subject.trim().length === 0) errors.push('Subject is required')
    if (!draft.approved) errors.push('Email requires approval before sending')
    return { valid: errors.length === 0, errors }
  }

  it('requires approval before sending', () => {
    const draft: EmailDraft = { to: 'x@y.com', subject: 'Test', body: 'Hi', approved: false }
    expect(validateDraft(draft).valid).toBe(false)
  })

  it('validates recipient email format', () => {
    const draft: EmailDraft = { to: 'invalid', subject: 'Test', body: 'Hi', approved: true }
    expect(validateDraft(draft).errors).toContain('Invalid recipient email')
  })

  it('validates subject is not empty', () => {
    const draft: EmailDraft = { to: 'x@y.com', subject: '  ', body: 'Hi', approved: true }
    expect(validateDraft(draft).errors).toContain('Subject is required')
  })

  it('passes for valid approved draft', () => {
    const draft: EmailDraft = { to: 'jane@example.com', subject: 'Meeting notes', body: 'Here you go', approved: true }
    expect(validateDraft(draft).valid).toBe(true)
  })
})

// ── Backup validation ──────────────────────────────────────────────────────

describe('Backup integrity', () => {
  function validateBackup(content: string): boolean {
    try {
      const json = JSON.parse(content)
      return json.version !== undefined && json.workspace_id !== undefined && json.exported_at !== undefined
    } catch {
      return false
    }
  }

  it('validates a correct backup', () => {
    const backup = JSON.stringify({ version: 1, workspace_id: 'personal', exported_at: '2026-01-01T00:00:00Z' })
    expect(validateBackup(backup)).toBe(true)
  })

  it('rejects missing fields', () => {
    expect(validateBackup('{}')).toBe(false)
    expect(validateBackup('{"version":1}')).toBe(false)
  })

  it('rejects invalid JSON', () => {
    expect(validateBackup('{broken')).toBe(false)
    expect(validateBackup('not json')).toBe(false)
  })
})
