//! Interactive regression for the real IPC/window-creation path, separate from pure frame-policy tests.

use super::{desktop_builder, windows, MAIN_WINDOW_LABEL};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tauri::{webview::PageLoadEvent, Manager, Webview};

const PREVIEW_LABEL: &str = "compact-development";
const TEST_TIMEOUT: Duration = Duration::from_secs(15);
const OPEN_PREVIEW: &str = "window.__TAURI_INTERNALS__.invoke('open_presentation_preview')";
const MAIN_READY: &str = "!!document.querySelector('.development-toggle button')";
const PREVIEW_READY: &str = "document.querySelectorAll('.native-development .scenario-buttons button').length === 7 && document.querySelector('.native-development [role=status]')?.textContent === 'Presentation applied to the main window.'";
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const CALLBACK_CAPACITY: usize = 1;
const CALLBACK_ABANDONED: &str = "Readiness observer disconnected";
const MAIN_LOADED: &str = "Main controls rendered; invoking development preview";
const PREVIEW_LOADED: &str =
    "Development controls rendered and applied; requesting normal window closure";
const CHECK_PASSED: &str =
    "PASS: development controls rendered, applied state, and both windows closed normally";
const FAILURE_EXIT: i32 = 1;
const SUCCESS_EXIT: i32 = 0;
const PREVIEW_TIMEOUT: &str =
    "FAIL: development WebView did not load and close normally before timeout";

/// Terminates only this explicitly invoked test process if the native event loop wedges and cannot service normal close.
fn watch_timeout(finished: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        // The native freeze prevents event-thread timers/exit requests; the independent watchdog bounds this isolated process.
        std::thread::sleep(TEST_TIMEOUT);
        if !finished.load(Ordering::SeqCst) {
            eprintln!("{PREVIEW_TIMEOUT}");
            std::process::exit(FAILURE_EXIT);
        }
    });
}

/// Reads a boolean DOM readiness condition from the actual WebView; timeout or malformed results fail the isolated check.
fn ready(webview: &Webview, script: &str) -> bool {
    let (sender, receiver) = mpsc::sync_channel(CALLBACK_CAPACITY);
    webview
        .eval_with_callback(script, move |result| {
            // Only a terminated observer can reject this owned readiness result.
            if sender.send(result).is_err() {
                eprintln!("{CALLBACK_ABANDONED}");
            }
        })
        .expect("Owned WebView must accept a readiness evaluation");
    let result = receiver
        .recv_timeout(TEST_TIMEOUT)
        .expect("Event loop must service readiness before timeout");
    serde_json::from_str(&result).expect("Readiness script must return a boolean")
}

/// Waits on a worker for actual React controls, excluding network error pages and loading placeholders from success.
fn wait_ready(webview: &Webview, script: &str) {
    while !ready(webview, script) {
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Opens the actual preview through frontend IPC, waits for its WebView to finish loading, and closes both real windows normally.
pub fn preview_window_loads_and_closes_through_real_ipc() {
    windows::enable_physical_dpi();
    let finished = Arc::new(AtomicBool::new(false));
    let loaded = Arc::new(AtomicBool::new(false));
    let opened = Arc::new(AtomicBool::new(false));
    watch_timeout(finished.clone());
    let observed = loaded.clone();
    let app = desktop_builder()
        .on_page_load(move |webview, payload| {
            // Load-start notifications cannot establish readiness or invoke the frontend bridge.
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            if webview.label() == MAIN_WINDOW_LABEL && !opened.swap(true, Ordering::SeqCst) {
                let webview = webview.clone();
                std::thread::spawn(move || {
                    // Release the native load callback before dispatching the same IPC used by the development button.
                    wait_ready(&webview, MAIN_READY);
                    eprintln!("{MAIN_LOADED}");
                    webview
                        .eval(OPEN_PREVIEW)
                        .expect("Loaded owned WebView must accept the IPC test script");
                });
            } else if webview.label() == PREVIEW_LABEL {
                let webview = webview.clone();
                let observed = observed.clone();
                let app = webview.app_handle().clone();
                std::thread::spawn(move || {
                    // Closing from a worker allows native navigation callbacks to return before window dispatch.
                    wait_ready(&webview, PREVIEW_READY);
                    observed.store(true, Ordering::SeqCst);
                    eprintln!("{PREVIEW_LOADED}");
                    app.get_webview_window(PREVIEW_LABEL)
                        .expect("Loaded preview must remain registered")
                        .close()
                        .expect("Owned preview must accept normal close");
                    app.get_webview_window(MAIN_WINDOW_LABEL)
                        .expect("Main must remain registered until requested close")
                        .close()
                        .expect("Owned main must accept normal close");
                });
            }
        })
        .build(tauri::generate_context!())
        .expect("Configured desktop integration app must build");
    let exit_code = app.run_return(|_, _| {});
    finished.store(true, Ordering::SeqCst);
    assert_eq!(
        exit_code, SUCCESS_EXIT,
        "Normal closure must return a successful event-loop exit"
    );
    assert!(
        loaded.load(Ordering::SeqCst),
        "The development WebView must finish loading before normal application exit"
    );
    eprintln!("{CHECK_PASSED}");
}
