//! Explicit real-WebView integration evidence, separate from deterministic layout and recovery-policy tests.

use crate::{
    configure::CONFIGURE_WINDOW, desktop_builder_with_library, library_commands::LibraryService,
    repository::Repository, windows,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};
use tauri::{webview::PageLoadEvent, AppHandle, Manager, WebviewWindow};

const TIMEOUT: Duration = Duration::from_secs(45);
const POLL: Duration = Duration::from_millis(50);
const CHANNEL_CAPACITY: usize = 1;
const MAIN: &str = "main";
const SUCCESS: i32 = 0;
const FAILURE: i32 = 1;
const OFFSET: i32 = 60;
const OFFSCREEN: i32 = 30_000;
const FIXTURE: &str = r#"{"schemaVersion":1,"id":"68833c94-3d18-4fb2-8a59-0cc72382b616","name":"Configure placement fixture","createdAt":"2026-09-28T21:32:08Z","updatedAt":"2026-09-28T21:32:08Z","recording":{"platform":"windows","coordinateSpace":"screen_physical_pixels","keyboardLayout":"00000409","displays":[{"x":0,"y":0,"width":1920,"height":1080,"scaleFactor":1.0}]},"durationMs":0,"playback":{"speed":1.0,"repeatMode":"once","totalRuns":1,"intervalMs":0},"events":[{"atMs":0,"type":"key_up","key":"KeyA","native":{"scanCode":30,"virtualKey":65,"extended":false}}]}"#;
const OPEN: &str = "window.__TAURI_INTERNALS__.invoke('open_configure',{macroId:'68833c94-3d18-4fb2-8a59-0cc72382b616'}).catch(e => document.body.dataset.configureCheckError=String(e))";
const MAIN_READY: &str = "!!document.querySelector('.macro-library')";
const NAME_SELECTED: &str = "document.activeElement?.id === 'configure-name' && document.activeElement.selectionStart === 0 && document.activeElement.selectionEnd === document.activeElement.value.length";
const EDIT_NAME: &str = "(()=>{const e=document.querySelector('#configure-name'); const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;set.call(e,'Retained after display recovery');e.dispatchEvent(new Event('input',{bubbles:true}));return e.value==='Retained after display recovery'})()";
const DRAFT_RETAINED: &str =
    "document.querySelector('#configure-name')?.value === 'Retained after display recovery'";
const CANCEL: &str = "(()=>{const buttons=Array.from(document.querySelectorAll('button'));buttons.find(b=>b.textContent==='Cancel').click();return true})()";
const DOM_ERROR: &str = "document.body.dataset.configureCheckError || document.body.innerText";
const HAS_OPEN_ERROR: &str = "Boolean(document.body.dataset.configureCheckError)";
const OUTPUT_DIRECTORY: &str = "../../.quality-output/ml-47-native";
const FIXTURE_FILE: &str = "fixture.json";
const TIMED_OUT: &str = "Configure native check timed out";
const EDITOR_DISAPPEARED: &str = "Opened editor disappeared";
const OWNER_MOVED_EDITOR: &str = "Open Configure followed owner movement";
const EDITOR_UNREACHABLE: &str = "Editor frame is not reachable";
const OWNER_NOT_DISABLED: &str = "Main must be disabled while Configure is owned";
const WRONG_DISPLAY: &str = "Editor opened on the wrong display";
const CANCEL_DISABLED_OWNER: &str = "Cancel left the main window disabled";
const EDIT_FAILED: &str = "Could not edit the native test draft";
const FOCUS_STOLEN: &str = "Configure recovery stole foreground focus";
const DRAFT_LOST: &str = "Recovery lost unsaved editor text";
const WATCHDOG_FAILED: &str = "FAIL Configure event-loop watchdog";
const LIBRARY_FAILED: &str = "Isolated Configure fixture library";
const MAIN_FAILED: &str = "Loaded native main window";
const BUILD_FAILED: &str = "Native Configure integration app builds";
const CHECK_FAILED: &str = "Real Configure checks must pass";

/// Executes a boolean observation in a real WebView; malformed/failed/abandoned evaluations are failures.
fn evaluate(window: &WebviewWindow, expression: &str) -> Result<bool, String> {
    let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
    window
        .as_ref()
        .eval_with_callback(expression, move |result| {
            let _ = sender.send(result);
        })
        .map_err(|error| error.to_string())?;
    let result = receiver
        .recv_timeout(TIMEOUT)
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&result).map_err(|error| format!("{error}: {result}"))
}

/// Polls real DOM/native state on a worker, leaving the event loop available for WebView2 initialization and callbacks.
fn wait(mut ready: impl FnMut() -> Result<bool, String>) -> Result<(), String> {
    let started = Instant::now();
    while !ready()? {
        if started.elapsed() > TIMEOUT {
            return Err(TIMED_OUT.into());
        }
        std::thread::sleep(POLL);
    }
    Ok(())
}

