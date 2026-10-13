//! Tauri/native adapter for the shared session command boundary.
use crate::app_mode::AppMode;
use crate::configure::ConfigureService;
use crate::input::{EventKind, Held};
use crate::library_commands::LibraryService;
use crate::presentation::{PresentationPhase, PresentationService, PreviewStatus};
use crate::session::{
    Phase, Platform, Session, Snapshot, StartRequest, ACTIVE_SESSION, SESSION_EVENT,
    STATUS_INTERVAL,
};
use crate::session_publication::Publications;
use crate::shortcuts::Availability;
use crate::window_view::ViewRequest;
use crate::windows;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

const MAIN: &str = "main";
const CONTENT_WIDTH: f64 = crate::window_view::MIN_CONTENT_WIDTH;
const CONTENT_HEIGHT: f64 = 156.0;
const NATIVE_UNAVAILABLE: &str = "Native session window is unavailable";
const LIBRARY_UNAVAILABLE: &str = "Wait for library loading or the current operation to finish";
const WRONG_CALLER: &str = "Session command is unavailable in this window";
const DEVELOPMENT_ONLY: &str = "Temporary replay is available only in development controls";
const TEMPORARY_NAME: &str = "Temporary recording";
const DELIVERY_FAILED: &str = "Session event delivery failed";
const SUCCESS_EXIT: i32 = 0;

