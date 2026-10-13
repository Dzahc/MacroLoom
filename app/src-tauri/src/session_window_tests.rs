//! Isolated real-WebView checks for OS registration conflicts, recovery, and app-focused hook dispatch.

use super::*;
use crate::presentation::PREVIEW_WINDOW;
use crate::session::{Phase, Snapshot};
use crate::shortcuts::{RECORD_UNAVAILABLE, STOP_UNAVAILABLE};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;
use tauri::{webview::PageLoadEvent, AppHandle, Manager, Webview, WebviewWindow};

const TIMEOUT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(20);
const FAILURE_EXIT: i32 = 1;
const SUCCESS_EXIT: i32 = 0;
const TEST_KEY: u16 = 0x41;
const TEST_SCAN: u16 = 0x1e;
const RETRY: &str = "Retry shortcuts";
const RECORD_BUTTON: &str = "document.querySelector('[data-action=record]')";
const STOP_BUTTON: &str = "document.querySelector('[data-action=stop]')";
const RETRY_BUTTON: &str = "Array.from(document.querySelectorAll('button')).find(button => button.textContent === 'Retry shortcuts')";
const OPEN_PREVIEW: &str = "window.__TAURI_INTERNALS__.invoke('open_presentation_preview')";
const PREVIEW_READY: &str = "document.querySelector('.native-development [role=status]')?.textContent === 'Presentation applied to the main window.'";
const CHECK_PASSED: &str = "PASS: visible F9/F8 conflicts, button fallback, retained capture, retry recovery, and main/development-focused hook controls";

/// Owns a real competing thread-local reservation; dropping it releases the key and joins the helper.
struct Conflict {
    release: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}