/// Resolves the one actual editor created by IPC, reporting any actionable opening failure from the main WebView.
fn editor(app: &AppHandle, main: &WebviewWindow) -> Result<WebviewWindow, String> {
    wait(|| {
        if evaluate(main, HAS_OPEN_ERROR)? {
            let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
            main.as_ref()
                .eval_with_callback(DOM_ERROR, move |result| {
                    let _ = sender.send(result);
                })
                .map_err(|error| error.to_string())?;
            return Err(receiver
                .recv_timeout(TIMEOUT)
                .map_err(|error| error.to_string())?);
        }
        Ok(app.get_webview_window(CONFIGURE_WINDOW).is_some())
    })?;
    app.get_webview_window(CONFIGURE_WINDOW)
        .ok_or_else(|| EDITOR_DISAPPEARED.into())
}

/// Checks normal Cancel and repeats opening from the latest main-window position on every connected display.
fn check_displays(app: &AppHandle, main: &WebviewWindow) -> Result<(), String> {
    let owner = main.hwnd().map_err(|error| error.to_string())?.0 as isize;
    for display in windows::configure_displays(owner)? {
        let editor = open_on_display(app, main, &display)?;
        check_visible_owner(main, &editor, owner)?;
        check_destination(&editor, display.work)?;
        eprintln!("Configure native check: checking owner movement");
        check_owner_movement(main, &editor)?;
        eprintln!("Configure native check: checking nonactivating recovery");
        check_recovery(app, main, &editor)?;
        check_cancel(app, main, &editor)?;
        eprintln!("PASS Configure display {:?}; initial Name selection, visible frame, retained draft, observer recovery, and Cancel", display);
    }
    Ok(())
}

/// Moves the owned main window to a real display before invoking the production opening command and checking initial focus.
fn open_on_display(
    app: &AppHandle,
    main: &WebviewWindow,
    display: &crate::configure_placement::Display,
) -> Result<WebviewWindow, String> {
    main.set_position(tauri::PhysicalPosition::new(
        display.work.left + OFFSET,
        display.work.top + OFFSET,
    ))
    .map_err(|error| error.to_string())?;
    main.set_focus().map_err(|error| error.to_string())?;
    main.eval(OPEN).map_err(|error| error.to_string())?;
    eprintln!("Configure native check: opening on {:?}", display);
    let editor = editor(app, main)?;
    eprintln!("Configure native check: editor created; waiting for selected Name input");
    wait(|| evaluate(&editor, NAME_SELECTED))?;
    eprintln!(
        "Configure native check: editor scale {}",
        editor.scale_factor().map_err(|error| error.to_string())?
    );
    Ok(editor)
}

/// Moving the main frame after opening must preserve the editor's independently chosen placement.
fn check_owner_movement(main: &WebviewWindow, editor: &WebviewWindow) -> Result<(), String> {
    let before = editor.outer_position().map_err(|error| error.to_string())?;
    let owner = main.outer_position().map_err(|error| error.to_string())?;
    main.set_position(tauri::PhysicalPosition::new(
        owner.x + OFFSET,
        owner.y + OFFSET,
    ))
    .map_err(|error| error.to_string())?;
    let _ = main.outer_position().map_err(|error| error.to_string())?;
    if editor.outer_position().map_err(|error| error.to_string())? != before {
        return Err(OWNER_MOVED_EDITOR.into());
    }
    Ok(())
}

/// Independently observes native owner disabling and actual editor reachability after successful opening.
fn check_visible_owner(
    main: &WebviewWindow,
    editor: &WebviewWindow,
    owner: isize,
) -> Result<(), String> {
    let target = editor.hwnd().map_err(|error| error.to_string())?.0 as isize;
    if !windows::configure_reachable(owner, target)? {
        return Err(EDITOR_UNREACHABLE.into());
    }
    if main.is_enabled().map_err(|error| error.to_string())? {
        return Err(OWNER_NOT_DISABLED.into());
    }
    Ok(())
}

/// Checks the actual editor origin belongs to the intended display rather than another reachable display.
fn check_destination(
    editor: &WebviewWindow,
    work: crate::configure_placement::Rect,
) -> Result<(), String> {
    let position = editor.outer_position().map_err(|error| error.to_string())?;
    if position.x < work.left
        || position.x >= work.right
        || position.y < work.top
        || position.y >= work.bottom
    {
        return Err(WRONG_DISPLAY.into());
    }
    Ok(())
}

