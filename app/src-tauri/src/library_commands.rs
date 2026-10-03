use crate::macro_format::MacroDocument;
use crate::repository::{LibraryState, Repository};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub const LIBRARY_EVENT: &str = "library-state";

/// Managed repository initialization result; path failures stay user-visible without storage fallback.
pub struct LibraryService(pub Result<Arc<Repository>, String>);
impl LibraryService {
    /// Resolves the process executable without disk I/O; directory setup occurs on the loading worker.
    pub fn new() -> Self {
        Self(
            std::env::current_exe()
                .map_err(|error| error.to_string())
                .and_then(|executable| Repository::beside_executable(&executable))
                .map(Arc::new),
        )
    }
    /// Returns shared repository ownership, propagating the original initialization error.
    fn repository(&self) -> Result<Arc<Repository>, String> {
        self.0.clone()
    }
}

#[tauri::command]
/// Starts idempotent discovery away from UI/native loops; emits revisioned metadata during validation.
pub async fn load_library(
    app: AppHandle,
    service: State<'_, LibraryService>,
) -> Result<LibraryState, String> {
    let repository = service.repository()?;
    tauri::async_runtime::spawn_blocking(move || {
        repository.load(|state| {
            // Final command state also contains all diagnostics if delivery fails or no listener exists.
            if let Err(error) = app.emit(LIBRARY_EVENT, state) {
                eprintln!("Library update delivery failed: {error}");
            }
        });
        repository.snapshot()
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
/// Obtains a validated selected snapshot lazily; IDs resolve only through trusted loaded associations.
/// Later Configure/Delete/Play consumers use this boundary; this command never writes or injects input.
pub async fn macro_snapshot(
    macro_id: String,
    service: State<'_, LibraryService>,
) -> Result<MacroDocument, String> {
    let repository = service.repository()?;
    tauri::async_runtime::spawn_blocking(move || repository.read(&macro_id))
        .await
        .map_err(|error| error.to_string())?
}
