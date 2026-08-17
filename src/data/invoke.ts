import { invoke as rawInvoke } from '@tauri-apps/api/core'
import type { InvokeArgs, InvokeOptions } from '@tauri-apps/api/core'

// ── Application-wide command logging ───────────────────────────────────────
// Every backend command in the app funnels through this wrapper, so all
// invocations (command name, args, duration, result/error) land in the
// on-disk application log (Diagnostics → Logs). Errors are re-thrown
// unchanged, so behaviour is identical to calling the backend directly.

let logReady = false
// Probe the backend logger once at startup (skipped under vitest, where the
// core module is mocked and any extra call would shift mock call indices).
if ((import.meta as { env?: { MODE?: string } }).env?.MODE !== 'test') {
  try {
    rawInvoke<string>('app_log_path')
      .then(() => { logReady = true })
      .catch(() => { logReady = false })
  } catch { /* backend not up yet */ }
}

function describe(value: unknown): string {
  if (value === undefined || value === null) return String(value)
  const text = typeof value === 'string' ? value : JSON.stringify(value)
  return text && text.length > 300 ? `${text.slice(0, 300)}…` : text
}

export function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  const started = performance.now()
  // Match the raw invoke call shape exactly (no trailing undefined options),
  // so mocks and tests observe identical argument lists.
  const call = args === undefined ? rawInvoke<T>(cmd) : options === undefined ? rawInvoke<T>(cmd, args) : rawInvoke<T>(cmd, args, options)
  // Mock/tests or a not-yet-ready backend may return a non-promise; behave
  // exactly like the raw invoke in that case.
  if (!call || typeof call.then !== 'function') return call
  // Never log the logging commands themselves (avoids recursion + noise).
  const isLogCommand = cmd === 'log_event' || cmd === 'read_app_log' || cmd === 'app_log_path'
  call.then(
    (result) => {
      if (!logReady || isLogCommand) return
      const ms = Math.round(performance.now() - started)
      void rawInvoke('log_event', { level: 'debug', source: 'invoke', message: `${cmd} ok (${ms}ms) → ${describe(result)}` }).catch(() => {})
    },
    (error: unknown) => {
      if (!logReady || isLogCommand) return
      const ms = Math.round(performance.now() - started)
      void rawInvoke('log_event', { level: 'error', source: 'invoke', message: `${cmd} failed (${ms}ms): ${describe(error)}` }).catch(() => {})
    },
  )
  return call
}
