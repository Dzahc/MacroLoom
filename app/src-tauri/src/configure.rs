//! Native singleton Configure window, worker persistence and recoverable authoritative save results.

use crate::app_mode::AppMode;
use crate::configure_model::{
    ConfigureCompletion, ConfigureDraft, ConfigureDraftInput, ConfigureResult, ConfigureState,
    ConfigureStatus, CONFIGURE_BUSY, CONFIGURE_MISSING, CONFIGURE_PENDING,
};
use crate::library_commands::LibraryService;
use crate::library_commands::LIBRARY_EVENT;
use crate::repository::Repository;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const CONFIGURE_WINDOW: &str = "configure";
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
const SAVED_LIBRARY_DELIVERY_FAILED: &str = "Saved library update delivery failed";

#[derive(Default)]
/// Managed singleton editor interlock shared by native commands, persistence workers, and window lifecycle events.
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
/// Validates and persists a complete draft on a worker; native state owns the commit and the recoverable outcome.
/// Pending Saves cannot be dismissed. Repeating a committed request returns its receipt without another write.
pub async fn configure_submit(
    draft: ConfigureDraftInput,
    window: WebviewWindow,
    app: AppHandle,
    service: State<'_, ConfigureService>,
    library: State<'_, LibraryService>,
) -> Result<ConfigureResult, String> {
    require_caller(&window, CONFIGURE_WINDOW)?;
    AppMode::current().require_library()?;
    let repository = library.repository()?;
    if let Some(completed) = service.lock()?.completion.clone() {
        if completed.result.ok {
            return Ok(completed.result);
        }
    }
    let (attempt_id, draft) = match prepare_submission(&service, draft) {
        Ok(submission) => submission,
        Err(error) => return Ok(ConfigureResult::rejected(error)),
    };
    let state = service.inner.clone();
    // Store the completion on the worker itself, before IPC acknowledgement can fail or its future is dropped.
    tauri::async_runtime::spawn_blocking(move || {
        let mut completion = persist(&repository, draft, attempt_id);
        publish_saved(&app, &mut completion);
        let mut guard = state.lock().map_err(|_| LOCK_FAILED.to_string())?;
        guard.complete(completion)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Validates and claims the draft under one short-lived guard; malformed submissions retain field-specific feedback.
fn prepare_submission(
    service: &ConfigureService,
    draft: ConfigureDraftInput,
) -> Result<(u64, ConfigureDraft), crate::macro_format::ValidationError> {
    let mut state = service.lock().map_err(retained_error)?;
    let original = state
        .original
        .as_ref()
        .ok_or_else(|| retained_error(CONFIGURE_MISSING.into()))?;
    let draft = draft.validate(original)?;
    let submission = state.begin(draft)?;
    let draft = state.claim(submission.attempt_id).map_err(retained_error)?;
    Ok((submission.attempt_id, draft))
}

/// Maps native precommit state errors to actionable retained feedback without claiming persistence.
fn retained_error(message: String) -> crate::macro_format::ValidationError {
    crate::macro_format::ValidationError::new(crate::repository::DIAGNOSTIC_STORAGE, message)
}

/// Converts the repository receipt to an authoritative completion; only errors before disk commit are ordinary failures.
fn persist(repository: &Repository, draft: ConfigureDraft, attempt_id: u64) -> ConfigureCompletion {
    match repository.save_properties(&draft.macro_id, &draft.name, &draft.playback) {
        Ok(saved) => ConfigureCompletion {
            attempt_id,
            result: ConfigureResult {
                ok: true,
                message: String::new(),
                fields: Default::default(),
                changed: saved.changed,
                name: saved.name,
                warning: saved.warning,
            },
            library: saved.library,
        },
        Err(error) => ConfigureCompletion {
            attempt_id,
            result: ConfigureResult::rejected(error),
            library: None,
        },
    }
}

/// Publishes committed metadata without changing focus; a delivery problem cannot reverse or misreport the commit.
fn publish_saved(app: &AppHandle, completion: &mut ConfigureCompletion) {
    if let Some(library) = &completion.library {
        if let Err(error) = app.emit(LIBRARY_EVENT, library) {
            eprintln!("{SAVED_LIBRARY_DELIVERY_FAILED}: {error}");
        }
    }
}

#[tauri::command]
/// Returns the last native completion to the editor or owner for reconciliation; performs no disk writes or acknowledgements.
pub fn configure_status(
    window: WebviewWindow,
    service: State<'_, ConfigureService>,
) -> Result<ConfigureStatus, String> {
    if window.label() != MAIN_WINDOW && window.label() != CONFIGURE_WINDOW {
        return Err(INVALID_CALLER.into());
    }
    let state = service.lock()?;
    Ok(ConfigureStatus {
        pending: state.pending.is_some(),
        completion: state.completion.clone(),
    })
}

#[tauri::command]
/// Closes the idle editor after cancellation or acknowledged success; active workers cannot be dismissed.
/// A successful Save also honors application exit requested while the worker was pending.
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

/// Routes editor/owner lifecycle without blocking native events or dropping a pending worker's draft.
pub fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == CONFIGURE_WINDOW {
        editor_event(window, event);
    } else if window.label() == MAIN_WINDOW {
        owner_event(window, event);
    }
}

/// Prevents native close during Save and reenables the owner before destruction without forcing foreground activation.
fn editor_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            let service = window.state::<ConfigureService>();
            match service.lock() {
                Ok(state) => {
                    if state.pending.is_some() {
                        api.prevent_close();
                        return;
                    }
                }
                Err(_) => {
                    api.prevent_close();
                    return;
                }
            };
            // An enabled owner participates in Windows' normal close-time activation.
            // No foreground request is made when another application owns focus.
            if let Some(main) = window.app_handle().get_webview_window(MAIN_WINDOW) {
                if let Err(error) = main.set_enabled(true) {
                    api.prevent_close();
                    eprintln!("{error}");
                }
            }
        }
        tauri::WindowEvent::Destroyed => {
            reset(window.app_handle());
        }
        _ => {}
    }
}

/// Defers application exit while a worker or editor creation is active; idle editor closure discards its local draft.
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
