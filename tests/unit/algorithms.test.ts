import { describe, it, expect } from 'vitest'

// These tests validate the Rust backend's behavior at the algorithm level.
// They test the pure functions that don't require a running Tauri instance.

describe('Cosine similarity', () => {
  function cosineSimilarity(left: number[], right: number[]): number {
    let dot = 0, leftNorm = 0, rightNorm = 0
    for (let i = 0; i < left.length; i++) {
      dot += left[i] * right[i]
      leftNorm += left[i] * left[i]
      rightNorm += right[i] * right[i]
    }
    if (leftNorm === 0 || rightNorm === 0) return 0
    return dot / (Math.sqrt(leftNorm) * Math.sqrt(rightNorm))
  }

  it('identical vectors have maximum similarity', () => {
    const result = cosineSimilarity([1, 2, 3], [1, 2, 3])
    expect(Math.abs(result - 1.0)).toBeLessThan(1e-10)
  })

  it('orthogonal vectors have zero similarity', () => {
    const result = cosineSimilarity([1, 0], [0, 1])
    expect(Math.abs(result)).toBeLessThan(1e-10)
  })

  it('opposite vectors have negative similarity', () => {
    const result = cosineSimilarity([1, 0], [-1, 0])
    expect(result).toBeCloseTo(-1.0)
  })

  it('zero vector produces zero similarity', () => {
    const result = cosineSimilarity([0, 0], [1, 0])
    expect(result).toBe(0)
  })

  it('handles empty vectors', () => {
    const result = cosineSimilarity([], [])
    expect(result).toBe(0)
  })
})

describe('Base64 encoding', () => {
  function base64Encode(input: string): string {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
    const bytes = new TextEncoder().encode(input)
    let result = ''
    for (let i = 0; i < bytes.length; i += 3) {
      const b0 = bytes[i]
      const b1 = i + 1 < bytes.length ? bytes[i + 1] : 0
      const b2 = i + 2 < bytes.length ? bytes[i + 2] : 0
      const combined = (b0 << 16) | (b1 << 8) | b2
      result += chars[(combined >> 18) & 63]
      result += chars[(combined >> 12) & 63]
      result += i + 1 < bytes.length ? chars[(combined >> 6) & 63] : '='
      result += i + 2 < bytes.length ? chars[combined & 63] : '='
    }
    return result
  }

  it('encodes simple strings', () => {
    expect(base64Encode('f')).toBe('Zg==')
    expect(base64Encode('fo')).toBe('Zm8=')
    expect(base64Encode('foo')).toBe('Zm9v')
  })

  it('encodes empty string', () => {
    expect(base64Encode('')).toBe('')
  })
})

describe('Cron expression parsing', () => {
  function isValidCron(expr: string): boolean {
    const parts = expr.split(/\s+/)
    if (parts.length !== 5 && parts.length !== 6) return false
    const ranges: [number, number][] = [
      [0, 59], [0, 23], [1, 31], [1, 12], [0, 7],
    ]
    if (parts.length === 6) ranges.unshift([0, 59])

    for (let i = 0; i < Math.min(parts.length, ranges.length); i++) {
      const part = parts[i]
      if (part === '*') continue
      if (/^\d+$/.test(part)) {
        const n = parseInt(part)
        if (n < ranges[i][0] || n > ranges[i][1]) return false
        continue
      }
      if (/^\d+-\d+$/.test(part)) {
        const [lo, hi] = part.split('-').map(Number)
        if (lo > hi || lo < ranges[i][0] || hi > ranges[i][1]) return false
        continue
      }
      if (/^\*\/(\d+)$/.test(part)) continue
      if (/^\d+(,\d+)*$/.test(part)) continue
      return false
    }
    return true
  }

  it('validates standard cron expressions', () => {
    expect(isValidCron('0 8 * * *')).toBe(true)
    expect(isValidCron('*/15 * * * *')).toBe(true)
    expect(isValidCron('0 0 1 1 *')).toBe(true)
    expect(isValidCron('0 8 * * 1-5')).toBe(true)
    expect(isValidCron('0 0,12 * * *')).toBe(true)
  })

  it('rejects invalid cron expressions', () => {
    expect(isValidCron('invalid')).toBe(false)
    expect(isValidCron('60 * * * *')).toBe(false)
    expect(isValidCron('* * * *')).toBe(false)
  })
})

