use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use std::sync::Mutex;

#[derive(Serialize, Clone)]
pub struct BackgroundJob {
    pub id: String,
    pub job_type: String,
    pub status: String, // queued, running, completed, failed, cancelled
    pub progress_pct: i32,
    pub progress_message: String,
    pub workspace_id: String,
    pub payload_json: String,
    pub error: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub locked_by: Option<String>,
}

#[derive(Serialize)]
pub struct JobQueueStatus {
    pub queued: usize,
    pub running: usize,
    pub completed_today: usize,
    pub failed: usize,
}

static JOB_LOCK: Mutex<()> = Mutex::new(());

/// Enqueue a new background job
pub fn enqueue_job(
    connection: &Connection,
    job_type: &str,
    workspace_id: &str,
    payload: &serde_json::Value,
    max_retries: i32,
) -> Result<BackgroundJob, String> {
    let id = format!("job-{}", uuid::Uuid::new_v4());
    let now = Utc::now().to_rfc3339();

    connection
        .execute(
            "INSERT INTO background_jobs (id, job_type, status, progress_pct, progress_message,
             workspace_id, payload_json, retry_count, max_retries, created_at)
             VALUES (?1, ?2, 'queued', 0, '', ?3, ?4, 0, ?5, ?6)",
            params![
                id,
                job_type,
                workspace_id,
                serde_json::to_string(payload).map_err(|e| e.to_string())?,
                max_retries,
                now,
            ],
        )
        .map_err(|e| e.to_string())?;

    Ok(BackgroundJob {
        id,
        job_type: job_type.to_string(),
        status: "queued".into(),
        progress_pct: 0,
        progress_message: String::new(),
        workspace_id: workspace_id.to_string(),
        payload_json: serde_json::to_string(payload).map_err(|e| e.to_string())?,
        error: None,
        retry_count: 0,
        max_retries,
        created_at: now,
        started_at: None,
        completed_at: None,
        locked_by: None,
    })
}

