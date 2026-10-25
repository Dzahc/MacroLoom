//! Main-window name-only persistence with recoverable receipts independent of IPC acknowledgement.
use crate::configure_model::ConfigureResult;
use crate::library_commands::{LibraryService, LIBRARY_EVENT};
use crate::macro_format::{self, ValidationError};
use crate::repository::LibraryState;
use crate::session::Session;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

const MAIN: &str = "main";
const UNAVAILABLE: &str = "Rename is unavailable; wait for the current operation to finish";
const STALE: &str = "Rename attempt is stale; check its outcome before retrying";
const DELIVERY_FAILED: &str = "Rename library delivery failed";

#[derive(Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Explicit name-only input; the caller cannot submit paths, playback settings, or event data.
pub struct RenameInput {
    attempt_id: u64,
    macro_id: String,
    name: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Authoritative receipt; discarded failures yield to an already waiting start without reopening the editor.
pub struct RenameCompletion {
    attempt_id: u64,
    result: ConfigureResult,
    library: Option<LibraryState>,
    discarded: bool,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
/// Small recoverable operation state; querying it never repeats a disk write.
pub struct RenameStatus {
    attempt_id: u64,
    pending: bool,
    completion: Option<RenameCompletion>,
    #[serde(skip)]
    request: Option<RenameInput>,
}
#[derive(Default)]
/// Backend-owned rename receipt and submission exclusion for the single main window.
pub struct RenameService(Mutex<RenameStatus>);

/// Restricts both status and writes to the real main library and production library mode.
fn require_caller(window: &WebviewWindow) -> Result<(), String> {
    crate::app_mode::AppMode::current().require_library()?;
    if window.label() != MAIN {
        return Err(UNAVAILABLE.into());
    }
    Ok(())
}
#[tauri::command]
/// Returns the last accepted attempt and outcome; lost acknowledgements can be reconciled without resubmission.
pub fn rename_status(
    window: WebviewWindow,
    service: State<'_, RenameService>,
) -> Result<RenameStatus, String> {
    require_caller(&window)?;
    Ok(service.0.lock().map_err(|_| UNAVAILABLE)?.clone())
}
#[tauri::command]
/// Reserves an idle name-only write before dispatching disk work; stores its receipt on the worker before replying.
pub async fn rename_macro(
    input: RenameInput,
    window: WebviewWindow,
    app: AppHandle,
    service: State<'_, RenameService>,
    library: State<'_, LibraryService>,
) -> Result<RenameCompletion, String> {
    require_caller(&window)?;
    let repository = library.repository()?;
    let mut reservation = {
        let mut status = service.0.lock().map_err(|_| UNAVAILABLE)?;
        if status.request.as_ref() == Some(&input) {
            return status.completion.clone().ok_or_else(|| UNAVAILABLE.into());
        }
        validate_attempt(&status, &input)?;
        let session = app.state::<Arc<Session>>().inner().clone();
        let reservation = session.begin_rename()?;
        status.attempt_id = input.attempt_id;
        status.pending = true;
        status.completion = None;
        status.request = Some(input.clone());
        reservation
    };
    tauri::async_runtime::spawn_blocking(move || {
        let saved = repository.rename(&input.macro_id, &input.name);
        let mut completion = receipt(input.attempt_id, saved);
        let service = app.state::<RenameService>();
        let mut status = service.0.lock().map_err(|_| UNAVAILABLE)?;
        completion.discarded = reservation.finish(completion.result.ok)?;
        status.pending = false;
        status.completion = Some(completion.clone());
        drop(status);
        if let Some(library) = &completion.library {
            if let Err(error) = app.emit(LIBRARY_EVENT, library) {
                eprintln!("{DELIVERY_FAILED}: {error}");
            }
        }
        Ok(completion)
    })
    .await
    .map_err(|error| error.to_string())?
}
/// Rejects replayed/invalid sequence numbers before any operation can reach disk.
fn validate_attempt(status: &RenameStatus, input: &RenameInput) -> Result<(), String> {
    if status.pending {
        return Err(UNAVAILABLE.into());
    }
    if input.attempt_id <= status.attempt_id || input.attempt_id > macro_format::MAX_SAFE_INTEGER {
        return Err(STALE.into());
    }
    Ok(())
}
/// Converts atomic repository outcomes; a committed write with refresh trouble remains a successful Save.
fn receipt(
    attempt_id: u64,
    saved: Result<crate::repository::property_save::PropertySave, ValidationError>,
) -> RenameCompletion {
    let (result, library) = match saved {
        Ok(saved) => (
            ConfigureResult {
                ok: true,
                message: String::new(),
                fields: Default::default(),
                changed: saved.changed,
                name: saved.name,
                warning: saved.warning,
            },
            saved.library,
        ),
        Err(error) => (ConfigureResult::rejected(error), None),
    };
    RenameCompletion {
        attempt_id,
        result,
        library,
        discarded: false,
    }
}
