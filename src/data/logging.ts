import { invoke } from './invoke'

/** Ship a message from the web view into the on-disk application log. */
export function logEvent(level: 'debug' | 'info' | 'warn' | 'error', source: string, message: string) {
  console[level === 'debug' ? 'debug' : level](`[${source}] ${message}`)
  void invoke('log_event', { level, source, message }).catch(() => { /* logging must never throw */ })
}

/** Last `limit` lines of the application log file (Diagnostics view). */
export function readAppLog(limit = 300) {
  return invoke<string>('read_app_log', { limit })
}

/** Absolute path of the application log file. */
export function appLogPath() {
  return invoke<string>('app_log_path')
}
