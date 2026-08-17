//! End-to-end smoke test for the application logger: init a fresh log file in
//! a temp dir, write through the `log` macros, and confirm read_tail sees it.
use neural_agent_os_lib::logging;

#[test]
fn logger_writes_lines_visible_to_read_tail() {
    let dir = std::env::temp_dir().join(format!("nao-log-e2e-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    logging::init(&dir);
    let marker = format!("e2e-marker-{}", uuid::Uuid::new_v4());
    log::info!("{marker} hello from integration test");
    log::warn!("{marker} warn line");
    let tail = logging::read_tail(100);
    assert!(tail.contains(&marker), "log tail should contain marker; got:\n{tail}");
    // Path points inside the temp dir and the file physically exists.
    let path = logging::log_path().expect("log path");
    assert!(path.starts_with(&dir), "log path should be under temp dir: {path:?}");
    assert!(path.exists());
    std::fs::remove_dir_all(&dir).ok();
}