/// Update job progress
pub fn update_job_progress(
    connection: &Connection,
    job_id: &str,
    progress_pct: i32,
    message: &str,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE background_jobs SET progress_pct = ?1, progress_message = ?2 WHERE id = ?3",
            params![progress_pct, message, job_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Mark job as started (with locking)
pub fn start_job(
    connection: &Connection,
    job_id: &str,
    worker_id: &str,
) -> Result<bool, String> {
    let _lock = JOB_LOCK.lock().map_err(|e| e.to_string())?;
    let locked: bool = connection
        .query_row(
            "SELECT locked_by IS NULL OR locked_by = '' FROM background_jobs WHERE id = ?1 AND status = 'queued'",
            params![job_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !locked {
        return Ok(false);
    }

    connection
        .execute(
            "UPDATE background_jobs SET status = 'running', locked_by = ?1,
             started_at = CURRENT_TIMESTAMP WHERE id = ?2 AND status = 'queued'",
            params![worker_id, job_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Mark job as completed
pub fn complete_job(
    connection: &Connection,
    job_id: &str,
    output: Option<&str>,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE background_jobs SET status = 'completed', progress_pct = 100,
             progress_message = ?1, completed_at = CURRENT_TIMESTAMP, locked_by = NULL
             WHERE id = ?2",
            params![output.unwrap_or(""), job_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Mark job as failed (with retry logic)
pub fn fail_job(
    connection: &Connection,
    job_id: &str,
    error: &str,
) -> Result<String, String> {
    let (retry_count, max_retries): (i32, i32) = connection
        .query_row(
            "SELECT retry_count, max_retries FROM background_jobs WHERE id = ?1",
            params![job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if retry_count < max_retries {
        connection
            .execute(
                "UPDATE background_jobs SET status = 'queued', retry_count = retry_count + 1,
                 error = ?1, locked_by = NULL, progress_pct = 0 WHERE id = ?2",
                params![error, job_id],
            )
            .map_err(|e| e.to_string())?;
        Ok("retrying".into())
    } else {
        connection
            .execute(
                "UPDATE background_jobs SET status = 'failed', error = ?1,
                 completed_at = CURRENT_TIMESTAMP, locked_by = NULL WHERE id = ?2",
                params![error, job_id],
            )
            .map_err(|e| e.to_string())?;
        Ok("failed_permanently".into())
    }
}

/// Cancel a queued or running job
pub fn cancel_job(connection: &Connection, job_id: &str) -> Result<bool, String> {
    let changed = connection
        .execute(
            "UPDATE background_jobs SET status = 'cancelled', completed_at = CURRENT_TIMESTAMP,
             locked_by = NULL WHERE id = ?1 AND status IN ('queued', 'running')",
            params![job_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(changed > 0)
}

/// List jobs with optional status filter
pub fn list_jobs(
    connection: &Connection,
    workspace_id: &str,
    status_filter: Option<&str>,
    limit: i64,
) -> Result<Vec<BackgroundJob>, String> {
    let sql = match status_filter {
        Some(_) => "SELECT id, job_type, status, progress_pct, progress_message, workspace_id,
                    payload_json, error, retry_count, max_retries, created_at, started_at,
                    completed_at, locked_by
                    FROM background_jobs
                    WHERE workspace_id = ?1 AND status = ?2
                    ORDER BY created_at DESC LIMIT ?3",
        None => "SELECT id, job_type, status, progress_pct, progress_message, workspace_id,
                 payload_json, error, retry_count, max_retries, created_at, started_at,
                 completed_at, locked_by
                 FROM background_jobs
                 WHERE workspace_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
    };

    let mut stmt = connection.prepare(sql).map_err(|e| e.to_string())?;

    let rows: Vec<BackgroundJob> = if let Some(s) = status_filter {
        stmt.query_map(params![workspace_id, s, limit], |row| {
            map_job_row(row)
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    } else {
        stmt.query_map(params![workspace_id, limit], |row| map_job_row(row))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    };

    Ok(rows)
}

fn map_job_row(
    row: &rusqlite::Row,
) -> rusqlite::Result<BackgroundJob> {
    Ok(BackgroundJob {
        id: row.get(0)?,
        job_type: row.get(1)?,
        status: row.get(2)?,
        progress_pct: row.get(3)?,
        progress_message: row.get(4)?,
        workspace_id: row.get(5)?,
        payload_json: row.get(6)?,
        error: row.get(7)?,
        retry_count: row.get(8)?,
        max_retries: row.get(9)?,
        created_at: row.get(10)?,
        started_at: row.get(11)?,
        completed_at: row.get(12)?,
        locked_by: row.get(13)?,
    })
}

/// Get job queue status summary
pub fn get_queue_status(
    connection: &Connection,
    workspace_id: &str,
) -> Result<JobQueueStatus, String> {
    let queued: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM background_jobs WHERE workspace_id = ?1 AND status = 'queued'",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let running: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM background_jobs WHERE workspace_id = ?1 AND status = 'running'",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let completed_today: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM background_jobs WHERE workspace_id = ?1 AND status = 'completed' AND DATE(completed_at) = DATE('now')",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let failed: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM background_jobs WHERE workspace_id = ?1 AND status = 'failed'",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(JobQueueStatus {
        queued,
        running,
        completed_today,
        failed,
    })
}

/// Process next queued job — called by the scheduler
pub fn process_next_job(
    app: &tauri::AppHandle,
    connection: &Connection,
    worker_id: &str,
) -> Result<Option<BackgroundJob>, String> {
    let job_id: Option<String> = connection
        .query_row(
            "SELECT id FROM background_jobs WHERE status = 'queued' AND (locked_by IS NULL OR locked_by = '')
             ORDER BY created_at ASC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let Some(job_id) = job_id else {
        return Ok(None);
    };

    if !start_job(connection, &job_id, worker_id)? {
        return Ok(None);
    }

    // Fetch the started job by id. (Passing the id as a status filter would
    // match nothing and leave the job stuck in `running`.)
    let job = connection
        .query_row(
            "SELECT id, job_type, status, progress_pct, progress_message, workspace_id,
             payload_json, error, retry_count, max_retries, created_at, started_at,
             completed_at, locked_by
             FROM background_jobs WHERE id = ?1",
            params![job_id],
            map_job_row,
        )
        .ok();

    // Process based on job type
    if let Some(ref j) = job {
        let result = match j.job_type.as_str() {
            "index_sources" => {
                update_job_progress(connection, &job_id, 50, "Indexing sources...")?;
                super::index_sources(app.clone(), j.workspace_id.clone())
                    .map(|_| Some("Indexing complete".to_string()))
                    .map_err(|e| e.to_string())
            }
            "embed_sources" => {
                update_job_progress(connection, &job_id, 30, "Creating embeddings...")?;
                let model = serde_json::from_str::<serde_json::Value>(&j.payload_json)
                    .ok()
                    .and_then(|v| v["model"].as_str().map(String::from))
                    .unwrap_or_else(|| "nomic-embed-text".into());
                super::embed_sources(app.clone(), j.workspace_id.clone(), model)
                    .map(|_| Some("Embeddings complete".to_string()))
                    .map_err(|e| e.to_string())
            }
            "transcription" => {
                update_job_progress(connection, &job_id, 20, "Transcribing...")?;
                let meeting_id = serde_json::from_str::<serde_json::Value>(&j.payload_json)
                    .ok()
                    .and_then(|v| v["meeting_id"].as_str().map(String::from))
                    .unwrap_or_default();
                if meeting_id.is_empty() {
                    return Err("Missing meeting_id in transcription job payload".into());
                }
                let result = super::transcribe_meeting(app, &meeting_id, "local-whisper")
                    .map(|segments| Some(format!("Transcription complete: {segments} segments")));
                // Auto-pipeline: once transcribed, queue the summarization step.
                // Jobs run one at a time (FIFO), so back-to-back recordings
                // transcribe and summarize sequentially.
                if result.is_ok() {
                    let payload = serde_json::json!({ "meeting_id": meeting_id });
                    let _ = enqueue_job(connection, "summarize_meeting", &j.workspace_id, &payload, 2);
                }
                result.map_err(|e| e.to_string())
            }
            "summarize_meeting" => {
                let meeting_id = serde_json::from_str::<serde_json::Value>(&j.payload_json)
                    .ok()
                    .and_then(|v| v["meeting_id"].as_str().map(String::from))
                    .unwrap_or_default();
                super::summarize_meeting(app.clone(), meeting_id, "qwen3:14b".into())
                    .map(|s| Some(s.summary))
                    .map_err(|e| e.to_string())
            }
            _ => {
                update_job_progress(connection, &job_id, 100, "Job type not implemented")?;
                Ok(Some("Job type not implemented".into()))
            }
        };

        match result {
            Ok(output) => {
                complete_job(connection, &job_id, output.as_deref())?;
            }
            Err(error) => {
                let disposition = fail_job(connection, &job_id, &error)?;
                if disposition == "failed_permanently" {
                    eprintln!("Job {job_id} permanently failed: {error}");
                }
            }
        }
    }

    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_connection;

    fn setup() -> Connection {
        let conn = test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        conn
    }

    #[test]
    fn job_lifecycle_completes() {
        let conn = setup();
        let job = enqueue_job(&conn, "transcription", "w1", &serde_json::json!({"meeting_id": "m1"}), 3).unwrap();
        assert_eq!(job.status, "queued");
        assert_eq!(job.max_retries, 3);

        assert!(start_job(&conn, &job.id, "worker-1").unwrap());
        update_job_progress(&conn, &job.id, 50, "Halfway").unwrap();
        complete_job(&conn, &job.id, Some("done")).unwrap();

        let jobs = list_jobs(&conn, "w1", None, 10).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "completed");
        assert_eq!(jobs[0].progress_pct, 100);
    }

    #[test]
    fn job_failure_and_cancel() {
        let conn = setup();
        let job = enqueue_job(&conn, "transcription", "w1", &serde_json::json!({}), 0).unwrap();
        let outcome = fail_job(&conn, &job.id, "boom").unwrap();
        assert_eq!(outcome, "failed_permanently");
        let jobs = list_jobs(&conn, "w1", None, 10).unwrap();
        assert_eq!(jobs[0].status, "failed");
        assert_eq!(jobs[0].error.as_deref(), Some("boom"));

        let job2 = enqueue_job(&conn, "embeddings", "w1", &serde_json::json!({}), 2).unwrap();
        assert!(cancel_job(&conn, &job2.id).unwrap());
        assert!(!cancel_job(&conn, &job2.id).unwrap()); // already cancelled
    }

    #[test]
    fn queue_status_counts_by_state() {
        let conn = setup();
        let j1 = enqueue_job(&conn, "a", "w1", &serde_json::json!({}), 1).unwrap();
        let j2 = enqueue_job(&conn, "b", "w1", &serde_json::json!({}), 0).unwrap();
        start_job(&conn, &j1.id, "worker-1").unwrap();
        complete_job(&conn, &j1.id, None).unwrap();
        fail_job(&conn, &j2.id, "err").unwrap();

        let status = get_queue_status(&conn, "w1").unwrap();
        assert_eq!(status.completed_today, 1);
        assert_eq!(status.failed, 1);
        assert_eq!(status.running, 0);
    }
}