/// Cancels through the actual form, checking native destruction and restoration of main-window access.
fn check_cancel(
    app: &AppHandle,
    main: &WebviewWindow,
    editor: &WebviewWindow,
) -> Result<(), String> {
    evaluate(editor, CANCEL)?;
    wait(|| Ok(app.get_webview_window(CONFIGURE_WINDOW).is_none()))?;
    if !main.is_enabled().map_err(|error| error.to_string())? {
        return Err(CANCEL_DISABLED_OWNER.into());
    }
    Ok(())
}

/// Moves only this disposable editor offscreen, then sends a real work-area notification through its native subclass.
/// The same WebView must become reachable with its unsaved field value intact.
fn check_recovery(
    app: &AppHandle,
    main: &WebviewWindow,
    editor: &WebviewWindow,
) -> Result<(), String> {
    if !evaluate(editor, EDIT_NAME)? {
        return Err(EDIT_FAILED.into());
    }
    let target = focus_fixture(app)?;
    recover_offscreen(main, editor)?;
    if !windows::configure_has_focus(target) {
        return Err(FOCUS_STOLEN.into());
    }
    event_native(app, move || {
        windows::destroy_configure_focus_fixture(target).map(|_| 0)
    })?;
    if !evaluate(editor, DRAFT_RETAINED)? {
        return Err(DRAFT_LOST.into());
    }
    Ok(())
}

/// Creates and focuses a separate disposable blank window; product recovery must preserve this unrelated foreground owner.
fn focus_fixture(app: &AppHandle) -> Result<isize, String> {
    let target = event_native(app, windows::create_configure_focus_fixture)?;
    wait(|| Ok(windows::configure_has_focus(target)))?;
    Ok(target)
}

/// Runs short native fixture creation/destruction on the event thread, waiting only on this check's worker.
fn event_native(
    app: &AppHandle,
    job: impl FnOnce() -> Result<isize, String> + Send + 'static,
) -> Result<isize, String> {
    let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
    app.run_on_main_thread(move || {
        let _ = sender.send(job());
    })
    .map_err(|error| error.to_string())?;
    receiver
        .recv_timeout(TIMEOUT)
        .map_err(|error| error.to_string())?
}

/// Exercises the native observer with an inaccessible editor while leaving the real event loop free to recover it.
fn recover_offscreen(main: &WebviewWindow, editor: &WebviewWindow) -> Result<(), String> {
    editor
        .set_position(tauri::PhysicalPosition::new(OFFSCREEN, OFFSCREEN))
        .map_err(|error| error.to_string())?;
    let owner = main.hwnd().map_err(|error| error.to_string())?.0 as isize;
    let target = editor.hwnd().map_err(|error| error.to_string())?.0 as isize;
    windows::notify_configure_check(target)?;
    wait(|| windows::configure_reachable(owner, target))?;
    Ok(())
}

/// Supplies one isolated fixture library under ignored quality output; no user macro directory is read or changed.
fn library() -> Result<LibraryService, String> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(OUTPUT_DIRECTORY)
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    std::fs::write(directory.join(FIXTURE_FILE), FIXTURE).map_err(|error| error.to_string())?;
    Ok(LibraryService(Ok(Arc::new(Repository::new(directory)))))
}

/// Runs a bounded real-Tauri check; a watchdog bounds failures that prevent the event loop from responding.
pub fn run() {
    windows::enable_physical_dpi();
    let finished = Arc::new(AtomicBool::new(false));
    let watched = finished.clone();
    std::thread::spawn(move || {
        std::thread::sleep(TIMEOUT);
        if !watched.load(Ordering::SeqCst) {
            eprintln!("{WATCHDOG_FAILED}");
            std::process::exit(FAILURE);
        }
    });
    let started = Arc::new(AtomicBool::new(false));
    let app = desktop_builder_with_library(false, library().expect(LIBRARY_FAILED))
        .on_page_load(move |webview, payload| {
            eprintln!(
                "Configure native check: page {} {:?}",
                webview.label(),
                payload.event()
            );
            if webview.label() != MAIN
                || payload.event() != PageLoadEvent::Finished
                || started.swap(true, Ordering::SeqCst)
            {
                return;
            }
            let app = webview.app_handle().clone();
            std::thread::spawn(move || {
                let main = app.get_webview_window(MAIN).expect(MAIN_FAILED);
                eprintln!("Configure native check: waiting for main library DOM");
                let result =
                    wait(|| evaluate(&main, MAIN_READY)).and_then(|_| check_displays(&app, &main));
                match result {
                    Ok(()) => app.exit(SUCCESS),
                    Err(error) => {
                        eprintln!("FAIL Configure native check: {error}");
                        app.exit(FAILURE);
                    }
                }
            });
        })
        .build(tauri::generate_context!())
        .expect(BUILD_FAILED);
    let result = app.run_return(|_, _| {});
    finished.store(true, Ordering::SeqCst);
    assert_eq!(result, SUCCESS, "{CHECK_FAILED}");
}
