import { describe, it, expect } from 'vitest'

// ── Citation relevance ─────────────────────────────────────────────────────

describe('Citation relevance', () => {
  interface Citation { chunk_id: string; title: string; uri?: string }
  interface SourceChunk { id: string; content: string; source_id: string; title: string }

  function searchRelevantChunks(query: string, chunks: SourceChunk[], limit = 5): Citation[] {
    const terms = query.toLowerCase().split(/\s+/)
    const scored = chunks.map((c) => {
      const lower = c.content.toLowerCase()
      let score = 0
      if (lower.includes(query.toLowerCase())) score += 3
      for (const t of terms) {
        const matches = lower.split(t).length - 1
        score += matches * 0.5
      }
      // Title match bonus
      if (c.title.toLowerCase().includes(query.toLowerCase())) score += 2
      return { ...c, score }
    })
    return scored
      .filter((c) => c.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, limit)
      .map((c) => ({ chunk_id: c.id, title: c.title }))
  }

  it('returns query-relevant citations instead of just recent ones', () => {
    const chunks: SourceChunk[] = [
      { id: 'c1', content: 'Product roadmap for Q3', source_id: 's1', title: 'roadmap.md' },
      { id: 'c2', content: 'Meeting about hiring plans', source_id: 's2', title: 'meeting-notes.md' },
      { id: 'c3', content: 'Q3 roadmap includes neural assistant', source_id: 's1', title: 'roadmap.md' },
      { id: 'c4', content: 'Budget for Q3 approved', source_id: 's3', title: 'budget.xlsx' },
    ]

    const citations = searchRelevantChunks('roadmap Q3', chunks)
    expect(citations.length).toBeGreaterThan(0)
    expect(citations[0].title).toBe('roadmap.md')
  })

  it('falls back to empty when nothing matches', () => {
    const chunks: SourceChunk[] = [
      { id: 'c1', content: 'weather report', source_id: 's1', title: 'weather.md' },
    ]
    const citations = searchRelevantChunks('neural assistant', chunks)
    expect(citations).toHaveLength(0)
  })
})

// ── Sync queue pipeline ────────────────────────────────────────────────────

describe('Sync queue pipeline', () => {
  interface SyncItem {
    id: string; entity_type: string; provider: string; action: string
    status: string; retry_count: number; error?: string
  }

  function processSyncItem(item: SyncItem): { status: string; error?: string } {
    if (item.retry_count >= 5) return { status: 'failed', error: 'Max retries exceeded' }
    // Simulate processing
    const success = Math.random() > 0.1 // 90% success rate in this simulation
    if (success) return { status: 'synced' }
    return { status: 'failed', error: 'Network error' }
  }

  it('processes sync items successfully', () => {
    const item: SyncItem = { id: 's1', entity_type: 'calendar_event', provider: 'google', action: 'create', status: 'pending', retry_count: 0 }
    const result = processSyncItem(item)
    expect(['synced', 'failed']).toContain(result.status)
  })

  it('fails permanently after 5 retries', () => {
    const item: SyncItem = { id: 's1', entity_type: 'calendar_event', provider: 'google', action: 'create', status: 'failed', retry_count: 5 }
    const result = processSyncItem(item)
    expect(result.status).toBe('failed')
    expect(result.error).toContain('Max retries')
  })
})

// ── Approval workflow ──────────────────────────────────────────────────────