impl Conflict {
    /// Reserves the requested actual control key on another thread, failing clearly if a user application already owns it.
    fn hold(key: u16) -> Result<Self, String> {
        let (ready, receive_ready) = mpsc::sync_channel(1);
        let (release, receive_release) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let registered = update_registration(false, RECORD_ID, key, true);
            let _reservation = match &registered {
                Ok(true) => Some(TestReservation(RECORD_ID)),
                _ => None,
            };
            let _ = ready.send(registered);
            let _ = receive_release.recv();
        });
        let conflict = Self {
            release,
            thread: Some(thread),
        };
        if !receive_ready
            .recv_timeout(TIMEOUT)
            .map_err(|error| error.to_string())??
        {
            return Err("F8/F9 must be free before this isolated check; close MacroLoom and other key owners".into());
        }
        Ok(conflict)
    }
}
impl Drop for Conflict {
    /// Releases only this fixture's reservation and waits for its owning thread to complete unregistration.
    fn drop(&mut self) {
        let _ = self.release.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Checks an invariant and returns a descriptive test failure without panicking on the native event thread.
fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
/// Evaluates a synchronous boolean in the actual WebView, preserving permission errors in the rendered UI for assertions.
fn evaluate(webview: &Webview, script: &str) -> Result<bool, String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    webview
        .eval_with_callback(format!("Boolean({script})"), move |value| {
            let _ = sender.send(value);
        })
        .map_err(|error| error.to_string())?;
    let value = receiver
        .recv_timeout(TIMEOUT)
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&value).map_err(|error| error.to_string())
}
/// Waits for one read-only UI condition; bounded failure reports the condition that did not become visible.
fn wait_ui(webview: &Webview, condition: &str) -> Result<(), String> {
    let deadline = Instant::now() + TIMEOUT;
    while !evaluate(webview, condition)? {
        require(Instant::now() < deadline, condition)?;
        std::thread::sleep(POLL);
    }
    Ok(())
}
/// Clicks an available real React control exactly once; unavailable controls fail rather than bypassing UI preconditions.
fn click(webview: &Webview, selector: &str) -> Result<(), String> {
    let script = format!("(() => {{ const button = {selector}; if (!button || button.disabled) return false; button.click(); return true; }})()");
    require(
        evaluate(webview, &script)?,
        &format!("Expected available control: {selector}"),
    )
}
/// Reads the rendered banner and retry control together, proving conflict feedback is visible in the actual application.
fn conflict_visible(webview: &Webview, message: &str) -> Result<(), String> {
    let message = serde_json::to_string(message).map_err(|error| error.to_string())?;
    let retry = serde_json::to_string(RETRY).map_err(|error| error.to_string())?;
    wait_ui(webview, &format!("document.querySelector('.library-banner')?.textContent.includes({message}) && Array.from(document.querySelectorAll('button')).some(button => button.textContent === {retry})"))
}
/// Waits for the public native session phase; workers remain responsible for transitions and cleanup.
fn phase(session: &Session, expected: Phase) -> Result<Snapshot, String> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let snapshot = session.snapshot()?;
        if snapshot.phase == expected {
            return Ok(snapshot);
        }
        require(
            Instant::now() < deadline,
            "Session phase did not change before timeout",
        )?;
        std::thread::sleep(POLL);
    }
}
/// Sends a non-injected hook callback fixture through the actual handler; never injects input into the Windows desktop.
fn physical_key(key: u16, down: bool) -> bool {
    let input = KBDLLHOOKSTRUCT {
        vkCode: u32::from(key),
        scanCode: 0,
        flags: 0,
        time: 0,
        dwExtraInfo: 0,
    };
    let message = if down {
        WM_KEYDOWN as usize
    } else {
        WM_KEYUP as usize
    };
    let consumed = handle_keyboard(&input, message);
    trace_key(&input, message, consumed);
    consumed
}
/// Exercises reserved controls with the selected real app window focused, while ordinary own-window keys remain excluded.
fn focused_controls(window: &WebviewWindow, session: &Session) -> Result<(), String> {
    window.set_focus().map_err(|error| error.to_string())?;
    require(
        own_window_focused(),
        "The test app must own foreground focus",
    )?;
    control_press(RECORD_KEY)?;
    phase(session, Phase::Recording)?;
    exclude_own_keys(session)?;
    control_press(STOP_KEY)?;
    phase(session, Phase::Idle)?;
    Ok(())
}
/// Exercises a complete fresh reserved control sequence, asserting suppression of both its down and release.
fn control_press(key: u16) -> Result<(), String> {
    require(
        physical_key(key, true) && physical_key(key, false),
        "Focused control down/release must be consumed",
    )
}
/// Verifies that ordinary typing into the focused test app remains outside its active recording.
fn exclude_own_keys(session: &Session) -> Result<(), String> {
    require(
        !physical_key(TEST_KEY, true),
        "Own-window ordinary input must pass through",
    )?;
    physical_key(TEST_KEY, false);
    require(
        session.snapshot()?.event_count == 0,
        "Own-window typing must not be recorded",
    )?;
    Ok(())
}
/// Finds the asynchronously created development window without blocking the native page-load callback.
fn preview(app: &AppHandle) -> Result<WebviewWindow, String> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(window) = app.get_webview_window(PREVIEW_WINDOW) {
            return Ok(window);
        }
        require(Instant::now() < deadline, "Development window did not open")?;
        std::thread::sleep(POLL);
    }
}
/// Verifies Record conflict button fallback and UI retry after actual competing ownership ends.
fn record_conflict(webview: &Webview, session: &Session, conflict: Conflict) -> Result<(), String> {
    conflict_visible(webview, RECORD_UNAVAILABLE)?;
    click(webview, RECORD_BUTTON)?;
    phase(session, Phase::Recording)?;
    click(webview, STOP_BUTTON)?;
    phase(session, Phase::Idle)?;
    drop(conflict);
    click(webview, RETRY_BUTTON)?;
    wait_ui(
        webview,
        &format!("({RECORD_BUTTON})?.disabled === false && !({RETRY_BUTTON})"),
    )?;
    require(
        session.snapshot()?.shortcuts.record,
        "Retry must acquire F9",
    )
}
/// Verifies Stop conflict rejects UI startup, preserves a nonempty prior capture, and recovers after the competing key is released.
fn stop_conflict(webview: &Webview, session: &Session) -> Result<(), String> {
    let retained = retained_capture(session)?;
    let conflict = Conflict::hold(STOP_KEY)?;
    click(webview, RECORD_BUTTON)?;
    conflict_visible(webview, STOP_UNAVAILABLE)?;
    capture_unchanged(session, &retained)?;
    drop(conflict);
    click(webview, RETRY_BUTTON)?;
    wait_ui(
        webview,
        &format!("({RECORD_BUTTON})?.disabled === false && !({RETRY_BUTTON})"),
    )?;
    button_session(webview, session)
}
/// Creates one nonempty in-memory capture through the real backend, without sending text to any application.
fn retained_capture(session: &Session) -> Result<Snapshot, String> {
    session.record()?;
    session.capture(EventKind::Key {
        vk: TEST_KEY,
        scan: TEST_SCAN,
        extended: false,
        down: true,
    });
    session.capture(EventKind::Key {
        vk: TEST_KEY,
        scan: TEST_SCAN,
        extended: false,
        down: false,
    });
    session.stop()
}
/// Checks that rejected startup preserves both retained capture and Idle ownership.
fn capture_unchanged(session: &Session, retained: &Snapshot) -> Result<(), String> {
    let blocked = session.snapshot()?;
    require(
        blocked.phase == Phase::Idle
            && blocked.event_count == retained.event_count
            && blocked.unsaved,
        "Stop conflict must reject startup and retain the old capture",
    )
}
/// Starts and stops an empty recording through real React toolbar buttons, ending with no unsaved test data.
fn button_session(webview: &Webview, session: &Session) -> Result<(), String> {
    click(webview, RECORD_BUTTON)?;
    phase(session, Phase::Recording)?;
    click(webview, STOP_BUTTON)?;
    phase(session, Phase::Idle)?;
    Ok(())
}
/// Runs all real registration/UI checks and both focused-window hook fixtures before normally closing only the test windows.
fn check(webview: Webview, conflict: Conflict, checked: Arc<AtomicBool>) -> Result<(), String> {
    let app = webview.app_handle().clone();
    let session = app.state::<Arc<Session>>().inner().clone();
    let main = app
        .get_webview_window(crate::MAIN_WINDOW_LABEL)
        .ok_or("Main test window missing")?;
    record_conflict(&webview, &session, conflict)?;
    focused_controls(&main, &session)?;
    let development = focused_development(&webview, &app, &session)?;
    stop_conflict(&webview, &session)?;
    checked.store(true, Ordering::Release);
    development.close().map_err(|error| error.to_string())?;
    main.close().map_err(|error| error.to_string())?;
    Ok(())
}
/// Opens the real development pane and verifies its focused shortcut path after presentation setup settles.
fn focused_development(
    webview: &Webview,
    app: &AppHandle,
    session: &Session,
) -> Result<WebviewWindow, String> {
    webview
        .eval(OPEN_PREVIEW)
        .map_err(|error| error.to_string())?;
    let development = preview(app)?;
    wait_ui(development.as_ref(), PREVIEW_READY)?;
    focused_controls(&development, session)?;
    Ok(development)
}
/// Starts an isolated app with real hooks and a bounded watchdog; failures never close or modify another application's windows.
pub(super) fn run() {
    enable_physical_dpi();
    let conflict = Conflict::hold(RECORD_KEY).expect("F9 must be free for the competing fixture");
    let conflict = Arc::new(Mutex::new(Some(conflict)));
    let finished = Arc::new(AtomicBool::new(false));
    let checked = Arc::new(AtomicBool::new(false));
    let observed = checked.clone();
    let watchdog = finished.clone();
    std::thread::spawn(move || {
        std::thread::sleep(TIMEOUT);
        if !watchdog.load(Ordering::Acquire) {
            eprintln!("FAIL: session control check timed out");
            std::process::exit(FAILURE_EXIT);
        }
    });
    let app = crate::desktop_builder(true)
        .on_page_load(move |webview, payload| {
            if payload.event() != PageLoadEvent::Finished
                || webview.label() != crate::MAIN_WINDOW_LABEL
            {
                return;
            }
            let Some(conflict) = conflict
                .lock()
                .expect("Fixture ownership must remain healthy")
                .take()
            else {
                return;
            };
            let webview = webview.clone();
            let checked = observed.clone();
            std::thread::spawn(move || {
                if let Err(error) = check(webview, conflict, checked) {
                    eprintln!("FAIL: {error}");
                    std::process::exit(FAILURE_EXIT);
                }
            });
        })
        .build(tauri::generate_context!())
        .expect("Configured check app must build");
    let exit = app.run_return(|_, _| {});
    finished.store(true, Ordering::Release);
    assert_eq!(exit, SUCCESS_EXIT, "Test windows must close normally");
    assert!(
        checked.load(Ordering::Acquire),
        "All session assertions must complete before test windows close"
    );
    eprintln!("{CHECK_PASSED}");
}
