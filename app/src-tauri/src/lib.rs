//! Desktop shell and typed service registration for backend-owned sessions.
mod app_mode;
mod configure;
mod configure_model;
mod input;
mod library_commands;
pub mod macro_format;
mod presentation;
pub mod repository;
mod session;
mod session_control;
mod session_native;
mod session_publication;
mod shortcuts;
mod window_view;
mod windows;

use app_mode::AppMode;
pub use input::{EventKind, Held, MouseButton};
use session::Session;
use std::sync::Arc;
use tauri::{Manager, State};

const DESKTOP_START_FAILURE: &str = "failed to run MacroLoom desktop event loop";
const MAIN_WINDOW_LABEL: &str = "main";
const MAIN_WINDOW_UNAVAILABLE: &str = "Main window unavailable";

#[tauri::command]
/// Returns compile-time launch metadata; ordinary library builds now own real recording controls.
fn app_mode() -> AppMode {
    AppMode::current()
}
#[tauri::command]
/// Compatibility snapshot for the explicitly selected prototype view, without changing ownership.
fn snapshot(session: State<'_, Arc<Session>>) -> Result<session::Snapshot, String> {
    session.snapshot()
}

/// Runs the desktop event loop after physical DPI initialization; a startup failure is fatal because no recovery window exists.
pub fn run() {
    windows::enable_physical_dpi();
    desktop_builder(!cfg!(feature = "native-checks"))
        .run(tauri::generate_context!())
        .expect(DESKTOP_START_FAILURE);
}
/// Runs the development-only Windows shortcut-conflict fixture; reserves a key only until Enter/error exit.
pub fn hold_conflict_shortcut() -> Result<(), String> {
    windows::hold_conflict_shortcut()
}
/// Registers services and lifecycle owners; input_enabled selects real hooks or an isolated presentation-only check.
fn desktop_builder(input_enabled: bool) -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        // Always filters Tao raw device events, removing its keyboard registration that suppresses our focused-window hooks.
        .device_event_filter(tauri::DeviceEventFilter::Always)
        .setup(move |app| {
            app.manage(configure::ConfigureService::default());
            app.manage(library_commands::LibraryService::new());
            let window = app
                .get_webview_window(MAIN_WINDOW_LABEL)
                .ok_or(MAIN_WINDOW_UNAVAILABLE)?;
            app.manage(presentation::PresentationService::new(window.clone())?);
            let session = Session::new(Arc::new(session_native::NativePlatform::new(
                app.handle().clone(),
            )));
            app.manage(session.clone());
            // The isolated presentation-check executable deliberately starts no input hooks.
            if input_enabled {
                let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as isize;
                windows::start_hooks(session.clone(), hwnd)?;
                windows::watch_suspend(hwnd)?;
                session_native::start_status(session);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            configure::window_event(window, event);
            session_native::window_event(window, event);
            presentation::window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            app_mode,
            snapshot,
            session_native::session_snapshot,
            session_native::start_recording,
            session_native::stop,
            session_native::play,
            session_native::retry_shortcuts,
            session_native::session_confirmation,
            session_native::session_keep_open,
            session_native::session_discard_exit,
            library_commands::load_library,
            library_commands::macro_snapshot,
            library_commands::delete_macro,
            configure::open_configure,
            configure::configure_snapshot,
            configure::configure_submit,
            configure::configure_status,
            configure::close_configure,
            presentation::presentation_snapshot,
            presentation::preview_presentation,
            presentation::open_presentation_preview,
            presentation::set_window_view
        ])
}
#[cfg(feature = "native-checks")]
mod preview_window_tests;
#[cfg(feature = "native-checks")]
/// Runs the isolated real-WebView check without session capture hooks; observes actual tagged keyboard delivery and closes its own windows.
pub fn check_development_preview() {
    const CHECK_SUPPORTED: &str =
        "Native preview checks require development library mode without input hooks";
    presentation::require_preview().expect(CHECK_SUPPORTED);
    preview_window_tests::preview_window_loads_and_closes_through_real_ipc();
}

#[cfg(feature = "native-checks")]
/// Runs bounded real-registration and focused-window control checks; requires Vite and closes only its test windows.
pub fn check_session_controls() {
    presentation::require_preview().expect("Session checks require development library mode");
    windows::check_session_controls();
}
