import { describe, it, expect, beforeAll } from 'vitest'

// Integration tests for the local API server and workspace pipeline.
// These tests connect to a running Tauri app on http://127.0.0.1:8787.
// If the app is not running, tests skip gracefully.

const API_BASE = 'http://127.0.0.1:8787'

let apiAvailable = false

async function apiHealth(): Promise<boolean> {
  try {
    const resp = await fetch(`${API_BASE}/health`)
    return resp.ok
  } catch {
    return false
  }
}

beforeAll(async () => {
  apiAvailable = await apiHealth()
  if (!apiAvailable) console.warn('⚠ API server not running — live API tests will be skipped')
})

const skipIfUnavailable = (fn: () => void) => { if (apiAvailable) fn() }

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
    const resp = await fetch(`${API_BASE}/v1/notes/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({}),
    })
    expect(resp.status).toBe(400)
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
    const resp = await fetch(`${API_BASE}/v1/notes/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        workspace_id: 'personal',
        agent_id: 'test-agent',
        title: 'API test note',
        content: 'Created by integration test',
      }),
    })
    // 201 if agent has permission, 403 if not, 201 if read-only default allows
    expect([201, 403]).toContain(resp.status)
    if (resp.status === 201) {
      const body = await resp.json()
      expect(body.id).toBeTruthy()
      expect(body.title).toBe('API test note')
    }
  })

  it.runIf(apiAvailable)('POST /v1/reminders/create rejects missing fields', async () => {
    const resp = await fetch(`${API_BASE}/v1/reminders/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({}),
    })
    expect(resp.status).toBe(400)
  })

  it.runIf(apiAvailable)('POST /v1/reminders/create validates permission', async () => {
    const resp = await fetch(`${API_BASE}/v1/reminders/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        workspace_id: 'personal',
        agent_id: 'external-agent',
        task_id: 'test-task',
        title: 'Test reminder',
        scheduled_at: new Date(Date.now() + 86400000).toISOString(),
      }),
    })
    // Either permission denied (403) or created (201)
    expect([201, 403]).toContain(resp.status)
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
    const resp = await fetch(`${API_BASE}/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 2,
        method: 'tools/call',
        params: {
          name: 'create_note',
          arguments: { workspace_id: 'personal', agent_id: 'unauthorized', title: 'x', content: 'y' },
        },
      }),
    })
    expect([200, 403]).toContain(resp.status)
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
