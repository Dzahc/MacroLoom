//! Event-thread Configure placement and recoverable main-window actions; no draft is copied or resubmitted.

use crate::app_mode::AppMode;
use crate::configure::{ConfigureService, CONFIGURE_WINDOW};
use crate::configure_model::{ConfigureResult, CONFIGURE_MISSING};
use crate::windows;
use serde::Serialize;
use std::sync::mpsc;
use tauri::{AppHandle, Manager, State, WebviewWindow};

const MAIN: &str = "main";
const CHANNEL_CAPACITY: usize = 1;
const INVALID_CALLER: &str = "Configure recovery is available only in the main window";
const CALLER_DISCONNECTED: &str = "Configure placement caller disconnected";

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
/// Authoritative main-window recovery presentation; pending saves always block Discard.
pub struct RecoveryStatus {
    pub occupied: bool,
    pub needed: bool,
    pub pending: bool,
    pub message: String,
    pub completion: Option<RecoveryReceipt>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Compact authoritative save outcome presented to the recovering owner before it can explicitly discard that attempt.
pub struct RecoveryReceipt {
    pub attempt_id: u64,
    pub result: ConfigureResult,
}

#[derive(Clone, Copy)]
/// Serialized native operation; only Open installs an observer and chooses the default content size.
enum Action {
    Open,
    Retry,
    Discard(Option<u64>),
}

/// Verifies native callers independently of frontend availability/capabilities.
fn require_main(window: &WebviewWindow) -> Result<(), String> {
    AppMode::current().require_library()?;
    if window.label() != MAIN {
        return Err(INVALID_CALLER.into());
    }
    Ok(())
}

/// Returns native recovery flags under a short-lived lock, without touching the editor's form or save outcome.
fn snapshot(service: &ConfigureService) -> Result<RecoveryStatus, String> {
    let state = service.lock()?;
    Ok(RecoveryStatus {
        occupied: state.occupied,
        needed: state.recovery_error.is_some(),
        pending: state.pending.is_some(),
        message: state.recovery_error.clone().unwrap_or_default(),
        completion: state.completion.as_ref().map(|completed| RecoveryReceipt {
            attempt_id: completed.attempt_id,
            result: completed.result.clone(),
        }),
    })
}

#[tauri::command]
/// Reads authoritative recovery availability; failed queries do not authorize dismissal or another editor.
pub fn configure_recovery_status(
    window: WebviewWindow,
    service: State<'_, ConfigureService>,
) -> Result<RecoveryStatus, String> {
    require_main(&window)?;
    snapshot(&service)
}

#[tauri::command]
/// Retries placement of the same editor, retaining all draft strings and any active Save without resubmission.
pub async fn retry_configure_placement(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<RecoveryStatus, String> {
    require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || dispatch(&app, Action::Retry))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
/// Explicitly discards only a failed-placement editor after its save has settled; backend guards reject races.
pub async fn discard_configure_placement(
    observed_attempt_id: Option<u64>,
    window: WebviewWindow,
    app: AppHandle,
) -> Result<RecoveryStatus, String> {
    require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        dispatch(&app, Action::Discard(observed_attempt_id))
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Finishes hidden editor creation on its owning thread; called from asynchronous IPC after WebView2 initialization.
pub fn open(app: &AppHandle) -> Result<(), String> {
    dispatch(app, Action::Open).map(|_| ())
}

/// Dispatches native mutations to the event thread; only the non-event caller waits for completion.
fn dispatch(app: &AppHandle, action: Action) -> Result<RecoveryStatus, String> {
    let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
    let event_app = app.clone();
    app.run_on_main_thread(move || {
        if sender.send(perform(&event_app, action)).is_err() {
            eprintln!("{CALLER_DISCONNECTED}");
        }
    })
    .map_err(|error| error.to_string())?;
    receiver.recv().map_err(|error| error.to_string())?
}

/// Routes serialized opening/recovery/discard operations against the current singleton window.
fn perform(app: &AppHandle, action: Action) -> Result<RecoveryStatus, String> {
    let service = app.state::<ConfigureService>();
    let editor = app
        .get_webview_window(CONFIGURE_WINDOW)
        .ok_or(CONFIGURE_MISSING)?;
    let main = app.get_webview_window(MAIN).ok_or(CONFIGURE_MISSING)?;
    match action {
        Action::Open => open_editor(app, &main, &editor)?,
        Action::Retry => {
            if !snapshot(&service)?.needed {
                return Err(CONFIGURE_MISSING.into());
            }
            recover(app, &main, &editor)?;
        }
        Action::Discard(observed_attempt) => discard(&service, &editor, observed_attempt)?,
    }
    snapshot(&service)
}

/// Places a newly built hidden editor and installs its owned display observer before returning successful creation.
fn open_editor(
    app: &AppHandle,
    main: &WebviewWindow,
    editor: &WebviewWindow,
) -> Result<(), String> {
    place(main, editor, true)?;
    let observed = app.clone();
    windows::watch_configure_displays(handle(editor)?, move || check(&observed))
}

/// Extracts a borrowed live HWND only for synchronous event-thread native work.
fn handle(window: &WebviewWindow) -> Result<isize, String> {
    window
        .hwnd()
        .map(|hwnd| hwnd.0 as isize)
        .map_err(|error| error.to_string())
}

/// Fits and shows the existing editor before disabling the owner, requesting focus only when MacroLoom already owns foreground.
fn place(main: &WebviewWindow, editor: &WebviewWindow, initial: bool) -> Result<(), String> {
    let owner = handle(main)?;
    let target = handle(editor)?;
    let focus = windows::configure_has_focus(owner) || windows::configure_has_focus(target);
    windows::place_configure(owner, target, initial)?;
    windows::show_configure(target)?;
    main.set_enabled(false).map_err(|error| error.to_string())?;
    if focus {
        editor.set_focus().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Retains failures as actionable main-window state; successful placement restores modal ownership without replacing the document.
fn recover(app: &AppHandle, main: &WebviewWindow, editor: &WebviewWindow) -> Result<(), String> {
    let result = place(main, editor, false);
    let service = app.state::<ConfigureService>();
    service.lock()?.recovery_error = result.as_ref().err().cloned();
    if result.is_err() {
        main.set_enabled(true).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Runs after native topology/DPI processing; a reachable user-positioned editor is never recentered.
fn check(app: &AppHandle) {
    if let Err(error) = check_current(app) {
        eprintln!("{error}");
    }
}

/// Checks real bounds and recovers only an inaccessible editor, preserving active worker ownership and local edits.
fn check_current(app: &AppHandle) -> Result<(), String> {
    let Some(editor) = app.get_webview_window(CONFIGURE_WINDOW) else {
        return Ok(());
    };
    let main = app.get_webview_window(MAIN).ok_or(CONFIGURE_MISSING)?;
    let needed = snapshot(&app.state::<ConfigureService>())?.needed;
    if needed || !windows::configure_reachable(handle(&main)?, handle(&editor)?).unwrap_or(false) {
        recover(app, &main, &editor)?;
    }
    Ok(())
}

/// Reserves dismissal atomically before closing; failed native closure restores the draft's ability to Save/Retry.
fn discard(
    service: &ConfigureService,
    editor: &WebviewWindow,
    observed_attempt: Option<u64>,
) -> Result<(), String> {
    service.lock()?.reserve_recovery_discard(observed_attempt)?;
    if let Err(error) = editor.close() {
        service.lock()?.discard_requested = false;
        return Err(error.to_string());
    }
    Ok(())
}
