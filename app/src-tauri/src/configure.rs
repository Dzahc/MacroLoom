//! Native singleton Configure window and asynchronous, nonpersisting callback bridge.

use crate::app_mode::AppMode;
use crate::configure_model::{
    ConfigureDraft, ConfigureResult, ConfigureState, CONFIGURE_BUSY, CONFIGURE_MISSING,
    CONFIGURE_PENDING,
};
use crate::library_commands::LibraryService;
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const CONFIGURE_WINDOW: &str = "configure";
pub const CONFIGURE_SUBMIT_EVENT: &str = "configure-submit";
pub const CONFIGURE_CLOSED_EVENT: &str = "configure-closed";
const MAIN_WINDOW: &str = "main";
const EDITOR_URL: &str = "index.html?configure";
const EDITOR_TITLE: &str = "MacroLoom — Configure macro";
const WIDTH: f64 = 480.0;
const HEIGHT: f64 = 620.0;
const MIN_WIDTH: f64 = 360.0;
const MIN_HEIGHT: f64 = 300.0;
const LOCK_FAILED: &str = "Configure state is unavailable; restart MacroLoom";
const INVALID_CALLER: &str = "Configure command is unavailable in this window";
const DELIVERY_FAILED: &str = "Configure callback delivery failed";
const CONSUMED_ATTEMPT: u64 = 0;

#[derive(Default)]
/// Managed singleton editor interlock shared by native commands, callback workers, and window lifecycle events.
pub struct ConfigureService {
    pub inner: Arc<Mutex<ConfigureState>>,
}

impl ConfigureService {
    /// Returns a short-lived state guard, surfacing poison rather than allowing conflicting operations.
    pub fn lock(&self) -> Result<MutexGuard<'_, ConfigureState>, String> {
        self.inner.lock().map_err(|_| LOCK_FAILED.into())
    }
    /// Rejects native operations while Configure owns the modal reservation.
    pub fn require_operation(&self) -> Result<(), String> {
        self.lock()?.require_operation()
    }
    /// Atomically reserves a library mutation; drop releases it even on failed disk work.
    pub fn reserve_operation(&self) -> Result<OperationReservation, String> {
        let mut state = self.lock()?;
        state.require_operation()?;
        state.operation = true;
        Ok(OperationReservation(self.inner.clone()))
    }
}

/// Worker-owned mutation reservation; never holds a mutex while performing disk or native work.
pub struct OperationReservation(Arc<Mutex<ConfigureState>>);
impl Drop for OperationReservation {
    /// Releases the exclusion flag on every success/error/panic path; poisoning remains a visible failure.
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lock() {
            state.operation = false;
        }
    }
}

/// Enforces the exact native caller; capabilities alone do not authorize backend command arguments.
fn require_caller(window: &WebviewWindow, label: &str) -> Result<(), String> {
    if window.label() == label {
        Ok(())
    } else {
        Err(INVALID_CALLER.into())
    }
}

#[tauri::command]
/// Reserves one editor, reads its validated snapshot on a worker, and creates a real owned modal window.
/// Rejects prototype/busy/deleting states; read/build failures release the reservation and preserve library data.
pub async fn open_configure(
    macro_id: String,
    window: WebviewWindow,
    app: AppHandle,
    service: State<'_, ConfigureService>,
    library: State<'_, LibraryService>,
) -> Result<(), String> {
    require_caller(&window, MAIN_WINDOW)?;
    AppMode::current().require_library()?;
    let repository = library.repository()?;
    service.lock()?.reserve()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        if repository.snapshot()?.deleting.is_some() {
            return Err(CONFIGURE_BUSY.into());
        }
        repository
            .read(&macro_id)
            .map(ConfigureDraft::from_document)
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(|result| result);
    install_snapshot(&app, &service, result)?;
    finish_open(&app, &window, &service)
}

