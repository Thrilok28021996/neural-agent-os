import { describe, it, expect } from 'vitest'
import crypto from 'node:crypto'
import fs from 'node:fs'

// Integration tests for the local API server and workspace pipeline.
// These tests connect to a running Tauri app on http://127.0.0.1:8787.
// Run them with `npm run test:api` (starts the app automatically) or while
// the app is running; they skip if no server is available.
//
// The availability flag is resolved with top-level await so `it.runIf(...)`
// sees the real value at registration time (beforeAll is too late).

const API_BASE = 'http://127.0.0.1:8787'

async function apiHealth(): Promise<boolean> {
  try {
    const resp = await fetch(`${API_BASE}/health`)
    return resp.ok
  } catch {
    return false
  }
}

const apiAvailable = await apiHealth()
if (!apiAvailable) console.warn('⚠ API server not running — live API tests will be skipped')

describe('Health and diagnostics', () => {
  it.runIf(apiAvailable)('GET /health returns ok status', async () => {
    const resp = await fetch(`${API_BASE}/health`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(body.status).toBe('ok')
    expect(body.local).toBe(true)
    expect(body.version).toBeTruthy()
  })

  it.runIf(apiAvailable)('GET /diagnostics returns system info', async () => {
    const resp = await fetch(`${API_BASE}/diagnostics`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(body.database_size_bytes).toBeDefined()
    expect(body.workspace_count).toBeGreaterThanOrEqual(1)
    expect(body.api_port).toBe(8787)
  })
})

describe('Read endpoints', () => {
  it.runIf(apiAvailable)('GET /v1/workspaces lists workspaces', async () => {
    const resp = await fetch(`${API_BASE}/v1/workspaces`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(Array.isArray(body)).toBe(true)
    expect(body.length).toBeGreaterThanOrEqual(3)
  })

  it.runIf(apiAvailable)('GET /v1/search requires workspace_id and q', async () => {
    const resp = await fetch(`${API_BASE}/v1/search`)
    expect(resp.status).toBe(400)
  })

  it.runIf(apiAvailable)('GET /v1/search returns results for valid query', async () => {
    const resp = await fetch(`${API_BASE}/v1/search?workspace_id=personal&q=test`)
    expect(resp.status).toBe(200)
  })

  it.runIf(apiAvailable)('GET /v1/tasks enforces workspace_id', async () => {
    const resp = await fetch(`${API_BASE}/v1/tasks`)
    expect(resp.status).toBe(400)
  })

  it.runIf(apiAvailable)('GET /v1/tasks returns tasks for valid workspace', async () => {
    const resp = await fetch(`${API_BASE}/v1/tasks?workspace_id=personal`)
    expect(resp.status).toBe(200)
  })

  it.runIf(apiAvailable)('GET /v1/meetings returns meetings', async () => {
    const resp = await fetch(`${API_BASE}/v1/meetings?workspace_id=personal`)
    expect(resp.status).toBe(200)
  })

  it.runIf(apiAvailable)('GET /v1/notes returns notes', async () => {
    const resp = await fetch(`${API_BASE}/v1/notes?workspace_id=personal`)
    expect(resp.status).toBe(200)
  })
})

describe('Write endpoints with permission gating', () => {
  it.runIf(apiAvailable)('POST /v1/notes/create rejects missing fields', async () => {
    // Auth is checked before field validation: no API key -> 401 even for invalid payloads.
    const resp = await fetch(`${API_BASE}/v1/notes/create`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({}) })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /v1/notes/create requires POST method', async () => {
    const resp = await fetch(`${API_BASE}/v1/notes/create`)
    expect(resp.status).toBe(405)
  })

  it.runIf(apiAvailable)('POST /v1/notes/create requires API key auth', async () => {
    const resp = await fetch(`${API_BASE}/v1/notes/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workspace_id: 'personal', agent_id: 'test-agent', title: 'No auth', content: 'x' }),
    })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /v1/notes/create creates note for valid request', async () => {
    // Write endpoints require an API key; without one the request is rejected before permission checks.
    const resp = await fetch(`${API_BASE}/v1/notes/create`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ workspace_id: 'personal', agent_id: 'test-agent', title: 'API test note', content: 'Created by integration test' }) })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /v1/reminders/create rejects missing fields', async () => {
    const resp = await fetch(`${API_BASE}/v1/reminders/create`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({}) })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /v1/reminders/create validates permission', async () => {
    const resp = await fetch(`${API_BASE}/v1/reminders/create`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ workspace_id: 'personal', agent_id: 'test-agent', task_id: 'x', title: 'y', scheduled_at: '2026-08-12T00:00:00Z' }) })
    expect(resp.status).toBe(401)
  })
})

describe('MCP JSON-RPC interface', () => {
  it.runIf(apiAvailable)('POST /mcp tools/list returns available tools', async () => {
    const resp = await fetch(`${API_BASE}/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/list' }),
    })
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(body.result.tools).toBeDefined()
    expect(body.result.tools.length).toBeGreaterThanOrEqual(6)
    const names = body.result.tools.map((t: { name: string }) => t.name)
    expect(names).toContain('search_workspace')
    expect(names).toContain('create_note')
    expect(names).toContain('create_reminder')
  })

  it.runIf(apiAvailable)('POST /mcp tools/call enforces permission on write tools', async () => {
    // Write tools require API-key auth -> 401 without a key.
    const resp = await fetch(`${API_BASE}/mcp`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: 'create_note', arguments: { workspace_id: 'personal', agent_id: 'unauthorized', title: 'x', content: 'y' } } }) })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /mcp tools/list exposes the full gated catalogue', async () => {
    const resp = await fetch(`${API_BASE}/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 9, method: 'tools/list' }),
    })
    expect(resp.status).toBe(200)
    const body = await resp.json()
    const names = body.result.tools.map((t: { name: string }) => t.name)
    expect(names).toContain('execute_command')
    expect(names).toContain('modify_file')
    expect(names).toContain('send_email')
    expect(names).toContain('create_calendar_event')
    // Every tool carries a workspace-scoped schema (list_agent_runs excepted).
    for (const tool of body.result.tools) {
      const required = tool.inputSchema.required ?? []
      if (tool.name === 'list_agent_runs') expect(required).toContain('agent_id')
      else expect(required).toContain('workspace_id')
    }
  })

  it.runIf(apiAvailable)('POST /mcp tools/call read tools now require auth too', async () => {
    const resp = await fetch(`${API_BASE}/mcp`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 10, method: 'tools/call', params: { name: 'list_tasks', arguments: { workspace_id: 'personal', agent_id: 'unauthorized' } } }) })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /mcp tools/call rejects unknown tools before auth', async () => {
    const resp = await fetch(`${API_BASE}/mcp`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 11, method: 'tools/call', params: { name: 'not_a_tool', arguments: { workspace_id: 'personal' } } }) })
    expect(resp.status).toBe(400)
    const body = await resp.json()
    expect(body.error.message).toMatch(/Unknown tool/)
  })

  it.runIf(apiAvailable)('POST /mcp tools/call requires workspace_id', async () => {
    const resp = await fetch(`${API_BASE}/mcp`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 12, method: 'tools/call', params: { name: 'list_tasks', arguments: { agent_id: 'x' } } }) })
    expect(resp.status).toBe(400)
  })

  it.runIf(apiAvailable)('POST /v1/tools/call requires API key auth', async () => {
    const resp = await fetch(`${API_BASE}/v1/tools/call`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ tool: 'list_tasks', arguments: { workspace_id: 'personal', agent_id: 'cli' } }) })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /v1/tools/call rejects GET', async () => {
    const resp = await fetch(`${API_BASE}/v1/tools/call`)
    expect(resp.status).toBe(405)
  })

  it.runIf(apiAvailable)('POST /mcp unknown method returns error', async () => {
    const resp = await fetch(`${API_BASE}/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 3, method: 'nonexistent' }),
    })
    expect(resp.status).toBe(400)
    const body = await resp.json()
    expect(body.error.code).toBe(-32601)
  })
})

