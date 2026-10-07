use crate::app_mode::AppMode;
use crate::window_view::{ViewRequest, ViewResult, WindowView};
use crate::windows::DesktopWindow;
use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const PRESENTATION_EVENT: &str = "presentation-state";
pub const WINDOW_VIEW_EVENT: &str = "window-view-state";
const MAIN_WINDOW: &str = "main";
pub(crate) const PREVIEW_WINDOW: &str = "compact-development";
const PREVIEW_URL: &str = "index.html?compact-development";
const PREVIEW_TITLE: &str = "MacroLoom — Compact development preview";
const PREVIEW_WIDTH: f64 = 460.0;
const PREVIEW_HEIGHT: f64 = 720.0;
const MAX_ELAPSED_MS: u64 = 9_007_199_254_740_991;
const MAX_NAME_LENGTH: usize = 1024;
const PREVIEW_DISABLED: &str =
    "Presentation preview is available only in development library builds";
const INVALID_STATUS: &str = "Invalid controlled session presentation";
const INVALID_WINDOW: &str = "Window presentation commands belong to the main library window";
const STATE_POISONED: &str = "Window presentation state is unavailable";
const VIEW_CHANNEL_CAPACITY: usize = 1;
const CALLER_DISCONNECTED: &str = "Window view caller disconnected";
const EVENT_DELIVERY_FAILED: &str = "Window view event delivery failed";
const PREVIEW_CLOSE_FAILED: &str = "Preview close restoration failed";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
/// Presentation-only phase; no variant starts input capture, injection, scheduling, or persistence.
pub enum PresentationPhase {
    Idle,
    Saving,
    Recording,
    Playing,
    Interval,
    Stopping,
}

