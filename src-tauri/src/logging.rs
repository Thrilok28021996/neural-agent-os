use chrono::Utc;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Application-wide logger: every line goes to a rotating file in the app
/// data directory (`neural-agent-os.log`) and to stderr, so failures are
/// visible both from a terminal and from the in-app Diagnostics → Logs view.
///
/// Usage: `log::info!("...")`, `log::warn!(...)`, `log::error!(...)`,
/// `log::debug!(...)` from any module. Initialize once at startup with
/// `logging::init(&data_dir)`.

const MAX_BYTES: u64 = 4 * 1024 * 1024; // rotate at 4 MB

struct LogFile {
    file: File,
    path: PathBuf,
}

static LOG_TARGET: OnceLock<Mutex<LogFile>> = OnceLock::new();

fn timestamp() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Initialize the logger against the given data directory (e.g. the app data
/// dir). Safe to call once; subsequent calls are ignored.
pub fn init(data_dir: &Path) {
    if LOG_TARGET.get().is_some() {
        return;
    }
    let _ = std::fs::create_dir_all(data_dir);
    let path = data_dir.join("neural-agent-os.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|_error| {
            // If we cannot open the log file, fall back to /tmp so logging
            // never breaks the app.
            let fallback = std::env::temp_dir().join("neural-agent-os.log");
            OpenOptions::new().create(true).append(true).open(&fallback)
                .expect("cannot open any log file")
        });
    let _ = LOG_TARGET.set(Mutex::new(LogFile { file, path: path.clone() }));
    let _ = log::set_logger(&AppLogger);
    // Default to Info; set NEURAL_LOG_LEVEL=debug for full verbosity.
    let level = std::env::var("NEURAL_LOG_LEVEL").ok().map(|v| match v.to_lowercase().as_str() {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    }).unwrap_or(log::LevelFilter::Info);
    log::set_max_level(level);
    log::info!("logger initialized: {} (level={level})", path.display());

    // Capture panics so crashes show up in the log file too.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("PANIC: {info}");
        default_hook(info);
    }));
}

/// Absolute path of the current log file (used by the Diagnostics view).
pub fn log_path() -> Option<PathBuf> {
    LOG_TARGET.get().map(|slot| {
        slot.lock().map(|f| f.path.clone()).unwrap_or_default()
    })
}

/// Read the last `limit` lines of the log file.
pub fn read_tail(limit: usize) -> String {
    let path = match log_path() { Some(p) => p, None => return "Logging not initialized".into() };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(error) => return format!("Could not read log file {}: {error}", path.display()),
    };
    content.lines().rev().take(limit).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")
}

/// Write a line to the log file (with rotation) and stderr.
fn write_line(line: &str) {
    if let Some(slot) = LOG_TARGET.get() {
        if let Ok(mut log_file) = slot.lock() {
            let too_big = log_file.file.metadata().map(|m| m.len() > MAX_BYTES).unwrap_or(false);
            if too_big {
                maybe_rotate(&mut log_file);
            }
            let _ = writeln!(log_file.file, "{line}");
            let _ = log_file.file.flush();
        }
    }
    eprintln!("{line}");
}

/// Rotate the log file: move the current file to `<name>.log.1` and open a
/// fresh one. Kept separate so it can be unit-tested without touching the
/// global logger.
fn maybe_rotate(log_file: &mut LogFile) {
    let rotated = log_file.path.with_extension("log.1");
    let _ = log_file.file.flush();
    let _ = std::fs::rename(&log_file.path, &rotated);
    match OpenOptions::new().create(true).append(true).open(&log_file.path) {
        Ok(fresh) => log_file.file = fresh,
        Err(_) => {
            // Reopen may fail if the directory vanished; fall back to a temp
            // file so logging keeps working.
            let fallback = std::env::temp_dir().join("neural-agent-os.log");
            if let Ok(fresh) = OpenOptions::new().create(true).append(true).open(&fallback) {
                log_file.file = fresh;
            }
        }
    }
}

struct AppLogger;

impl log::Log for AppLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        let line = format!(
            "{} {:<5} [{}] {}",
            timestamp(),
            record.level(),
            record.target(),
            record.args()
        );
        write_line(&line);
    }
    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_moves_current_file_to_log_1() {
        let dir = std::env::temp_dir().join(format!("nao-log-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.log");
        let file = OpenOptions::new().create(true).append(true).open(&path).unwrap();
        let mut log_file = LogFile { file, path: path.clone() };
        let _ = writeln!(log_file.file, "first");
        let _ = log_file.file.flush();

        maybe_rotate(&mut log_file);

        assert!(path.with_extension("log.1").exists());
        assert!(path.exists());
        let rotated_content = std::fs::read_to_string(path.with_extension("log.1")).unwrap();
        assert!(rotated_content.contains("first"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
