import { invoke } from './invoke'

export type BackgroundJob = {
  id: string
  job_type: string
  status: string
  progress_pct: number
  progress_message: string
  workspace_id: string
  payload_json: string
  error?: string
  retry_count: number
  max_retries: number
  created_at: string
  started_at?: string
  completed_at?: string
  locked_by?: string
}

export type JobQueueStatus = {
  queued: number
  running: number
  completed_today: number
  failed: number
}

export function enqueueJob(jobType: string, workspaceId: string, payload: Record<string, unknown>, maxRetries?: number) {
  return invoke<BackgroundJob>('enqueue_job', { jobType, workspaceId, payloadJson: JSON.stringify(payload), maxRetries })
}
export function listJobs(workspaceId: string, statusFilter?: string, limit?: number) {
  return invoke<BackgroundJob[]>('list_jobs', { workspaceId, statusFilter, limit })
}
export function cancelJob(jobId: string) { return invoke<boolean>('cancel_job', { jobId }) }
export function getQueueStatus(workspaceId: string) { return invoke<JobQueueStatus>('get_queue_status', { workspaceId }) }