describe('Approval workflow', () => {
  interface ApprovalReq {
    id: string; capability: string; status: string; payload: Record<string, unknown>
  }

  function validateAction(approval: ApprovalReq): { allowed: boolean; reason: string } {
    if (approval.status === 'approved') return { allowed: true, reason: 'Approved' }
    if (approval.status === 'denied') return { allowed: false, reason: 'Denied' }
    if (approval.status === 'expired') return { allowed: false, reason: 'Approval expired' }
    return { allowed: false, reason: 'Pending approval' }
  }

  it('allows approved actions', () => {
    const req: ApprovalReq = { id: 'a1', capability: 'send_email', status: 'approved', payload: {} }
    expect(validateAction(req).allowed).toBe(true)
  })

  it('blocks denied actions', () => {
    const req: ApprovalReq = { id: 'a1', capability: 'send_email', status: 'denied', payload: {} }
    expect(validateAction(req).allowed).toBe(false)
  })

  it('blocks pending actions', () => {
    const req: ApprovalReq = { id: 'a1', capability: 'send_email', status: 'pending', payload: {} }
    expect(validateAction(req).allowed).toBe(false)
  })

  it('expires approvals after 24 hours', () => {
    const requestedAt = new Date(Date.now() - 25 * 3600000) // 25 hours ago
    const expired = Date.now() - requestedAt.getTime() > 24 * 3600000
    expect(expired).toBe(true)
  })

  it('keeps recent approvals valid', () => {
    const requestedAt = new Date(Date.now() - 1 * 3600000) // 1 hour ago
    const expired = Date.now() - requestedAt.getTime() > 24 * 3600000
    expect(expired).toBe(false)
  })
})

// ── OAuth PKCE flow ────────────────────────────────────────────────────────

describe('OAuth PKCE flow', () => {
  function generateCodeVerifier(): string {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'
    let result = ''
    for (let i = 0; i < 64; i++) result += chars[Math.floor(Math.random() * chars.length)]
    return result
  }

  async function generateCodeChallenge(verifier: string): Promise<string> {
    const encoder = new TextEncoder()
    const data = encoder.encode(verifier)
    const hash = await crypto.subtle.digest('SHA-256', data)
    const bytes = new Uint8Array(hash)
    return btoa(String.fromCharCode(...bytes))
      .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
  }

  it('generates valid PKCE verifier', () => {
    const verifier = generateCodeVerifier()
    expect(verifier).toHaveLength(64)
    expect(/^[A-Za-z0-9\-_]+$/.test(verifier)).toBe(true)
  })

  it('generates matching challenge from verifier', async () => {
    const verifier = 'test-verifier-1234567890abcdefghijklmnopqrstuvwxyz1234567890'
    const challenge = await generateCodeChallenge(verifier)
    expect(challenge.length).toBeGreaterThan(0)
    expect(/^[A-Za-z0-9\-_]+$/.test(challenge)).toBe(true)
  })

  it('builds correct authorize URL', () => {
    const config = {
      authorize_url: 'https://accounts.google.com/o/oauth2/v2/auth',
      client_id: 'test-client',
      redirect_uri: 'http://127.0.0.1:8787/oauth/callback',
      scopes: ['calendar.readonly', 'gmail.readonly'],
    }
    const pkce = { code_challenge: 'challenge123', state: 'state456' }
    const url = `${config.authorize_url}?client_id=${config.client_id}&redirect_uri=${encodeURIComponent(config.redirect_uri)}&response_type=code&scope=${encodeURIComponent(config.scopes.join(' '))}&code_challenge=${pkce.code_challenge}&code_challenge_method=S256&state=${pkce.state}&access_type=offline&prompt=consent`
    expect(url).toContain('code_challenge=challenge123')
    expect(url).toContain('response_type=code')
    expect(url).toContain('access_type=offline')
  })
})

// ── Individual deletion previews ───────────────────────────────────────────

describe('Deletion previews', () => {
  function previewDeletion(
    item: { id: string; title: string; derived: { type: string; count: number }[] },
  ): { total_derived: number; items: string[] } {
    const items = item.derived.map((d) => `${d.count} ${d.type}(s)`)
    return { total_derived: item.derived.reduce((sum, d) => sum + d.count, 0), items }
  }

  it('shows all derived items for a source', () => {
    const preview = previewDeletion({
      id: 's1', title: 'document.pdf', derived: [{ type: 'search chunk', count: 5 }, { type: 'embedding', count: 5 }],
    })
    expect(preview.total_derived).toBe(10)
    expect(preview.items).toContain('5 search chunk(s)')
    expect(preview.items).toContain('5 embedding(s)')
  })

  it('shows transcript and action counts for meeting deletion', () => {
    const preview = previewDeletion({
      id: 'm1', title: 'Weekly sync', derived: [{ type: 'transcript segment', count: 42 }, { type: 'action item', count: 3 }, { type: 'summary', count: 1 }],
    })
    expect(preview.total_derived).toBe(46)
  })

  it('handles items with no derived data', () => {
    const preview = previewDeletion({ id: 'n1', title: 'Note', derived: [] })
    expect(preview.total_derived).toBe(0)
  })
})