/// Owned application handle bridges lifecycle effects; it never retains a raw borrowed window handle.
pub struct NativePlatform {
    app: AppHandle,
    live_active: AtomicBool,
    publications: Publications,
}
impl NativePlatform {
    /// Binds the running Tauri application without starting any native input.
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            live_active: AtomicBool::new(false),
            publications: Publications::default(),
        }
    }
    /// Rejects starts during library discovery/deletion or when executable-relative storage is unavailable.
    fn require_library(&self) -> Result<(), String> {
        if AppMode::current().input_prototype {
            return Ok(());
        }
        let library = self
            .app
            .state::<LibraryService>()
            .repository()?
            .snapshot()?;
        if library.loading || !library.writable || library.deleting.is_some() {
            return Err(LIBRARY_UNAVAILABLE.into());
        }
        Ok(())
    }
    /// Emits an already ordered live snapshot; old Idle snapshots cannot overwrite a newer active presentation.
    fn publish_current(&self, snapshot: Snapshot) {
        let status = presentation_status(&snapshot);
        let active = snapshot.phase != Phase::Idle;
        let was_active = self.live_active.swap(active, Ordering::AcqRel);
        if active || was_active {
            if let Err(error) = self
                .app
                .state::<PresentationService>()
                .publish_live(&self.app, status)
            {
                eprintln!("{DELIVERY_FAILED}: {error}");
            }
        }
        if let Err(error) = self.app.emit(SESSION_EVENT, snapshot) {
            eprintln!("{DELIVERY_FAILED}: {error}");
        }
    }
}
impl Platform for NativePlatform {
    /// Claims the existing backend operation interlock throughout a session and its cleanup.
    fn reserve(&self) -> Result<Box<dyn Send>, String> {
        self.require_library()?;
        Ok(Box::new(
            self.app.state::<ConfigureService>().reserve_operation()?,
        ))
    }
    /// Checks idle application availability without relying on frontend toolbar state.
    fn available(&self) -> bool {
        self.require_library().is_ok()
            && self
                .app
                .state::<ConfigureService>()
                .require_operation()
                .is_ok()
    }
    /// Performs reservation updates on the native hook message thread.
    fn shortcuts(&self, record: bool, stop: bool) -> Result<Availability, String> {
        windows::configure_shortcuts(record, stop)
    }
    /// Serializes native placement on the main event thread, waiting only on the calling worker.
    fn view(&self, compact: bool) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let app = self.app.clone();
        self.app
            .run_on_main_thread(move || {
                let result = apply_view(&app, compact);
                let _ = sender.send(result);
            })
            .map_err(|error| error.to_string())?;
        receiver.recv().map_err(|error| error.to_string())?
    }
    /// Obtains the current physical pointer position at the capture boundary.
    fn cursor(&self) -> (i32, i32) {
        windows::cursor_position()
    }
    /// Rejects targets inside the actual main-window frame before injecting mouse actions.
    fn inject(&self, kind: &EventKind) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window(MAIN)
            .ok_or(NATIVE_UNAVAILABLE)?;
        let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as isize;
        windows::send_session_event(hwnd, kind)
    }
    /// Propagates native release failure for the session owner to retain and retry.
    fn release(&self, held: Held) -> Result<(), String> {
        windows::release(held)
    }
    /// Publishes live compact status and revisioned session state without activating any window.
    fn publish(&self, snapshot: Snapshot) {
        if let Err(error) = self
            .publications
            .send(snapshot, |snapshot| self.publish_current(snapshot))
        {
            eprintln!("{DELIVERY_FAILED}: {error}");
        }
    }
}
/// Applies actual placement independently of frontend measurements, retaining full placement on failure.
fn apply_view(app: &AppHandle, compact: bool) -> Result<(), String> {
    let service = app.state::<PresentationService>();
    let mut view = service.view.lock().map_err(|_| NATIVE_UNAVAILABLE)?;
    let result = view.update(ViewRequest {
        compact,
        width: CONTENT_WIDTH,
        height: CONTENT_HEIGHT,
    });
    app.emit(crate::presentation::WINDOW_VIEW_EVENT, &result)
        .map_err(|error| error.to_string())?;
    match result.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
/// Converts the authoritative lifecycle to the existing compact presentation contract.
fn presentation_status(snapshot: &Snapshot) -> PreviewStatus {
    let phase = match snapshot.phase {
        Phase::Idle => PresentationPhase::Idle,
        Phase::Recording => PresentationPhase::Recording,
        Phase::Playing => PresentationPhase::Playing,
        Phase::Stopping => PresentationPhase::Stopping,
    };
    PreviewStatus {
        phase,
        macro_name: (snapshot.phase == Phase::Playing).then(|| TEMPORARY_NAME.into()),
        elapsed_ms: snapshot.elapsed_ms,
        ..PreviewStatus::default()
    }
}
/// Publishes bounded progress and reconciles idle reservations away from UI/input event loops.
pub fn start_status(session: Arc<Session>) {
    std::thread::spawn(move || {
        while windows::hooks_running() {
            if let Err(error) = session.refresh() {
                session.report(error);
            }
            std::thread::sleep(STATUS_INTERVAL);
        }
    });
}
/// Enforces session command callers, including the explicit development control window.
fn require_caller(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == MAIN || window.label() == crate::presentation::PREVIEW_WINDOW {
        Ok(())
    } else {
        Err(WRONG_CALLER.into())
    }
}
#[tauri::command]
/// Returns current backend state without starting input or requiring a mounted listener.
pub fn session_snapshot(session: State<'_, Arc<Session>>) -> Result<Snapshot, String> {
    session.snapshot()
}
#[tauri::command]
/// Runs the shared Record workflow on a worker; backend ownership and required Stop are checked atomically.
pub async fn start_recording(
    window: WebviewWindow,
    session: State<'_, Arc<Session>>,
) -> Result<Snapshot, String> {
    require_caller(&window)?;
    let request = session.start_request()?.ok_or(ACTIVE_SESSION)?;
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || command_record(&session, request))
        .await
        .map_err(|error| error.to_string())?
}
/// Reports failed button starts through the same persistent status used for global-control failures.
fn command_record(session: &Session, request: StartRequest) -> Result<Snapshot, String> {
    match session.record_requested(request) {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            session.report(error.clone());
            Err(error)
        }
    }
}
#[tauri::command]
/// Signals Stop immediately, then performs cleanup on a worker independently of frontend events.
pub async fn stop(
    window: WebviewWindow,
    session: State<'_, Arc<Session>>,
) -> Result<Snapshot, String> {
    require_caller(&window)?;
    let request = session.stop_request(false)?;
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || session.complete_stop(request))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
/// Retries idle native reservations without starting a session.
pub async fn retry_shortcuts(
    window: WebviewWindow,
    session: State<'_, Arc<Session>>,
) -> Result<Snapshot, String> {
    require_caller(&window)?;
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || session.retry_shortcuts())
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
/// Replays only the retained temporary recording from explicit debug development controls; saved-library Play is deferred.
pub async fn play(
    window: WebviewWindow,
    session: State<'_, Arc<Session>>,
) -> Result<Snapshot, String> {
    require_caller(&window)?;
    if !cfg!(debug_assertions) {
        return Err(DEVELOPMENT_ONLY.into());
    }
    let request = session.start_request()?.ok_or(ACTIVE_SESSION)?;
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || session.play_requested(request))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
/// Reserves/releases a frontend confirmation through the backend interlock before displaying it.
pub async fn session_confirmation(
    blocked: bool,
    owner: String,
    window: WebviewWindow,
    session: State<'_, Arc<Session>>,
) -> Result<Snapshot, String> {
    require_caller(&window)?;
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || session.confirmation(&owner, blocked))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
/// Cancels the explicit close decision while keeping the temporary recording available.
pub fn session_keep_open(
    window: WebviewWindow,
    session: State<'_, Arc<Session>>,
) -> Result<Snapshot, String> {
    require_caller(&window)?;
    session.keep_open()
}
#[tauri::command]
/// Discards temporary input only on the user's explicit action, then closes after cleanup.
pub fn session_discard_exit(
    window: WebviewWindow,
    session: State<'_, Arc<Session>>,
) -> Result<(), String> {
    require_caller(&window)?;
    window
        .app_handle()
        .state::<ConfigureService>()
        .require_operation()?;
    session.discard_exit()?;
    windows::shutdown_hooks();
    window.app_handle().exit(SUCCESS_EXIT);
    Ok(())
}
/// Prevents native close until worker cleanup and an explicit unsaved-recording decision complete.
pub fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() != MAIN {
        return;
    }
    let tauri::WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let Some(session) = window.try_state::<Arc<Session>>() else {
        return;
    };
    match session.request_close() {
        Ok(false) => {}
        Ok(true) => {
            api.prevent_close();
            windows::queue_stop();
        }
        Err(error) => {
            api.prevent_close();
            session.report(error);
        }
    }
}