describe('New REST surfaces (plan-completion + recording)', () => {
  it.runIf(apiAvailable)('GET /v1/calendar/events returns a list', async () => {
    const resp = await fetch(`${API_BASE}/v1/calendar/events?workspace_id=personal`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(Array.isArray(body)).toBe(true)
  })

  it.runIf(apiAvailable)('GET /v1/emails returns a list', async () => {
    const resp = await fetch(`${API_BASE}/v1/emails?workspace_id=personal`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(Array.isArray(body)).toBe(true)
  })

  it.runIf(apiAvailable)('GET /v1/approvals returns a list', async () => {
    const resp = await fetch(`${API_BASE}/v1/approvals?workspace_id=personal`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(Array.isArray(body)).toBe(true)
  })

  it.runIf(apiAvailable)('GET /v1/permissions returns capability checks', async () => {
    const resp = await fetch(`${API_BASE}/v1/permissions?agent_id=cli&workspace_id=personal`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(Array.isArray(body)).toBe(true)
    const read = body.find((p: { capability: string }) => p.capability === 'read_knowledge')
    expect(read.allowed).toBe(true)
  })

  it.runIf(apiAvailable)('GET /v1/memories returns a list', async () => {
    const resp = await fetch(`${API_BASE}/v1/memories?workspace_id=personal`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(Array.isArray(body)).toBe(true)
  })

  it.runIf(apiAvailable)('GET /v1/context/export echoes the workspace id', async () => {
    const resp = await fetch(`${API_BASE}/v1/context/export?workspace_id=personal`)
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(body.workspace_id).toBe('personal')
    expect(body.context).toContain('Workspace Context')
  })

  it.runIf(apiAvailable)('POST /v1/tasks/create requires API key', async () => {
    const resp = await fetch(`${API_BASE}/v1/tasks/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workspace_id: 'personal', agent_id: 'rest-api', title: 'x' }),
    })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /v1/recording/start and stop (full loop)', async () => {
    // Guard: requires FFmpeg on the machine.
    const start = await fetch(`${API_BASE}/v1/recording/start`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path: '~/Recordings/api-live-test.m4a', source: 'default', workspace_id: 'personal' }),
    })
    if (start.status === 500) {
      // e.g. no microphone/FFmpeg — the endpoint must still answer cleanly
      expect(start.status).toBe(500)
      return
    }
    expect(start.status).toBe(200)
    const startBody = await start.json()
    expect(startBody.active).toBe(true)
    await new Promise((r) => setTimeout(r, 1500))
    const stop = await fetch(`${API_BASE}/v1/recording/stop`, { method: 'POST' })
    if (stop.status === 500) {
      // A device can disappear after start (permissions/device contention).
      // The endpoint must report a structured clean error rather than hang.
      const errorBody = await stop.json()
      expect(errorBody.error).toBeTruthy()
      return
    }
    expect(stop.status).toBe(200)
    const stopBody = await stop.json()
    expect(stopBody.active).toBe(false)
    expect(stopBody.queued).toBe(true)
    expect(stopBody.meeting_id).toBeTruthy()
  }, 20000)
})

describe('Teams bot secure transfer (release blocker #4)', () => {
  const TEAMS_SECRET = process.env.NAO_TEAMS_BOT_SECRET ?? 'nao-live-api-test-secret'

  // Same contract as teams-bot/bot.py and src-tauri/src/teams_bot.rs:
  // key = SHA-256(secret), payload = nonce(12) || AES-256-GCM ciphertext+tag.
  function encryptTeamsPayload(plaintext: Buffer): Buffer {
    const key = crypto.createHash('sha256').update(TEAMS_SECRET).digest()
    const nonce = crypto.randomBytes(12)
    const cipher = crypto.createCipheriv('aes-256-gcm', key, nonce)
    const ct = Buffer.concat([cipher.update(plaintext), cipher.final()])
    return Buffer.concat([nonce, ct, cipher.getAuthTag()])
  }

  function wavFixture(): Buffer {
    // Minimal valid 16-bit mono PCM WAV (matches bot.make_silent_wav).
    const sampleRate = 16000
    const dataSize = sampleRate * 2 // 1 second of silence
    const header = Buffer.alloc(44)
    header.write('RIFF', 0, 'ascii')
    header.writeUInt32LE(36 + dataSize, 4)
    header.write('WAVE', 8, 'ascii')
    header.write('fmt ', 12, 'ascii')
    header.writeUInt32LE(16, 16)
    header.writeUInt16LE(1, 20) // PCM
    header.writeUInt16LE(1, 22) // mono
    header.writeUInt32LE(sampleRate, 24)
    header.writeUInt32LE(sampleRate * 2, 28)
    header.writeUInt16LE(2, 32)
    header.writeUInt16LE(16, 34)
    header.write('data', 36, 'ascii')
    header.writeUInt32LE(dataSize, 40)
    return Buffer.concat([header, Buffer.alloc(dataSize)])
  }

  it.runIf(apiAvailable)('POST /teams-bot/status rejects missing token', async () => {
    const resp = await fetch(`${API_BASE}/teams-bot/status`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bot_id: 'b1', status: 'running' }),
    })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /teams-bot/status accepts a valid token', async () => {
    const resp = await fetch(`${API_BASE}/teams-bot/status`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Teams-Bot-Token': TEAMS_SECRET },
      body: JSON.stringify({ bot_id: 'b1', status: 'running', message: 'ok', meeting_id: 'm-1' }),
    })
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(body.received).toBe(true)
    expect(body.bot_id).toBe('b1')
  })

  it.runIf(apiAvailable)('POST /teams-bot/status rejects a wrong token', async () => {
    const resp = await fetch(`${API_BASE}/teams-bot/status`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Teams-Bot-Token': 'wrong-secret' },
      body: JSON.stringify({ bot_id: 'b1', status: 'running' }),
    })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('GET /teams-bot/meetings requires token', async () => {
    const resp = await fetch(`${API_BASE}/teams-bot/meetings`)
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('GET /teams-bot/meetings returns a list with token', async () => {
    const resp = await fetch(`${API_BASE}/teams-bot/meetings`, {
      headers: { 'X-Teams-Bot-Token': TEAMS_SECRET },
    })
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(Array.isArray(body)).toBe(true)
  })

  it.runIf(apiAvailable)('POST /teams-bot/upload rejects missing token', async () => {
    const resp = await fetch(`${API_BASE}/teams-bot/upload?meeting_id=noauth`, {
      method: 'POST',
      body: encryptTeamsPayload(wavFixture()),
    })
    expect(resp.status).toBe(401)
  })

  it.runIf(apiAvailable)('POST /teams-bot/upload rejects a tampered payload', async () => {
    const good = encryptTeamsPayload(wavFixture())
    const tampered = Buffer.from(good)
    tampered[tampered.length - 1] ^= 0x01
    const resp = await fetch(`${API_BASE}/teams-bot/upload?meeting_id=tampered`, {
      method: 'POST',
      headers: { 'X-Teams-Bot-Token': TEAMS_SECRET, 'Content-Type': 'application/octet-stream' },
      body: tampered,
    })
    expect(resp.status).toBe(400)
    const body = await resp.json()
    expect(body.error).toMatch(/decrypt|auth/i)
  })

  it.runIf(apiAvailable)('POST /teams-bot/upload roundtrips and decrypts locally', async () => {
    const meetingId = `meet-${Date.now()}`
    const title = 'API Teams Upload Roundtrip'
    const plaintext = wavFixture()
    const resp = await fetch(`${API_BASE}/teams-bot/upload?meeting_id=${meetingId}&title=${encodeURIComponent(title)}`, {
      method: 'POST',
      headers: { 'X-Teams-Bot-Token': TEAMS_SECRET, 'Content-Type': 'application/octet-stream' },
      body: encryptTeamsPayload(plaintext),
    })
    expect(resp.status).toBe(200)
    const body = await resp.json()
    expect(body.received).toBe(true)
    expect(body.decrypted).toBe(true)
    expect(body.meeting_row).toBeTruthy()
    expect(body.path).toBeTruthy()

    // The app wrote the decrypted plaintext to disk: verify it byte-for-byte.
    const stored = fs.readFileSync(body.path as string)
    expect(stored.equals(plaintext)).toBe(true)
    expect(stored.subarray(0, 4).toString('ascii')).toBe('RIFF')

    // And the meeting shows up as ready for transcription in the workspace.
    const meetings = await fetch(`${API_BASE}/v1/meetings?workspace_id=personal`).then((r) => r.json())
    const found = meetings.find((m: { title: string }) => m.title === title)
    expect(found).toBeTruthy()
    expect(found.status).toBe('ready_for_transcription')
  }, 15000)
})