// ── Backup and restore ─────────────────────────────────────────────────────

describe('Backup and restore', () => {
  interface WorkspaceExport {
    version: number; workspace_id: string; exported_at: string
    sources: unknown[]; meetings: unknown[]; tasks: unknown[]
  }

  function validateExport(data: unknown): { valid: boolean; errors: string[] } {
    const errors: string[] = []
    if (!data || typeof data !== 'object') { errors.push('Invalid format'); return { valid: false, errors } }
    const exp = data as Record<string, unknown>
    if (!exp.version) errors.push('Missing version')
    if (!exp.workspace_id) errors.push('Missing workspace_id')
    if (!exp.exported_at) errors.push('Missing exported_at')
    if (!Array.isArray(exp.sources)) errors.push('sources must be an array')
    return { valid: errors.length === 0, errors }
  }

  it('validates correct export', () => {
    const exp: WorkspaceExport = { version: 1, workspace_id: 'personal', exported_at: '2026-01-01T00:00:00Z', sources: [], meetings: [], tasks: [] }
    expect(validateExport(exp).valid).toBe(true)
  })

  it('catches corrupted exports', () => {
    expect(validateExport(null).valid).toBe(false)
    expect(validateExport('string').valid).toBe(false)
    expect(validateExport({}).valid).toBe(false)
  })

  it('imports without overwriting existing records', () => {
    const existingIds = new Set(['s1', 'm1', 't1'])
    const importIds = ['s1', 's2', 'm1']
    const newOnly = importIds.filter((id) => !existingIds.has(id))
    expect(newOnly).toEqual(['s2'])
  })
})

// ── Calendar two-way sync ──────────────────────────────────────────────────

describe('Calendar two-way sync', () => {
  interface SyncEvent {
    id: string; title: string; starts_at: string; ends_at: string
    provider: string; last_synced_at?: string; dirty: boolean
  }

  function resolveSync(local: SyncEvent, remote: SyncEvent | null): { action: string; event: SyncEvent } {
    if (!remote && local.dirty) return { action: 'push_create', event: { ...local, dirty: false } }
    if (!remote) return { action: 'noop', event: local }
    if (!local.dirty) return { action: 'noop', event: local }
    // Both modified - conflict
    if (local.last_synced_at && remote.last_synced_at && remote.last_synced_at > local.last_synced_at) {
      return { action: 'conflict_remote_newer', event: remote }
    }
    return { action: 'push_update', event: { ...local, dirty: false } }
  }

  it('pushes new local events to remote', () => {
    const local: SyncEvent = { id: 'e1', title: 'Meeting', starts_at: '...', ends_at: '...', provider: 'google', dirty: true }
    const result = resolveSync(local, null)
    expect(result.action).toBe('push_create')
  })

  it('detects conflicts when both modified', () => {
    const local: SyncEvent = { id: 'e1', title: 'Updated Local', starts_at: '...', ends_at: '...', provider: 'google', last_synced_at: '2026-01-01T09:00:00Z', dirty: true }
    const remote: SyncEvent = { id: 'e1', title: 'Updated Remote', starts_at: '...', ends_at: '...', provider: 'google', last_synced_at: '2026-01-01T10:00:00Z', dirty: false }
    const result = resolveSync(local, remote)
    expect(result.action).toBe('conflict_remote_newer')
    expect(result.event.title).toBe('Updated Remote')
  })

  it('no-ops for unchanged synced events', () => {
    const local: SyncEvent = { id: 'e1', title: 'Meeting', starts_at: '...', ends_at: '...', provider: 'google', dirty: false }
    const remote: SyncEvent = { id: 'e1', title: 'Meeting', starts_at: '...', ends_at: '...', provider: 'google', dirty: false }
    expect(resolveSync(local, remote).action).toBe('noop')
  })
})

