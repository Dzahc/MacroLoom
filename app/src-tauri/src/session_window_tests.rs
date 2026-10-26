//! Isolated real-WebView checks for OS registration conflicts, recovery, and app-focused hook dispatch.

use super::*;
use crate::presentation::PREVIEW_WINDOW;
use crate::session::{Phase, Snapshot};
use crate::shortcuts::{PLAY_UNAVAILABLE, RECORD_UNAVAILABLE, STOP_UNAVAILABLE};
use std::path::PathBuf;
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
const PLAY_BUTTON: &str = "document.querySelector('[data-action=play]')";
const SELECTED_READY: &str = "!!document.querySelector('[data-macro-id][aria-pressed=true]')";
const INITIAL_WAIT: Duration = Duration::from_secs(3);
const SELECT_TO_PLAY: &str = "Select a macro to play";
const RETRY_BUTTON: &str = "Array.from(document.querySelectorAll('button')).find(button => button.textContent === 'Retry shortcuts')";
const OPEN_PREVIEW: &str = "window.__TAURI_INTERNALS__.invoke('open_presentation_preview')";
const PREVIEW_READY: &str = "document.querySelector('.native-development [role=status]')?.textContent === 'Presentation applied to the main window.'";
const FOCUS_READY: &str = "document.hasFocus()";
const CHECK_PASSED: &str = "PASS: F9/F10/F8 conflicts, GUI fallback, recording exclusion, toast, Play/Stop, real owned/external F9/F10 delivery, focus preservation, retry and shutdown";
const TEST_LIBRARY_PREFIX: &str = "macroloom-native-session-check";