/// Completes native creation or rolls back its reservation, honoring exit requested while its snapshot was loading.
fn finish_open(
    app: &AppHandle,
    window: &WebviewWindow,
    service: &ConfigureService,
) -> Result<(), String> {
    if let Err(error) = create_window(app) {
        reset(app);
        return Err(error);
    }
    if service.lock()?.exit_requested {
        window.close().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Installs a worker-read snapshot, releasing native modal ownership if its file could not be read.
fn install_snapshot(
    app: &AppHandle,
    service: &ConfigureService,
    result: Result<ConfigureDraft, String>,
) -> Result<(), String> {
    match result {
        Ok(original) => {
            service.lock()?.original = Some(original);
            Ok(())
        }
        Err(error) => {
            reset(app);
            Err(error)
        }
    }
}

/// Builds the owned resizable editor and disables its parent using Tauri's native window adapter.
/// Creation is invoked from asynchronous IPC so WebView2 initialization cannot deadlock the event thread.
fn create_window(app: &AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or(CONFIGURE_MISSING)?;
    let editor =
        WebviewWindowBuilder::new(app, CONFIGURE_WINDOW, WebviewUrl::App(EDITOR_URL.into()))
            .owner(&main)
            .map_err(|error| error.to_string())?
            .title(EDITOR_TITLE)
            .inner_size(WIDTH, HEIGHT)
            .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
            .resizable(true)
            .minimizable(false)
            .build()
            .map_err(|error| error.to_string())?;
    if let Err(error) = main.set_enabled(false) {
        let _ = editor.destroy();
        return Err(error.to_string());
    }
    Ok(())
}

#[tauri::command]
/// Returns only the supplied editor's frozen properties, without event arrays or disk work.
pub fn configure_snapshot(
    window: WebviewWindow,
    service: State<'_, ConfigureService>,
) -> Result<ConfigureDraft, String> {
    require_caller(&window, CONFIGURE_WINDOW)?;
    service
        .lock()?
        .original
        .clone()
        .ok_or_else(|| CONFIGURE_MISSING.into())
}

#[tauri::command]
/// Validates a complete draft, emits it once to the main consumer, and waits on a worker for acknowledgement.
/// No storage is performed; failures leave original data and frontend drafts intact for deliberate retry.
pub async fn configure_submit(
    draft: ConfigureDraft,
    window: WebviewWindow,
    app: AppHandle,
    service: State<'_, ConfigureService>,
) -> Result<ConfigureResult, String> {
    require_caller(&window, CONFIGURE_WINDOW)?;
    let (sender, receiver) = mpsc::channel();
    let submission = service.lock()?.begin(draft, sender)?;
    if app
        .emit_to(MAIN_WINDOW, CONFIGURE_SUBMIT_EVENT, submission)
        .is_err()
    {
        let result = ConfigureResult::failure();
        service.lock()?.settle(&result);
        return Ok(result);
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        receiver
            .recv()
            .unwrap_or_else(|_| ConfigureResult::failure())
    })
    .await
    .map_err(|error| error.to_string());
    let result = result.unwrap_or_else(|_| ConfigureResult::failure());
    service.lock()?.settle(&result);
    Ok(result)
}

#[tauri::command]
/// Resolves only the matching active callback from the main window; duplicate/stale acknowledgements fail.
pub fn configure_resolve(
    attempt_id: u64,
    result: ConfigureResult,
    window: WebviewWindow,
    service: State<'_, ConfigureService>,
) -> Result<(), String> {
    require_caller(&window, MAIN_WINDOW)?;
    result.validate()?;
    let mut state = service.lock()?;
    let pending = state.pending.as_ref().ok_or(CONFIGURE_MISSING)?;
    if pending.attempt_id == CONSUMED_ATTEMPT || pending.attempt_id != attempt_id {
        return Err(CONFIGURE_MISSING.into());
    }
    pending
        .sender
        .send(result)
        .map_err(|_| DELIVERY_FAILED.to_string())?;
    // Keep the pending guard until the submit worker settles; mark this sender consumed to reject duplicate resolution.
    if let Some(pending) = state.pending.as_mut() {
        pending.attempt_id = CONSUMED_ATTEMPT;
    }
    Ok(())
}

#[tauri::command]
/// Closes the idle editor after cancellation or acknowledged success; active callbacks cannot be dismissed.
/// A successful Save also honors application exit requested while the callback was pending.
pub fn close_configure(
    window: WebviewWindow,
    app: AppHandle,
    service: State<'_, ConfigureService>,
) -> Result<(), String> {
    require_caller(&window, CONFIGURE_WINDOW)?;
    let exit = {
        let state = service.lock()?;
        if state.pending.is_some() {
            return Err(CONFIGURE_PENDING.into());
        }
        state.exit_requested && state.save_succeeded
    };
    window.close().map_err(|error| error.to_string())?;
    if exit {
        if let Some(main) = app.get_webview_window(MAIN_WINDOW) {
            main.close().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Releases native modal ownership on close or failed creation and announces dismissal to the main view.
fn reset(app: &AppHandle) {
    let service = app.state::<ConfigureService>();
    if let Ok(mut state) = service.lock() {
        state.occupied = false;
        state.original = None;
        state.pending = None;
        state.exit_requested = false;
        state.save_succeeded = false;
    }
    if let Some(main) = app.get_webview_window(MAIN_WINDOW) {
        if let Err(error) = main.set_enabled(true) {
            eprintln!("{error}");
        }
        if let Err(error) = main.emit(CONFIGURE_CLOSED_EVENT, ()) {
            eprintln!("{error}");
        }
    }
}

/// Routes editor/owner lifecycle without blocking native events or dropping a pending callback's draft.
pub fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == CONFIGURE_WINDOW {
        editor_event(window, event);
    } else if window.label() == MAIN_WINDOW {
        owner_event(window, event);
    }
}

/// Prevents native close during Save and reenables/refocuses the parent after editor destruction.
fn editor_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            let service = window.state::<ConfigureService>();
            match service.lock() {
                Ok(state) => {
                    if state.pending.is_some() {
                        api.prevent_close();
                    }
                }
                Err(_) => api.prevent_close(),
            };
        }
        tauri::WindowEvent::Destroyed => {
            reset(window.app_handle());
            if let Some(main) = window.app_handle().get_webview_window(MAIN_WINDOW) {
                if let Err(error) = main.set_focus() {
                    eprintln!("{error}");
                }
            }
        }
        _ => {}
    }
}

/// Defers application exit while a callback or editor creation is active; idle editor closure discards its local draft.
fn owner_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    let tauri::WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let service = window.state::<ConfigureService>();
    let Ok(mut state) = service.lock() else {
        api.prevent_close();
        return;
    };
    if !state.occupied {
        return;
    }
    if state.pending.is_some() || state.original.is_none() {
        state.exit_requested = true;
        api.prevent_close();
    }
}