impl PresentationPhase {
    /// Returns whether toolbar/banner occupy the compact view until cleanup completes.
    pub fn compact(self) -> bool {
        matches!(
            self,
            Self::Recording | Self::Playing | Self::Interval | Self::Stopping
        )
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Backend-owned status payload; elapsed is session time, remaining is an unscaled interval countdown.
pub struct PreviewStatus {
    pub phase: PresentationPhase,
    pub macro_name: Option<String>,
    pub elapsed_ms: u64,
    pub run: u32,
    pub total_runs: Option<u32>,
    pub remaining_ms: u64,
}

impl Default for PreviewStatus {
    /// Starts with idle presentation and no fabricated recording name.
    fn default() -> Self {
        Self {
            phase: PresentationPhase::Idle,
            macro_name: None,
            elapsed_ms: 0,
            run: 1,
            total_runs: Some(1),
            remaining_ms: 0,
        }
    }
}

impl PreviewStatus {
    /// Rejects invalid external timestamps, run counts, and oversized names before publishing status.
    pub fn validate(&self) -> Result<(), String> {
        if self.elapsed_ms > MAX_ELAPSED_MS || self.remaining_ms > MAX_ELAPSED_MS || self.run == 0 {
            return Err(INVALID_STATUS.into());
        }
        if let Some(total) = self.total_runs {
            if total == 0 || self.run > total {
                return Err(INVALID_STATUS.into());
            }
        }
        if let Some(name) = &self.macro_name {
            if name.chars().count() > MAX_NAME_LENGTH || name.trim().is_empty() {
                return Err(INVALID_STATUS.into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID_STATUS: &str = r#"{"phase":"recording","macroName":null,"elapsedMs":18000,"run":1,"totalRuns":1,"remainingMs":0}"#;
    const INVALID_PHASE: &str = "unsupported";
    const STATUS_REVISION: u64 = 7;
    const RECORDING_PHASE: &str = "recording";
    const REVISION_FIELD: &str = "revision";
    const MACRO_NAME_FIELD: &str = "macroName";
    const PHASE_FIELD: &str = "phase";
    const ELAPSED_MS_FIELD: &str = "elapsedMs";
    const NON_CONTRACT_ELAPSED_MS_FIELD: &str = "elapsed_ms";

    #[test]
    /// External controlled recording accepts no name and serializes the exact TypeScript camelCase contract.
    fn recording_payload_without_a_name_is_valid_and_revisioned() {
        let status: PreviewStatus = serde_json::from_str(VALID_STATUS)
            .expect("Known-valid contract fixture must deserialize");
        assert!(status.validate().is_ok());
        let snapshot = PresentationSnapshot {
            revision: STATUS_REVISION,
            status,
        };
        let json = serde_json::to_value(snapshot).expect("Owned serializable status cannot fail");
        assert_eq!(json[REVISION_FIELD], STATUS_REVISION);
        assert!(json[MACRO_NAME_FIELD].is_null());
        assert_eq!(json[PHASE_FIELD], RECORDING_PHASE);
        assert!(json.get(ELAPSED_MS_FIELD).is_some());
        assert!(json.get(NON_CONTRACT_ELAPSED_MS_FIELD).is_none());
    }

    #[test]
    /// External payloads cannot publish unsupported phases, unsafe timestamps, or invalid run counts.
    fn invalid_external_status_is_rejected_before_publishing() {
        let invalid = VALID_STATUS.replace(RECORDING_PHASE, INVALID_PHASE);
        assert!(serde_json::from_str::<PreviewStatus>(&invalid).is_err());
        let mut status = PreviewStatus {
            elapsed_ms: MAX_ELAPSED_MS + 1,
            ..PreviewStatus::default()
        };
        assert_eq!(status.validate(), Err(INVALID_STATUS.into()));
        status.elapsed_ms = 0;
        status.run = 0;
        assert_eq!(status.validate(), Err(INVALID_STATUS.into()));
        status.run = 1;
        status.total_runs = Some(0);
        assert_eq!(status.validate(), Err(INVALID_STATUS.into()));
        status.total_runs = None;
        assert!(status.validate().is_ok());
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Revisioned snapshot prevents late command replies from replacing newer controlled status events.
pub struct PresentationSnapshot {
    pub revision: u64,
    #[serde(flatten)]
    pub status: PreviewStatus,
}

/// Managed presentation store and native full-placement owner; main-thread mutations keep HWND lifetime clear.
pub struct PresentationService {
    pub view: Mutex<WindowView<DesktopWindow>>,
    status: Mutex<PresentationSnapshot>,
}

impl PresentationService {
    /// Binds only the owned main window; initialization errors prevent invalid-handle commands.
    pub fn new(window: WebviewWindow) -> Result<Self, String> {
        Ok(Self {
            view: Mutex::new(WindowView::new(DesktopWindow::new(window)?)),
            status: Mutex::new(PresentationSnapshot {
                revision: 0,
                status: PreviewStatus::default(),
            }),
        })
    }
    /// Returns an owned revisioned status snapshot without native or disk side effects.
    fn snapshot(&self) -> Result<PresentationSnapshot, String> {
        self.status
            .lock()
            .map(|state| state.clone())
            .map_err(|_| STATE_POISONED.into())
    }
    /// Validates and updates controlled status; does not perform session actions.
    fn publish(&self, status: PreviewStatus) -> Result<PresentationSnapshot, String> {
        status.validate()?;
        let mut current = self.status.lock().map_err(|_| STATE_POISONED)?;
        current.revision += 1;
        current.status = status;
        Ok(current.clone())
    }
}

/// Enforces development and library-mode preconditions at the backend, independent of hidden controls.
pub(crate) fn require_preview() -> Result<(), String> {
    if !cfg!(debug_assertions) || AppMode::current().input_prototype {
        return Err(PREVIEW_DISABLED.into());
    }
    Ok(())
}

#[tauri::command]
/// Returns current presentation for listener-first startup; no input or session work is started.
pub fn presentation_snapshot(
    service: State<'_, PresentationService>,
) -> Result<PresentationSnapshot, String> {
    service.snapshot()
}

#[tauri::command]
/// Publishes validated development presentation directly; release/prototype builds reject before state changes.
pub fn preview_presentation(
    status: PreviewStatus,
    app: AppHandle,
    service: State<'_, PresentationService>,
) -> Result<PresentationSnapshot, String> {
    require_preview()?;
    let snapshot = service.publish(status)?;
    app.emit_to(MAIN_WINDOW, PRESENTATION_EVENT, &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
/// Opens separate development controls asynchronously; synchronous Windows IPC would deadlock WebView2 creation on the event thread.
pub async fn open_presentation_preview(app: AppHandle) -> Result<(), String> {
    require_preview()?;
    if let Some(window) = app.get_webview_window(PREVIEW_WINDOW) {
        window.show().map_err(|error| error.to_string())?;
        return window.set_focus().map_err(|error| error.to_string());
    }
    WebviewWindowBuilder::new(&app, PREVIEW_WINDOW, WebviewUrl::App(PREVIEW_URL.into()))
        .title(PREVIEW_TITLE)
        .inner_size(PREVIEW_WIDTH, PREVIEW_HEIGHT)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Applies native work on the event thread with main-window and phase preconditions.
fn change_view(app: &AppHandle, request: ViewRequest) -> Result<ViewResult, String> {
    let service = app.state::<PresentationService>();
    let compact = service.snapshot()?.status.phase.compact();
    let mut view = service.view.lock().map_err(|_| STATE_POISONED)?;
    // A previously queued measurement may arrive after the phase changes; ignore it without mutating native state or displaying a false failure.
    if request.compact != compact {
        return Ok(view.current());
    }
    Ok(view.update(request))
}

#[tauri::command]
/// Serializes native changes on the UI thread; waiting happens on a worker, never blocking native event dispatch.
pub async fn set_window_view(
    request: ViewRequest,
    window: WebviewWindow,
    app: AppHandle,
) -> Result<ViewResult, String> {
    AppMode::current().require_library()?;
    if window.label() != MAIN_WINDOW {
        return Err(INVALID_WINDOW.into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let (sender, receiver) = mpsc::sync_channel(VIEW_CHANNEL_CAPACITY);
        let event_app = app.clone();
        app.run_on_main_thread(move || {
            // An abandoned IPC receiver needs no state rollback: actual view remains owned by the main window.
            if sender.send(change_view(&event_app, request)).is_err() {
                eprintln!("{CALLER_DISCONNECTED}");
            }
        })
        .map_err(|error| error.to_string())?;
        receiver.recv().map_err(|error| error.to_string())?
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Keeps a moved/DPI-changed compact frame fitted; recursive events during an owned transition are skipped safely.
pub fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == PREVIEW_WINDOW {
        preview_window_event(window, event);
        return;
    }
    if window.label() != MAIN_WINDOW {
        return;
    }
    match event {
        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => {}
        _ => return,
    }
    let Some(service) = window.try_state::<PresentationService>() else {
        return;
    };
    let result = match service.view.try_lock() {
        Ok(mut view) => {
            if !view.current().compact {
                return;
            }
            view.refresh()
        }
        // A synchronous SetWindowPos callback occurs inside the current transition; it already fits this frame.
        Err(std::sync::TryLockError::WouldBlock) => return,
        Err(std::sync::TryLockError::Poisoned(_)) => {
            eprintln!("{STATE_POISONED}");
            return;
        }
    };
    if let Err(error) = window.emit(WINDOW_VIEW_EVENT, result) {
        eprintln!("{EVENT_DELIVERY_FAILED}: {error}");
    }
}

/// Restores idle presentation when development controls close, keeping the main window reachable without session actions.
fn preview_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if !matches!(event, tauri::WindowEvent::Destroyed) {
        return;
    }
    let Some(service) = window.try_state::<PresentationService>() else {
        return;
    };
    let result = service
        .publish(PreviewStatus::default())
        .and_then(|snapshot| {
            window
                .app_handle()
                .emit_to(MAIN_WINDOW, PRESENTATION_EVENT, snapshot)
                .map_err(|error| error.to_string())
        });
    if let Err(error) = result {
        eprintln!("{PREVIEW_CLOSE_FAILED}: {error}");
    }
}