/// Owns only a unique temporary native-check library, never the executable's ordinary macros folder.
struct CheckLibrary(PathBuf);
impl CheckLibrary {
    /// Chooses an isolated path; the ordinary library worker performs discovery and directory creation.
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("{TEST_LIBRARY_PREFIX}-{}", uuid::Uuid::new_v4())))
    }
    /// Creates the trusted service using this fixture's directory, without overriding global storage preferences.
    fn service(&self) -> crate::library_commands::LibraryService {
        crate::library_commands::LibraryService(Ok(Arc::new(crate::repository::Repository::new(
            self.0.clone(),
        ))))
    }
}
impl Drop for CheckLibrary {
    /// Removes only the verified uniquely named temporary fixture root after its owned windows have closed.
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(self
            .0
            .file_name()
            .expect("Check library name")
            .to_string_lossy()
            .starts_with(TEST_LIBRARY_PREFIX));
        if self.0.is_dir() {
            std::fs::remove_dir_all(&self.0).expect("Remove native-check library");
        }
    }
}

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
    handle_keyboard(&input, message)
}
/// Exercises reserved controls with the selected real app window focused, while ordinary own-window keys remain excluded.
fn focused_controls(window: &WebviewWindow, session: &Session) -> Result<(), String> {
    window.set_focus().map_err(|error| error.to_string())?;
    wait_ui(window.as_ref(), FOCUS_READY)?;
    require(
        own_window_focused(),
        "The test app must own foreground focus",
    )?;
    check_focused_key_delivery()?;
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
    control_press(PLAY_KEY)?;
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
/// Verifies Stop conflict rejects UI startup, preserves a committed recording, and recovers after the competing key is released.
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
/// Commits one nonempty capture through the real backend into the isolated library, without sending text to another app.
fn retained_capture(session: &Session) -> Result<Snapshot, String> {
    session.record()?;
    std::thread::sleep(INITIAL_WAIT);
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
/// Checks that rejected startup preserves the previous save receipt, its persistence state and Idle ownership.
fn capture_unchanged(session: &Session, retained: &Snapshot) -> Result<(), String> {
    let blocked = session.snapshot()?;
    let retained_id = retained
        .recording_outcome
        .as_ref()
        .map(|outcome| &outcome.id);
    let blocked_id = blocked
        .recording_outcome
        .as_ref()
        .map(|outcome| &outcome.id);
    require(
        blocked.phase == Phase::Idle
            && blocked.event_count == retained.event_count
            && blocked.unsaved == retained.unsaved
            && retained_id.is_some()
            && retained_id == blocked_id,
        "Stop conflict must reject startup and preserve the committed recording",
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
    no_selection_play(&main)?;
    let development = focused_development(&webview, &app, &session)?;
    stop_conflict(&webview, &session)?;
    check_playback(&main, &session)?;
    checked.store(true, Ordering::Release);
    development.close().map_err(|error| error.to_string())?;
    main.close().map_err(|error| error.to_string())?;
    Ok(())
}
/// Checks selected playback through both the conflicted GUI fallback and the recovered native F10 route.
fn check_playback(window: &WebviewWindow, session: &Session) -> Result<(), String> {
    play_conflict(window, session)?;
    global_play(window, session)?;
    check_external_delivery(|| release_playback(session))
}
/// Drives no-selection F10 through the native handler and real toast rendering without changing focus.
fn no_selection_play(window: &WebviewWindow) -> Result<(), String> {
    control_press(PLAY_KEY)?;
    let message = serde_json::to_string(SELECT_TO_PLAY).map_err(|error| error.to_string())?;
    wait_ui(
        window.as_ref(),
        &format!("document.body.textContent.includes({message})"),
    )
}
/// Proves actual F10 registration failure, usable GUI Play/Stop during an initial wait, and retry after release.
fn play_conflict(window: &WebviewWindow, session: &Session) -> Result<(), String> {
    let conflict = session.preview(|| {
        configure_shortcuts(false, false)?;
        Conflict::hold(PLAY_KEY)
    })?;
    session.refresh()?;
    conflict_visible(window.as_ref(), PLAY_UNAVAILABLE)?;
    gui_playback(window, session)?;
    drop(conflict);
    click(window.as_ref(), RETRY_BUTTON)?;
    wait_ui(window.as_ref(), &format!("!({RETRY_BUTTON})"))?;
    require(session.snapshot()?.shortcuts.play, "Retry must acquire F10")
}
/// Waits for authoritative selected/available GUI state, then cancels real playback before any fixture input is due.
fn gui_playback(window: &WebviewWindow, session: &Session) -> Result<(), String> {
    wait_ui(window.as_ref(), SELECTED_READY)?;
    wait_ui(
        window.as_ref(),
        &format!("({PLAY_BUTTON})?.disabled === false"),
    )?;
    click(window.as_ref(), PLAY_BUTTON)?;
    phase(session, Phase::Playing)?;
    stop_starting_playback();
    phase(session, Phase::Idle)?;
    Ok(())
}
/// Exercises release-origin Play and F8 cancellation through real hooks/workers, before any fixture macro input is due.
fn global_play(window: &WebviewWindow, session: &Session) -> Result<(), String> {
    window.set_focus().map_err(|error| error.to_string())?;
    wait_ui(window.as_ref(), FOCUS_READY)?;
    wait_ui(window.as_ref(), SELECTED_READY)?;
    release_playback(session)?;
    require(
        own_window_focused(),
        "Play/Stop transitions preserve test focus",
    )
}
/// Starts the selected fixture through F10 and stops before its initial input deadline, without activating any window.
fn release_playback(session: &Session) -> Result<(), String> {
    control_press(PLAY_KEY)?;
    phase(session, Phase::Playing)?;
    stop_starting_playback();
    phase(session, Phase::Idle)?;
    Ok(())
}
/// Exercises the earliest physical F8 arrival, which may precede startup reservation and must still cancel backend intent.
fn stop_starting_playback() {
    physical_key(STOP_KEY, true);
    physical_key(STOP_KEY, false);
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
    let library = CheckLibrary::new();
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
    let app = crate::desktop_builder_with_library(true, library.service())
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
    assert!(
        !hooks_running(),
        "Main close must acknowledge hook shutdown before exit"
    );
    for key in [RECORD_KEY, PLAY_KEY, STOP_KEY] {
        drop(Conflict::hold(key).expect("Closed application released its shortcut"));
    }
    eprintln!("{CHECK_PASSED}");
}