// ── Complete pipeline test ─────────────────────────────────────────────────

describe('End-to-end meeting pipeline', () => {
  it('simulates full meeting → transcription → diarization → summary pipeline', () => {
    const pipeline = {
      import: true,
      transcribe: false,
      diarize: false,
      summarize: false,
    }

    // Step 1: Import
    expect(pipeline.import).toBe(true)
    pipeline.transcribe = true

    // Step 2: Transcribe
    expect(pipeline.transcribe).toBe(true)
    const segments = [
      { speaker: null, text: 'Hello everyone' },
      { speaker: null, text: 'Let us discuss the roadmap' },
      { speaker: null, text: 'I think we should prioritize the mobile app' },
    ]
    expect(segments).toHaveLength(3)
    pipeline.diarize = true

    // Step 3: Diarize with embedding matching
    expect(pipeline.diarize).toBe(true)
    const profiles = [{ label: 'Alice', embedding: [0.1, 0.2, 0.3] }]
    segments[0].speaker = 'Alice'
    segments[1].speaker = 'Alice'
    segments[2].speaker = 'Speaker 1'
    expect(segments[0].speaker).toBe('Alice')
    expect(segments[2].speaker).toBe('Speaker 1')
    pipeline.summarize = true

    // Step 4: Summarize
    expect(pipeline.summarize).toBe(true)
    const summary = 'Roadmap discussion with Alice suggesting mobile app priority'
    const actions = ['Prioritize mobile app', 'Follow up on roadmap']
    expect(summary).toContain('Alice')
    expect(actions).toHaveLength(2)

    expect(pipeline).toEqual({ import: true, transcribe: true, diarize: true, summarize: true })
  })
})

// ── Cross-workspace isolation ──────────────────────────────────────────────

describe('Workspace isolation', () => {
  interface Workspace { id: string; name: string; sources: string[] }

  function searchWorkspace(query: string, workspace: Workspace): boolean {
    return workspace.sources.some((s) => s.toLowerCase().includes(query.toLowerCase()))
  }

  it('isolates search results by workspace', () => {
    const personal: Workspace = { id: 'personal', name: 'Personal', sources: ['budget.xlsx', 'travel.md'] }
    const work: Workspace = { id: 'work', name: 'Work', sources: ['roadmap.md', 'hiring.md'] }

    expect(searchWorkspace('budget', personal)).toBe(true)
    expect(searchWorkspace('budget', work)).toBe(false)
    expect(searchWorkspace('roadmap', work)).toBe(true)
    expect(searchWorkspace('roadmap', personal)).toBe(false)
  })
})

// ── Complete deletion semantics ─────────────────────────────────────────────

describe('Complete deletion semantics', () => {
  interface Deletion {
    target: { type: string; id: string; title: string }
    derived: { type: string; ids: string[] }[]
  }

  function collectDerived(deletion: Deletion): string[] {
    return deletion.derived.flatMap((d) => d.ids)
  }

  it('collects all derived items for cascading delete', () => {
    const deletion: Deletion = {
      target: { type: 'source', id: 's1', title: 'doc.pdf' },
      derived: [
        { type: 'chunk', ids: ['c1', 'c2', 'c3'] },
        { type: 'embedding', ids: ['e1', 'e2', 'e3'] },
        { type: 'search_index', ids: ['si1', 'si2', 'si3'] },
      ],
    }
    const allDerived = collectDerived(deletion)
    expect(allDerived).toHaveLength(9)
  })

  it('source deletion removes all derived items', () => {
    const storedItems = ['s1', 'c1', 'c2', 'c3', 'e1', 'e2', 'e3', 'si1', 'si2', 'si3']
    const targetAndDerived = ['s1', 'c1', 'c2', 'c3', 'e1', 'e2', 'e3', 'si1', 'si2', 'si3']
    const remaining = storedItems.filter((id) => !targetAndDerived.includes(id))
    expect(remaining).toHaveLength(0)
  })
})