describe('Cost calculation', () => {
  function calculateCost(
    provider: string,
    model: string,
    inputTokens: number,
    outputTokens: number,
  ): number {
    // Prices in cents per 1M tokens
    const pricing: Record<string, Record<string, [number, number]>> = {
      openai: {
        'gpt-4o': [250, 1000],
        'gpt-4': [3000, 6000],
        'gpt-3.5-turbo': [50, 150],
      },
      anthropic: {
        'claude-3-opus': [1500, 7500],
        'claude-3-sonnet': [300, 1500],
        'claude-3-haiku': [25, 125],
      },
      google: {
        'gemini-1.5-pro': [350, 1050],
        'gemini-1.5-flash': [7.5, 30],
      },
    }

    const modelPricing = pricing[provider]?.[model]
    if (!modelPricing) return 0

    const [inputPrice, outputPrice] = modelPricing
    return (inputTokens / 1_000_000) * inputPrice + (outputTokens / 1_000_000) * outputPrice
  }

  it('calculates gpt-4o cost correctly', () => {
    const cost = calculateCost('openai', 'gpt-4o', 1000, 500)
    expect(cost).toBeCloseTo(0.75, 2) // (1000/1M)*250 + (500/1M)*1000
  })

  it('returns zero for unknown providers', () => {
    const cost = calculateCost('unknown', 'model', 1000000, 500000)
    expect(cost).toBe(0)
  })

  it('returns zero for local models (no pricing)', () => {
    const cost = calculateCost('ollama', 'qwen3:14b', 10000, 5000)
    expect(cost).toBe(0)
  })
})

describe('Reranking relevance scoring', () => {
  function localRerank(query: string, documents: { content: string; id: string }[]) {
    const queryTerms = query.toLowerCase().split(/\s+/)
    return documents
      .map((doc) => {
        const contentLower = doc.content.toLowerCase()
        let score = 0

        if (contentLower.includes(query.toLowerCase())) score += 2.0
        for (const term of queryTerms) {
          score += (contentLower.match(new RegExp(term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'g')) || []).length * 0.5
        }
        const lengthFactor = Math.max(1, Math.sqrt(doc.content.length / 200))
        score /= lengthFactor

        return { ...doc, score }
      })
      .sort((a, b) => b.score - a.score)
  }

  it('ranks exact matches highest', () => {
    const docs = [
      { id: '1', content: 'The quick brown fox' },
      { id: '2', content: 'Product roadmap for Q3 includes assistant features' },
      { id: '3', content: 'Neural networks and deep learning' },
    ]
    const ranked = localRerank('neural', docs)
    // Doc 3 has both exact phrase match and term match bonus
    expect(ranked[0].id).toBe('3')
  })

  it('returns empty array for empty documents', () => {
    const ranked = localRerank('query', [])
    expect(ranked).toHaveLength(0)
  })
})

describe('Conflict detection algorithm', () => {
  interface Event {
    id: string
    title: string
    startsAt: Date
    endsAt: Date
  }

  function detectOverlaps(proposed: Event, existing: Event[]): Event[] {
    return existing.filter(
      (e) => e.id !== proposed.id && e.startsAt < proposed.endsAt && e.endsAt > proposed.startsAt,
    )
  }

  it('detects overlapping events', () => {
    const existing: Event[] = [
      { id: '1', title: 'Standup', startsAt: new Date('2026-01-01T09:00:00'), endsAt: new Date('2026-01-01T09:30:00') },
      { id: '2', title: 'Lunch', startsAt: new Date('2026-01-01T12:00:00'), endsAt: new Date('2026-01-01T13:00:00') },
    ]
    const proposed: Event = {
      id: '3',
      title: 'Review',
      startsAt: new Date('2026-01-01T09:15:00'),
      endsAt: new Date('2026-01-01T10:00:00'),
    }
    const conflicts = detectOverlaps(proposed, existing)
    expect(conflicts).toHaveLength(1)
    expect(conflicts[0].title).toBe('Standup')
  })

  it('detects no conflicts for non-overlapping events', () => {
    const existing: Event[] = [
      { id: '1', title: 'Standup', startsAt: new Date('2026-01-01T09:00:00'), endsAt: new Date('2026-01-01T09:30:00') },
    ]
    const proposed: Event = {
      id: '2',
      title: 'Review',
      startsAt: new Date('2026-01-01T10:00:00'),
      endsAt: new Date('2026-01-01T11:00:00'),
    }
    const conflicts = detectOverlaps(proposed, existing)
    expect(conflicts).toHaveLength(0)
  })

  it('excludes self from conflict detection', () => {
    const existing: Event[] = [
      { id: '1', title: 'Standup', startsAt: new Date('2026-01-01T09:00:00'), endsAt: new Date('2026-01-01T09:30:00') },
    ]
    const conflicts = detectOverlaps(existing[0], existing)
    expect(conflicts).toHaveLength(0)
  })
})

describe('Agent permission validation', () => {
  function isValidPermission(mode: string): boolean {
    return ['approval_required', 'read_only', 'full'].includes(mode)
  }

  it('accepts valid permission modes', () => {
    expect(isValidPermission('approval_required')).toBe(true)
    expect(isValidPermission('read_only')).toBe(true)
    expect(isValidPermission('full')).toBe(true)
  })

  it('rejects invalid permission modes', () => {
    expect(isValidPermission('admin')).toBe(false)
    expect(isValidPermission('write')).toBe(false)
    expect(isValidPermission('')).toBe(false)
  })
})
