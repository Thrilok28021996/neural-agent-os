use rusqlite::{params, Connection};
use serde::Serialize;
use chrono::Utc;

#[derive(Serialize, Debug)]
pub struct MonitoredFolder {
    pub id: String,
    pub workspace_id: String,
    pub path: String,
    pub enabled: bool,
    pub last_checked_at: Option<String>,
    pub files_found: i64,
}

#[derive(Serialize)]
pub struct MonitorResult {
    pub folder_id: String,
    pub new_files: usize,
    pub indexed_files: usize,
}

/// Register a folder for periodic monitoring
pub fn add_monitored_folder(
    connection: &Connection,
    workspace_id: &str,
    path: &str,
) -> Result<MonitoredFolder, String> {
    let canonical = std::path::PathBuf::from(path);
    if !canonical.is_dir() {
        return Err(format!("Folder does not exist: {path}"));
    }

    let id = format!("monitor-{}", uuid::Uuid::new_v4());
    connection
        .execute(
            "INSERT INTO monitored_folders (id, workspace_id, path, enabled)
             VALUES (?1, ?2, ?3, 1)",
            params![id, workspace_id, path],
        )
        .map_err(|e| e.to_string())?;

    Ok(MonitoredFolder {
        id,
        workspace_id: workspace_id.to_string(),
        path: path.to_string(),
        enabled: true,
        last_checked_at: None,
        files_found: 0,
    })
}

/// List monitored folders for a workspace
pub fn list_monitored_folders(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<MonitoredFolder>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, path, enabled, last_checked_at,
             (SELECT COUNT(*) FROM sources WHERE uri LIKE path || '%') as files_found
             FROM monitored_folders WHERE workspace_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = statement
        .query_map(params![workspace_id], |row| {
            Ok(MonitoredFolder {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                path: row.get(2)?,
                enabled: row.get(3)?,
                last_checked_at: row.get(4)?,
                files_found: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Toggle monitoring on/off
pub fn toggle_monitored_folder(
    connection: &Connection,
    folder_id: &str,
    enabled: bool,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE monitored_folders SET enabled = ?1 WHERE id = ?2",
            params![enabled, folder_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove a monitored folder
pub fn remove_monitored_folder(
    connection: &Connection,
    folder_id: &str,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM monitored_folders WHERE id = ?1",
            params![folder_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Check monitored folders for new files and index them.
/// Called by the scheduler periodically.
pub fn check_monitored_folders(
    app: &tauri::AppHandle,
    connection: &Connection,
) -> Result<Vec<MonitorResult>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, path FROM monitored_folders WHERE enabled = 1",
        )
        .map_err(|e| e.to_string())?;

    let folders: Vec<(String, String, String)> = statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    let supported = ["pdf", "md", "markdown", "txt", "docx", "html", "htm", "json", "csv"];

    for (folder_id, workspace_id, path) in &folders {
        let root = std::path::PathBuf::from(path);
        if !root.is_dir() {
            continue;
        }

        let mut new_files = 0;
        for entry in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            let entry_path = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            let extension = entry_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !supported.contains(&extension.to_lowercase().as_str()) {
                continue;
            }

            let uri = entry_path.to_string_lossy().into_owned();
            let source_id = format!("source-{}", uuid::Uuid::new_v4());
            let title = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled");

            let inserted = connection
                .execute(
                    "INSERT OR IGNORE INTO sources (id, workspace_id, kind, title, uri, status)
                     VALUES (?1, ?2, 'file', ?3, ?4, 'pending')",
                    params![source_id, workspace_id, title, uri],
                )
                .map_err(|e| e.to_string())?;

            if inserted > 0 {
                new_files += 1;
            }
        }

        connection
            .execute(
                "UPDATE monitored_folders SET last_checked_at = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), folder_id],
            )
            .map_err(|e| e.to_string())?;

        // Auto-index new sources
        let mut indexed = 0;
        if new_files > 0 {
            indexed = super::index_sources(app.clone(), workspace_id.clone())
                .map(|r| r.chunks_indexed)
                .unwrap_or(0);
        }

        results.push(MonitorResult {
            folder_id: folder_id.clone(),
            new_files,
            indexed_files: indexed,
        });
    }

    Ok(results)
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
    fn monitored_folder_crud() {
        let conn = setup();
        let dir = std::env::temp_dir().join(format!("nao-mon-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let folder = add_monitored_folder(&conn, "w1", dir.to_str().unwrap()).unwrap();
        assert!(folder.enabled);

        let folders = list_monitored_folders(&conn, "w1").unwrap();
        assert_eq!(folders.len(), 1);

        toggle_monitored_folder(&conn, &folder.id, false).unwrap();
        let folders = list_monitored_folders(&conn, "w1").unwrap();
        assert!(!folders[0].enabled);

        remove_monitored_folder(&conn, &folder.id).unwrap();
        assert!(list_monitored_folders(&conn, "w1").unwrap().is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nonexistent_folder_is_rejected() {
        let conn = setup();
        let result = add_monitored_folder(&conn, "w1", "/nonexistent/nao-test-xyz");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }
}
