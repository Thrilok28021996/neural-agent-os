//! Drives the sidebar command self-test from the process main thread.
//!
//! Builds a real Wry app against an isolated temp data dir and exercises every
//! Tauri command backing the left-sidebar views. Run:
//!
//!   cargo run --example sidebar_selftest

fn main() {
    match neural_agent_os_lib::run_sidebar_selftest() {
        Ok(report) => {
            println!();
            println!("==== {report} ====");
            println!("SIDEBAR SELFTEST: ALL CHECKS PASSED");
        }
        Err(report) => {
            eprintln!();
            eprintln!("==== {report} ====");
            eprintln!("SIDEBAR SELFTEST: FAILURES DETECTED");
            std::process::exit(1);
        }
    }
}
