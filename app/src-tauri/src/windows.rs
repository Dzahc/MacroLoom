use crate::session::Session;
use crate::session_control::{ControlRequest as ControlJob, QueuedControl};
use crate::shortcuts::{
    Availability, Control, KeySequences, RECORD_ID, RECORD_KEY, STOP_ID, STOP_KEY,
};
use crate::window_view::{ContentSize, Position, WindowAdapter};
use crate::{EventKind, Held, MouseButton};
use std::cell::Cell;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::{mpsc, LazyLock, Mutex};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, EnumDisplayMonitors, GetMonitorInfoW, MonitorFromRect, HDC, HMONITOR,
    MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForMonitor, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, MapVirtualKeyExW, RegisterHotKey, SendInput,
    UnregisterHotKey, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC_EX, MOD_NOREPEAT,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL,
    MOUSEINPUT,
};
use windows_sys::Win32::UI::Shell::{
    DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetSystemMetrics, GetWindowLongW, GetWindowPlacement, GetWindowRect, GetWindowThreadProcessId,
    KillTimer, PeekMessageW, PostThreadMessageW, SetTimer, SetWindowLongW, SetWindowPlacement,
    SetWindowPos, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, GWL_EXSTYLE, GWL_STYLE,
    HTCAPTION, HWND_NOTOPMOST, HWND_TOPMOST, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_INJECTED,
    MA_NOACTIVATE, MSG, MSLLHOOKSTRUCT, PBT_APMSUSPEND, PM_NOREMOVE, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
    USER_TIMER_MINIMUM, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWPLACEMENT, WM_APP, WM_CANCELMODE,
    WM_DISPLAYCHANGE, WM_HOTKEY, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_NCLBUTTONDBLCLK,
    WM_NCLBUTTONDOWN, WM_POWERBROADCAST, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WM_TIMER, WS_EX_NOACTIVATE, WS_MAXIMIZE, WS_MAXIMIZEBOX, WS_THICKFRAME,
};

static ENGINE: OnceLock<Arc<Session>> = OnceLock::new();
static HOOK_JOBS: OnceLock<mpsc::Sender<ReservationJob>> = OnceLock::new();
static CONTROL_JOBS: OnceLock<mpsc::Sender<QueuedControl>> = OnceLock::new();
static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static RESERVED: LazyLock<Mutex<Availability>> =
    LazyLock::new(|| Mutex::new(Availability::default()));
static SEQUENCES: LazyLock<Mutex<KeySequences>> =
    LazyLock::new(|| Mutex::new(KeySequences::default()));
static OWN_KEY_SEQUENCES: LazyLock<Mutex<OwnKeySequences>> =
    LazyLock::new(|| Mutex::new(OwnKeySequences::default()));
static OWN_MOUSE_SEQUENCES: LazyLock<Mutex<OwnMouseSequences>> =
    LazyLock::new(|| Mutex::new(OwnMouseSequences::default()));
const RESERVATION_MESSAGE: u32 = WM_APP + 1;
const HOOK_TIMEOUT: Duration = Duration::from_secs(5);
const HOOK_FAILED: &str =
    "Native input capture is unavailable; Retry shortcuts or restart MacroLoom.";
const UNREGISTER_FAILED: &str = "Could not release a global shortcut reservation.";
const POWER_SUBCLASS: usize = 14;
const POWER_HOOK_FAILED: &str = "Could not observe computer sleep notifications.";
const COMPACT_COLLISION: &str =
    "Playback target overlaps MacroLoom; move the compact window and retry.";
const NORMALIZED_RANGE: i64 = 65536;
const NORMALIZED_PIXEL_CENTER: i64 = NORMALIZED_RANGE / 2;
const INVALID_PHYSICAL_PIXEL: &str = "Playback target is outside the current virtual desktop; restore the recorded layout and retry.";
const CONFLICT_HELPER_FAILED: &str = "The requested key is already reserved. Close MacroLoom/the conflicting application and try again.";
const CONFLICT_HELPER_DISABLED: &str = "Shortcut conflict helper is development-only.";
const CONFLICT_RECORD: &str = "record";
const CONFLICT_STOP: &str = "stop";
const CONFLICT_USAGE: &str = "Use: shortcut-conflict-check record|stop";
const EXTRA_INFO: usize = 0x4D4C_3038;
const BASE_DPI: u32 = 96;
const FULL_MIN_WIDTH: f64 = 360.0;
const FULL_MIN_HEIGHT: f64 = 230.0;
const WINDOW_BOUNDS_ERROR: &str = "Could not read window bounds";
const WINDOW_PLACEMENT_ERROR: &str = "Could not read full window placement";
const WINDOW_STYLE_ERROR: &str = "Could not change window frame";
const WINDOW_MOVE_ERROR: &str = "Could not change window bounds or topmost state";
const WINDOW_WORK_AREA_ERROR: &str = "Could not read display work area";
const WINDOW_CONTENT_ERROR: &str = "Compact content does not fit the display work area";
const WINDOW_RESTORE_ERROR: &str = "Could not restore full window placement";
const COMPACT_ROLLBACK_FAILED: &str = "compact rollback failed";
const KEYBOARD_LAYOUT_MASK: usize = 0xffff_ffff;
const FIRST_CAPTURE_KEY: u16 = 8;
const LAST_CAPTURE_KEY: u16 = 254;
const FIRST_PLAYBACK_KEY: u16 = 1;
const GENERIC_MODIFIERS: [u16; 3] = [0x10, 0x11, 0x12];
const SNAPSHOT_MODIFIERS: [u16; 8] = [0xa2, 0xa3, 0xa0, 0xa1, 0xa4, 0xa5, 0x5b, 0x5c];
const LEFT_MOUSE_KEY: u16 = 1;
const RIGHT_MOUSE_KEY: u16 = 2;
const SNAPSHOT_BUTTONS: [(u16, MouseButton); 2] = [
    (LEFT_MOUSE_KEY, MouseButton::Left),
    (RIGHT_MOUSE_KEY, MouseButton::Right),
];
const EXTENDED_SCAN_MASK: u32 = 0xff00;
const LOW_SCAN_MASK: u32 = 0xff;
const WHEEL_DATA_SHIFT: u32 = 16;
const SNAPSHOT_CONTROL_KEYS: [u16; 4] = [RECORD_KEY, STOP_KEY, 0x0d, 0x20];
const RESERVED_CAPTURE_KEYS: [u16; 2] = [RECORD_KEY, STOP_KEY];
const ENVIRONMENT_UNAVAILABLE: &str =
    "Could not read the recording keyboard layout or display configuration";
const COMPACT_DRAG_SUBCLASS: usize = 18;
const COMPACT_DRAG_TIMER: usize = 0x4D4C_0018;
const COMPACT_DRAG_ERROR: &str = "Could not move the compact window without activation";
const COMPACT_DRAG_TIMER_ERROR: &str = "Could not release the compact dragging timer";
const MOUSE_WORD_SHIFT: u32 = 16;

#[derive(Clone, Copy)]
/// Physical cursor/frame origins copied at a title-bar press; they never refer to native borrowed storage.
struct DragAnchor {
    cursor: POINT,
    window: POINT,
}

#[derive(Default)]
/// Event-thread-only compact interaction, owned by its native subclass until WM_NCDESTROY.
/// Cells permit synchronous reentrant window messages without an outstanding mutable borrow.
struct CompactDrag {
    enabled: Cell<bool>,
    anchor: Cell<Option<DragAnchor>>,
}

/// Clones the subclass's owned state on the owning thread so reentrant destruction cannot invalidate a callback's copy.
fn compact_drag_state(hwnd: HWND) -> Option<Rc<CompactDrag>> {
    let mut data = 0;
    // SAFETY: called only on this HWND's event thread; successful lookup returns the Rc installed by compact_drag_mode.
    if unsafe {
        GetWindowSubclass(
            hwnd,
            Some(compact_message),
            COMPACT_DRAG_SUBCLASS,
            &mut data,
        )
    } == 0
    {
        return None;
    }
    // SAFETY: the subclass owns one strong reference until WM_NCDESTROY; no native call occurs between increment and reconstruction.
    unsafe {
        Rc::increment_strong_count(data as *const CompactDrag);
        Some(Rc::from_raw(data as *const CompactDrag))
    }
}

/// Installs/toggles nonactivating title-bar handling on the window thread; full mode ends dragging and preserves ordinary activation.
fn compact_drag_mode(hwnd: HWND, enabled: bool) -> Result<(), String> {
    let state = match compact_drag_state(hwnd) {
        Some(state) => state,
        None if !enabled => return Ok(()),
        None => {
            let state = Rc::new(CompactDrag::default());
            let owned = Rc::into_raw(state.clone());
            // SAFETY: live same-thread HWND; the static callback owns this strong reference through native destruction.
            if unsafe {
                SetWindowSubclass(
                    hwnd,
                    Some(compact_message),
                    COMPACT_DRAG_SUBCLASS,
                    owned as usize,
                )
            } == 0
            {
                // SAFETY: installation failed, so no native callback owns the otherwise leaked strong reference.
                drop(unsafe { Rc::from_raw(owned) });
                return Err(COMPACT_DRAG_ERROR.into());
            }
            state
        }
    };
    state.enabled.set(enabled);
    if !enabled {
        finish_compact_drag(hwnd, &state);
    }
    Ok(())
}

/// Filters only compact title-bar dragging, forwarding all controls and full-window messages through Tauri's existing chain.
unsafe extern "system" fn compact_message(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: this installed subclass owns the pointer; clone before any operation capable of reentrant native destruction.
    let state = unsafe {
        Rc::increment_strong_count(data as *const CompactDrag);
        Rc::from_raw(data as *const CompactDrag)
    };
    if message == WM_NCDESTROY {
        finish_compact_drag(hwnd, &state);
        // SAFETY: removes only this same-thread subclass, leaving this callback's cloned Rc alive until return.
        if unsafe { RemoveWindowSubclass(hwnd, Some(compact_message), COMPACT_DRAG_SUBCLASS) } != 0
        {
            // SAFETY: successful removal relinquishes the subclass's one owned strong reference exactly once.
            drop(unsafe { Rc::from_raw(data as *const CompactDrag) });
        } else {
            eprintln!("{COMPACT_DRAG_ERROR}");
        }
    } else if state.enabled.get() {
        if let Some(result) = compact_drag_message(hwnd, &state, message, wparam, lparam) {
            return result;
        }
    }
    // SAFETY: original by-value arguments are forwarded through the remaining live native subclass chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Handles short drag messages independently of recording/playback state; dragging never cancels or reschedules input.
fn compact_drag_message(
    hwnd: HWND,
    state: &CompactDrag,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEACTIVATE if lparam as u16 as u32 == HTCAPTION => Some(MA_NOACTIVATE as isize),
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if wparam == HTCAPTION as usize => {
            report_compact_drag(
                hwnd,
                state,
                begin_compact_drag(hwnd, state, screen_message_point(lparam)),
            );
            Some(0)
        }
        WM_MOUSEMOVE if state.anchor.get().is_some() => {
            report_compact_drag(hwnd, state, move_compact_from_client(hwnd, state, lparam));
            Some(0)
        }
        WM_TIMER if wparam == COMPACT_DRAG_TIMER => {
            report_compact_drag(hwnd, state, poll_compact_drag(hwnd, state));
            Some(0)
        }
        WM_LBUTTONUP | WM_CANCELMODE => {
            finish_compact_drag(hwnd, state);
            None
        }
        _ => None,
    }
}

/// Sign-extends native message words; unlike playback coordinates this is the Win32 mouse-message representation.
fn screen_message_point(value: LPARAM) -> POINT {
    POINT {
        x: i32::from(value as i16),
        y: i32::from((value >> MOUSE_WORD_SHIFT) as i16),
    }
}

/// Starts a bounded native timer instead of a foreground-activating modal move loop or foreground-only mouse capture.
fn begin_compact_drag(hwnd: HWND, state: &CompactDrag, cursor: POINT) -> Result<(), String> {
    let mut bounds = RECT::default();
    // SAFETY: live owned frame, initialized local output and a window-thread timer with no borrowed callback data.
    if unsafe { GetWindowRect(hwnd, &mut bounds) } == 0 {
        return Err(WINDOW_BOUNDS_ERROR.into());
    }
    // SAFETY: the timer is scoped to this live window/thread and handled by the static subclass, which cleans it up.
    if unsafe { SetTimer(hwnd, COMPACT_DRAG_TIMER, USER_TIMER_MINIMUM, None) } == 0 {
        return Err(COMPACT_DRAG_ERROR.into());
    }
    state.anchor.set(Some(DragAnchor {
        cursor,
        window: POINT {
            x: bounds.left,
            y: bounds.top,
        },
    }));
    Ok(())
}

/// Converts a delivered client mouse point before moving; the timer covers motion/release outside the inactive window.
fn move_compact_from_client(hwnd: HWND, state: &CompactDrag, value: LPARAM) -> Result<(), String> {
    let mut point = screen_message_point(value);
    // SAFETY: synchronous conversion writes an initialized local POINT for this live frame.
    if unsafe { ClientToScreen(hwnd, &mut point) } == 0 {
        return Err(COMPACT_DRAG_ERROR.into());
    }
    move_compact_drag(hwnd, state, point)
}

/// Polls only during an owned title-bar gesture, allowing background movement without consuming external keyboard input.
fn poll_compact_drag(hwnd: HWND, state: &CompactDrag) -> Result<(), String> {
    if !key_held(LEFT_MOUSE_KEY) {
        finish_compact_drag(hwnd, state);
        return Ok(());
    }
    let mut point = POINT::default();
    // SAFETY: initialized local storage is writable for the synchronous physical cursor query.
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err(COMPACT_DRAG_ERROR.into());
    }
    move_compact_drag(hwnd, state, point)
}

/// Moves the shared-cursor drag without activation, resizing, z-order changes, focus restoration or playback control.
fn move_compact_drag(hwnd: HWND, state: &CompactDrag, point: POINT) -> Result<(), String> {
    let Some(anchor) = state.anchor.get() else {
        return Ok(());
    };
    let x =
        i32::try_from(i64::from(anchor.window.x) + i64::from(point.x) - i64::from(anchor.cursor.x))
            .map_err(|_| COMPACT_DRAG_ERROR)?;
    let y =
        i32::try_from(i64::from(anchor.window.y) + i64::from(point.y) - i64::from(anchor.cursor.y))
            .map_err(|_| COMPACT_DRAG_ERROR)?;
    // SAFETY: live same-thread window and checked signed physical coordinates; flags expressly forbid activation and resizing.
    if unsafe {
        SetWindowPos(
            hwnd,
            null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER,
        )
    } == 0
    {
        return Err(WINDOW_MOVE_ERROR.into());
    }
    Ok(())
}

/// Stops timer work on release, cancellation, restoration or destruction; repeated termination is harmless.
fn finish_compact_drag(hwnd: HWND, state: &CompactDrag) {
    if state.anchor.take().is_some() {
        // SAFETY: this identifier belongs only to the live frame's drag handler; no timer callback holds borrowed state.
        if unsafe { KillTimer(hwnd, COMPACT_DRAG_TIMER) } == 0 {
            eprintln!("{COMPACT_DRAG_TIMER_ERROR}");
        }
    }
}

/// Terminates only a failed window gesture; playback and recording continue under their existing lifecycle owners.
fn report_compact_drag(hwnd: HWND, state: &CompactDrag, result: Result<(), String>) {
    if let Err(error) = result {
        finish_compact_drag(hwnd, state);
        eprintln!("{COMPACT_DRAG_ERROR}: {error}");
    }
}

/// Atomically moves a flushed staging file without replacement, even when another process wins the filename race.
pub fn commit_new_recording(source: &Path, destination: &Path) -> std::io::Result<()> {
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: both buffers are live nul-terminated paths throughout this synchronous call. No replacement flag is set.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Reads the foreground application's layout and every physical monitor; failures prevent misleading recording metadata.
pub fn recording_environment() -> Result<crate::macro_format::RecordingEnvironment, String> {
    // SAFETY: getters accept OS-owned HWND and a null optional process output; no handles are retained.
    let thread = unsafe { GetWindowThreadProcessId(GetForegroundWindow(), null_mut()) };
    let layout = unsafe { GetKeyboardLayout(thread) };
    if layout.is_null() {
        return Err(ENVIRONMENT_UNAVAILABLE.into());
    }
    let mut displays: Vec<crate::macro_format::Display> = Vec::new();
    // SAFETY: callback borrows this Vec exclusively for synchronous enumeration and never retains its pointer.
    if unsafe {
        EnumDisplayMonitors(
            null_mut(),
            std::ptr::null(),
            Some(recording_monitor),
            &mut displays as *mut _ as LPARAM,
        )
    } == 0
    {
        return Err(ENVIRONMENT_UNAVAILABLE.into());
    }
    displays.sort_by_key(|display| (display.x, display.y, display.width, display.height));
    if displays.is_empty() {
        return Err(ENVIRONMENT_UNAVAILABLE.into());
    }
    Ok(crate::recording::environment(
        format!("{:08X}", layout as usize & KEYBOARD_LAYOUT_MASK),
        displays,
    ))
}

/// Copies one enumerated physical monitor and its effective scale into the caller-owned synchronous output.
unsafe extern "system" fn recording_monitor(
    monitor: HMONITOR,
    _: HDC,
    bounds: *mut RECT,
    output: LPARAM,
) -> i32 {
    let mut dpi_x = BASE_DPI;
    let mut dpi_y = BASE_DPI;
    // SAFETY: monitor and bounds are supplied by EnumDisplayMonitors; DPI outputs are initialized writable locals.
    if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) } < 0 {
        return 0;
    }
    // SAFETY: enumeration caller passes a uniquely borrowed live Vec; bounds is valid only for this callback.
    let displays = unsafe { &mut *(output as *mut Vec<crate::macro_format::Display>) };
    let rect = unsafe { &*bounds };
    displays.push(crate::macro_format::Display {
        x: rect.left,
        y: rect.top,
        width: (rect.right - rect.left) as u32,
        height: (rect.bottom - rect.top) as u32,
        scale_factor: f64::from(dpi_x) / f64::from(BASE_DPI),
        extra: Default::default(),
    });
    1
}

/// Snapshots held inputs without injecting anything: modifiers first, then keys, then supported buttons.
pub fn initial_recording_inputs() -> Vec<EventKind> {
    let own_focus = own_window_focused();
    let (x, y) = cursor_position();
    let Ok(mut keys) = OWN_KEY_SEQUENCES.lock() else {
        return Vec::new();
    };
    let mut inputs = keys.initial_snapshot(own_focus, POINT { x, y }, false, key_held);
    drop(keys);
    if let Ok(mut mouse) = OWN_MOUSE_SEQUENCES.lock() {
        inputs.extend(mouse.initial_snapshot(POINT { x, y }, own_point(POINT { x, y }), key_held));
    }
    inputs
}

#[derive(Default)]
/// Tracks each held button's control/external origin independently until its observed release.
struct OwnMouseSequences {
    origins: [Option<bool>; 2],
}
impl OwnMouseSequences {
    /// Converts a native event and excludes control-origin buttons; external drags retain movement and wheel over controls.
    fn capture(&mut self, message: u32, input: &MSLLHOOKSTRUCT, own: bool) -> Option<EventKind> {
        let kind = mouse_kind(message, input)?;
        let excluded = match &kind {
            EventKind::Mouse { button, down, .. } => self.button(*button, *down, own),
            EventKind::Move { .. } | EventKind::Wheel { .. } => {
                own && !self.origins.contains(&Some(false))
            }
            EventKind::Key { .. } => true,
        };
        (!excluded).then_some(kind)
    }

    /// Returns the gesture's exclusion status, preserving the press origin across window crossings and other button changes.
    fn button(&mut self, button: MouseButton, down: bool, own: bool) -> bool {
        let origin = self.origins[button.index()].unwrap_or(own);
        self.origins[button.index()] = if down { Some(origin) } else { None };
        origin
    }

    /// Reconciles physical holds at recording start and returns only external buttons, in deterministic order at the current position.
    fn initial_snapshot(
        &mut self,
        position: POINT,
        own: bool,
        held: impl Fn(u16) -> bool,
    ) -> Vec<EventKind> {
        let mut inputs = Vec::new();
        for (key, button) in SNAPSHOT_BUTTONS {
            let origin = if held(key) {
                Some(self.origins[button.index()].unwrap_or(own))
            } else {
                None
            };
            self.origins[button.index()] = origin;
            if origin == Some(false) {
                inputs.push(EventKind::Mouse {
                    button,
                    down: true,
                    x: position.x,
                    y: position.y,
                });
            }
        }
        inputs
    }
}
#[derive(Default)]
/// Tracks control-origin exclusions and recording-owned initial presses through their actual releases.
struct OwnKeySequences {
    ignored: std::collections::HashSet<u16>,
    recorded: std::collections::HashSet<u16>,
}
impl OwnKeySequences {
    /// Transfers snapshotted key gestures to recording ownership, leaving excluded activation gestures untouched.
    fn initial_snapshot(
        &mut self,
        own_focus: bool,
        position: POINT,
        capture_buttons: bool,
        held: impl Fn(u16) -> bool,
    ) -> Vec<EventKind> {
        let inputs = ordered_initial_inputs(
            own_focus,
            position,
            capture_buttons,
            // Startup may outlive focus: a held activation gesture retains its control origin.
            |key| {
                held(key) && !(SNAPSHOT_CONTROL_KEYS.contains(&key) && self.ignored.contains(&key))
            },
        );
        self.recorded.clear();
        for input in &inputs {
            if let EventKind::Key { vk, down: true, .. } = input {
                self.ignored.remove(vk);
                self.recorded.insert(*vk);
            }
        }
        inputs
    }
    /// Returns whether a complete key gesture belongs to MacroLoom controls, retaining its origin across focus changes.
    fn key(&mut self, key: u16, down: bool, own: bool) -> bool {
        if self.recorded.contains(&key) {
            if !down {
                self.recorded.remove(&key);
            }
            return false;
        }
        if down && own {
            self.ignored.insert(key);
        }
        let excluded = own || self.ignored.contains(&key);
        if !down {
            self.ignored.remove(&key);
        }
        excluded
    }
}
/// Applies the boundary ordering/exclusion policy to a held-state reader; the reader never injects or mutates input.
fn ordered_initial_inputs(
    own_focus: bool,
    position: POINT,
    capture_buttons: bool,
    held: impl Fn(u16) -> bool,
) -> Vec<EventKind> {
    let mut inputs = Vec::new();
    for key in SNAPSHOT_MODIFIERS {
        if held(key) {
            append_initial_key(&mut inputs, key);
        }
    }
    for key in FIRST_CAPTURE_KEY..=LAST_CAPTURE_KEY {
        if initial_key_excluded(key, own_focus) || !held(key) {
            continue;
        }
        append_initial_key(&mut inputs, key);
    }
    if capture_buttons {
        for (key, button) in SNAPSHOT_BUTTONS {
            if held(key) {
                inputs.push(EventKind::Mouse {
                    button,
                    down: true,
                    x: position.x,
                    y: position.y,
                });
            }
        }
    }
    inputs
}
/// Excludes duplicate generic modifiers and complete Record/Stop or focused keyboard activation controls.
fn initial_key_excluded(key: u16, own_focus: bool) -> bool {
    if SNAPSHOT_MODIFIERS.contains(&key) || GENERIC_MODIFIERS.contains(&key) {
        return true;
    }
    if RESERVED_CAPTURE_KEYS.contains(&key) {
        return true;
    }
    own_focus && SNAPSHOT_CONTROL_KEYS.contains(&key)
}
/// Reports the actual down state without changing the user's keyboard or mouse state.
fn key_held(key: u16) -> bool {
    // SAFETY: this value-only getter accepts a supported virtual key and retains no pointer.
    unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
}

/// Refuses playback while any physical key or button remains down, including MacroLoom activation gestures.
/// This is a startup snapshot, not an input lock; later manual input remains enabled and can interfere.
pub fn playback_inputs_released() -> Result<(), String> {
    if (FIRST_PLAYBACK_KEY..=LAST_CAPTURE_KEY).any(key_held) {
        return Err(crate::playback::INPUT_HELD.into());
    }
    Ok(())
}
/// Adds one held native key with scan-code metadata from the foreground application's keyboard layout.
fn append_initial_key(inputs: &mut Vec<EventKind>, vk: u16) {
    // SAFETY: getters use the current foreground thread's OS-owned layout; conversion has no pointer parameters.
    let thread = unsafe { GetWindowThreadProcessId(GetForegroundWindow(), null_mut()) };
    let layout = unsafe { GetKeyboardLayout(thread) };
    let scan = unsafe { MapVirtualKeyExW(u32::from(vk), MAPVK_VK_TO_VSC_EX, layout) };
    inputs.push(EventKind::Key {
        vk,
        scan: (scan & LOW_SCAN_MASK) as u16,
        extended: scan & EXTENDED_SCAN_MASK != 0,
        down: true,
    });
}

#[cfg(feature = "native-checks")]
#[path = "session_window_tests.rs"]
mod session_window_tests;

#[cfg(feature = "native-checks")]
/// Runs the isolated session check using genuine OS reservations and hook-dispatch fixtures, with no desktop injection.
pub(crate) fn check_session_controls() {
    session_window_tests::run();
}

/// Enables per-monitor physical-coordinate handling before creating any application windows.
pub fn enable_physical_dpi() {
    // SAFETY: this process-wide Win32 call takes a documented constant, with no borrowed pointers.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[derive(Clone, Copy)]
/// Full native placement, physical visible bounds, and original frame/activation styles retained through compact movement.
pub struct FullWindowPlacement {
    bounds: RECT,
    placement: WINDOWPLACEMENT,
    style: i32,
    extended_style: i32,
}

/// Windows adapter for the owned live Tauri main window; all mutations run on its native event thread.
pub struct DesktopWindow {
    constraints: Box<dyn Fn(bool) -> Result<(), String> + Send + Sync>,
    hwnd: isize,
}

impl DesktopWindow {
    /// Keeps Tauri ownership alongside its HWND; fails before storing an invalid handle.
    pub fn new(window: tauri::WebviewWindow) -> Result<Self, String> {
        let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as isize;
        Ok(Self {
            hwnd,
            constraints: Box::new(move |compact| {
                // Captured Tauri ownership keeps the HWND alive and applies only toolkit size constraints.
                if compact {
                    window.set_min_size::<tauri::LogicalSize<f64>>(None)
                } else {
                    window.set_min_size(Some(tauri::LogicalSize::new(
                        FULL_MIN_WIDTH,
                        FULL_MIN_HEIGHT,
                    )))
                }
                .map_err(|error| error.to_string())
            }),
        })
    }

    #[cfg(test)]
    /// Binds a test-owned native frame without Tauri size constraints; the test destroys it only after the adapter drops.
    fn unconstrained(hwnd: isize) -> Self {
        Self {
            hwnd,
            constraints: Box::new(|_| Ok(())),
        }
    }

    /// Changes the requested native frame/activation style with explicit error handling; the documented index accepts values, not pointers.
    fn style(&self, index: i32, style: i32) -> Result<(), String> {
        // SAFETY: HWND remains owned by the live main window; the documented style indices accept values, not borrowed pointers.
        unsafe {
            SetLastError(0);
        }
        // SAFETY: the same live HWND and documented style index are valid throughout this event-thread operation.
        let previous = unsafe { SetWindowLongW(self.hwnd as HWND, index, style) };
        // SAFETY: thread-local last-error inspection has no pointer or lifetime requirements.
        if previous == 0 && unsafe { GetLastError() } != 0 {
            return Err(WINDOW_STYLE_ERROR.into());
        }
        Ok(())
    }

    /// Moves/sizes the owned frame and changes topmost state without activating or changing owner ordering.
    fn place(&self, bounds: RECT, compact: bool) -> Result<(), String> {
        let order = if compact {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        // SAFETY: live HWND and by-value signed physical coordinates; no borrowed pointers survive this call.
        let moved = unsafe {
            SetWindowPos(
                self.hwnd as HWND,
                order,
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
            )
        };
        if moved == 0 {
            return Err(WINDOW_MOVE_ERROR.into());
        }
        Ok(())
    }

    /// Returns actual physical frame bounds, propagating failure instead of assuming hidden-library geometry.
    fn bounds(&self) -> Result<RECT, String> {
        // SAFETY: zero is a valid initialized RECT representation, and the live HWND writes only this local value.
        let mut bounds: RECT = unsafe { std::mem::zeroed() };
        // SAFETY: bounds points to initialized writable storage for this synchronous OS call.
        if unsafe { GetWindowRect(self.hwnd as HWND, &mut bounds) } == 0 {
            return Err(WINDOW_BOUNDS_ERROR.into());
        }
        Ok(bounds)
    }

    /// Converts measured logical content to a physical decorated frame using the window's current monitor DPI.
    fn content_bounds(
        &self,
        position: Position,
        size: ContentSize,
        style: i32,
    ) -> Result<RECT, String> {
        // SAFETY: live HWND; getters return values and retain no borrowed pointers.
        let dpi = unsafe { GetDpiForWindow(self.hwnd as HWND) }.max(BASE_DPI);
        let scale = f64::from(dpi) / f64::from(BASE_DPI);
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: (size.width * scale).ceil() as i32,
            bottom: (size.height * scale).ceil() as i32,
        };
        // SAFETY: style getter uses the owned live HWND and a documented by-value index.
        let extended = unsafe { GetWindowLongW(self.hwnd as HWND, GWL_EXSTYLE) } as u32;
        // SAFETY: bounds is initialized writable local storage; the style/DPI values belong to this same window.
        if unsafe { AdjustWindowRectExForDpi(&mut bounds, style as u32, 0, extended, dpi) } == 0 {
            return Err(WINDOW_STYLE_ERROR.into());
        }
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let requested = RECT {
            left: position.x,
            top: position.y,
            right: position.x + width,
            bottom: position.y + height,
        };
        clamp_window_bounds(requested)
    }

    /// Applies an owned placement, constraints, frame, and stacking mode without activation; callers roll back partial failure.
    fn apply_placement(&self, saved: &FullWindowPlacement, compact: bool) -> Result<(), String> {
        compact_drag_mode(self.hwnd as HWND, compact)?;
        let mut placement = saved.placement;
        placement.showCmd = SW_SHOWNOACTIVATE as u32;
        // SAFETY: live HWND; captured initialized placement is local and its show command forbids activation.
        if unsafe { SetWindowPlacement(self.hwnd as HWND, &placement) } == 0 {
            return Err(WINDOW_RESTORE_ERROR.into());
        }
        (self.constraints)(compact)?;
        self.style(GWL_STYLE, saved.style)?;
        self.style(GWL_EXSTYLE, saved.extended_style)?;
        self.place(saved.bounds, compact)
    }
}

impl WindowAdapter for DesktopWindow {
    type Saved = FullWindowPlacement;

    /// Saves native restore geometry and full visible bounds before any size/constraint/style mutation.
    fn save(&self) -> Result<Self::Saved, String> {
        let bounds = self.bounds()?;
        // SAFETY: WINDOWPLACEMENT permits zero initialization when its length is filled before calling Win32.
        let mut placement: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
        placement.length = size_of::<WINDOWPLACEMENT>() as u32;
        // SAFETY: live HWND; placement points to correctly sized initialized writable local storage.
        if unsafe { GetWindowPlacement(self.hwnd as HWND, &mut placement) } == 0 {
            return Err(WINDOW_PLACEMENT_ERROR.into());
        }
        // SAFETY: documented value getter on the owned main window.
        let style = unsafe { GetWindowLongW(self.hwnd as HWND, GWL_STYLE) };
        // SAFETY: extended style is another by-value getter on the same live window.
        let extended_style = unsafe { GetWindowLongW(self.hwnd as HWND, GWL_EXSTYLE) };
        Ok(FullWindowPlacement {
            bounds,
            placement,
            style,
            extended_style,
        })
    }

    /// Returns the full view's signed physical upper-left position as the initial compact anchor.
    fn saved_position(&self, saved: &Self::Saved) -> Position {
        Position {
            x: saved.bounds.left,
            y: saved.bounds.top,
        }
    }

    /// Reads actual moved compact coordinates rather than reusing the full-window anchor.
    fn position(&self) -> Result<Position, String> {
        let bounds = self.bounds()?;
        Ok(Position {
            x: bounds.left,
            y: bounds.top,
        })
    }

    /// Disables native sizing/maximizing, obtains destination DPI, and fits the entire compact frame on its work area.
    fn enter(&mut self, position: Position, size: ContentSize) -> Result<(), String> {
        (self.constraints)(true)?;
        // SAFETY: live main HWND and by-value documented style index; no pointer is retained.
        let style = unsafe { GetWindowLongW(self.hwnd as HWND, GWL_STYLE) } as u32
            & !(WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MAXIMIZE);
        self.style(GWL_STYLE, style as i32)?;
        // SAFETY: getter reads this owned window's activation policy before preserving its other extended style bits.
        let extended = unsafe { GetWindowLongW(self.hwnd as HWND, GWL_EXSTYLE) } as u32;
        self.style(GWL_EXSTYLE, (extended | WS_EX_NOACTIVATE) as i32)?;
        let current = self.bounds()?;
        self.place(
            RECT {
                left: position.x,
                top: position.y,
                right: position.x + current.right - current.left,
                bottom: position.y + current.bottom - current.top,
            },
            true,
        )?;
        self.place(self.content_bounds(position, size, style as i32)?, true)?;
        compact_drag_mode(self.hwnd as HWND, true)
    }

    /// Restores full placement without activation; partial failure rolls back the prior compact frame and retains restoration data.
    fn restore(&mut self, saved: &Self::Saved) -> Result<(), String> {
        let previous = self.save()?;
        if let Err(error) = self.apply_placement(saved, false) {
            return match self.apply_placement(&previous, true) {
                Ok(()) => Err(error),
                Err(rollback) => Err(format!("{error}; {COMPACT_ROLLBACK_FAILED}: {rollback}")),
            };
        }
        Ok(())
    }

    /// Applies DPI/work-area corrections after compact movement only when the actual frame needs changing.
    fn fit(&mut self, size: ContentSize) -> Result<(), String> {
        let current = self.bounds()?;
        let position = Position {
            x: current.left,
            y: current.top,
        };
        // SAFETY: live owned HWND; style getter returns a value and retains no pointer.
        let style = unsafe { GetWindowLongW(self.hwnd as HWND, GWL_STYLE) };
        let fitted = self.content_bounds(position, size, style)?;
        if (current.left, current.top, current.right, current.bottom)
            == (fitted.left, fitted.top, fitted.right, fitted.bottom)
        {
            return Ok(());
        }
        self.place(fitted, true)
    }
}

/// Clamps a decorated physical frame to its nearest monitor work area, failing if its content cannot fit.
fn clamp_window_bounds(bounds: RECT) -> Result<RECT, String> {
    // SAFETY: bounds is initialized and borrowed only synchronously; MONITORINFO allows zero initialization with cbSize set.
    let monitor = unsafe { MonitorFromRect(&bounds, MONITOR_DEFAULTTONEAREST) };
    // SAFETY: all-zero MONITORINFO is valid local output storage once cbSize is initialized.
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    // SAFETY: monitor is an OS-owned nearest-monitor handle; info is correctly sized writable local storage.
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return Err(WINDOW_WORK_AREA_ERROR.into());
    }
    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    if width > info.rcWork.right - info.rcWork.left || height > info.rcWork.bottom - info.rcWork.top
    {
        return Err(WINDOW_CONTENT_ERROR.into());
    }
    let left = bounds
        .left
        .clamp(info.rcWork.left, info.rcWork.right - width);
    let top = bounds
        .top
        .clamp(info.rcWork.top, info.rcWork.bottom - height);
    Ok(RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    })
}

/// Owns native registration requests on the same message thread that installed the hooks.
struct ReservationJob {
    record: bool,
    stop: bool,
    reply: mpsc::SyncSender<Result<Availability, String>>,
}
/// Actual thread-owned reservations plus the last explicit Stop availability check.
struct HookRuntime {
    record: bool,
    stop: bool,
    stop_available: bool,
    hooks_ready: bool,
}
impl HookRuntime {
    /// Reconciles physical reservations; errors retain actual ownership rather than pretending unregistration succeeded.
    fn apply(&mut self, record: bool, stop: bool) -> Result<Availability, String> {
        if !self.hooks_ready {
            return Err(HOOK_FAILED.into());
        }
        let result = self.apply_keys(record, stop);
        *RESERVED.lock().map_err(|_| HOOK_FAILED)? = Availability {
            record: self.record,
            stop: self.stop,
        };
        result?;
        Ok(Availability {
            record: self.record,
            stop: self.stop_available,
        })
    }
    /// Updates individual physical reservations; the caller publishes actual ownership even after partial failure.
    fn apply_keys(&mut self, record: bool, stop: bool) -> Result<(), String> {
        self.record = update_registration(self.record, RECORD_ID, RECORD_KEY, record)?;
        self.stop = update_registration(self.stop, STOP_ID, STOP_KEY, stop)?;
        if stop {
            self.stop_available = self.stop;
        }
        Ok(())
    }
}
/// Registers/unregisters one OS key on the hook thread, retaining current ownership on failure.
fn update_registration(current: bool, id: i32, key: u16, wanted: bool) -> Result<bool, String> {
    if current == wanted {
        return Ok(current);
    }
    if wanted {
        // SAFETY: thread-owned hotkey identifiers and by-value virtual key/modifier constants; no pointers are retained.
        return Ok(unsafe { RegisterHotKey(null_mut(), id, MOD_NOREPEAT, u32::from(key)) } != 0);
    }
    // SAFETY: this thread owns this successfully registered identifier and uses a null HWND for thread registration.
    if unsafe { UnregisterHotKey(null_mut(), id) } == 0 {
        return Err(UNREGISTER_FAILED.into());
    }
    Ok(false)
}
/// Updates native reservations and waits for acknowledgement only from a worker, never a hook/UI callback.
pub fn configure_shortcuts(record: bool, stop: bool) -> Result<Availability, String> {
    if !hooks_running() && !record && !stop {
        return Ok(Availability::default());
    }
    let jobs = HOOK_JOBS.get().ok_or(HOOK_FAILED)?;
    let (reply, receive) = mpsc::sync_channel(1);
    jobs.send(ReservationJob {
        record,
        stop,
        reply,
    })
    .map_err(|_| HOOK_FAILED)?;
    wake_hook(RESERVATION_MESSAGE)?;
    receive
        .recv_timeout(HOOK_TIMEOUT)
        .map_err(|_| HOOK_FAILED)?
}
/// Posts a by-value thread message to the live hook thread; no borrowed data crosses threads.
fn wake_hook(message: u32) -> Result<(), String> {
    let thread = HOOK_THREAD.load(Ordering::Acquire);
    // SAFETY: the thread created its message queue before publishing its ID; the message contains no borrowed pointers.
    if unsafe { PostThreadMessageW(thread, message, 0, 0) } == 0 {
        return Err(HOOK_FAILED.into());
    }
    Ok(())
}
/// Installs capture callbacks and starts a backend control worker; failures remain distinct from shortcut conflicts.
pub fn start_hooks(session: Arc<Session>, hwnd: isize) -> Result<(), String> {
    ENGINE.set(session.clone()).map_err(|_| HOOK_FAILED)?;
    MAIN_HWND.store(hwnd, Ordering::Release);
    let (jobs, receive_jobs) = mpsc::channel();
    HOOK_JOBS.set(jobs).map_err(|_| HOOK_FAILED)?;
    let (controls, receive_controls) = mpsc::channel();
    CONTROL_JOBS.set(controls).map_err(|_| HOOK_FAILED)?;
    let (ready, startup) = mpsc::sync_channel(1);
    std::thread::spawn(move || hook_loop(receive_jobs, ready));
    startup
        .recv_timeout(HOOK_TIMEOUT)
        .map_err(|_| HOOK_FAILED)??;
    std::thread::spawn(move || control_loop(session, receive_controls));
    Ok(())
}
/// Processes direct backend controls sequentially, reporting errors even when the frontend is disconnected.
fn control_loop(session: Arc<Session>, controls: mpsc::Receiver<QueuedControl>) {
    while let Ok(control) = controls.recv() {
        let result = control.execute(&session);
        if let Err(error) = result {
            if error != crate::session::ACTIVE_SESSION {
                session.report(error);
            }
        }
    }
}
/// Pumps hooks and reservation jobs; every installed hook and actual registration is released before thread exit.
fn hook_loop(jobs: mpsc::Receiver<ReservationJob>, ready: mpsc::SyncSender<Result<(), String>>) {
    // SAFETY: initialized MSG is synchronous output storage; peeking creates this thread's queue before its ID is shared.
    let mut message: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
    }
    HOOK_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::Release);
    // SAFETY: static callbacks remain live until these hooks are uninstalled; global Session ownership outlives the loop.
    let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), null_mut(), 0) };
    // SAFETY: the same static-lifetime callback and process-local thread hook contract applies to mouse input.
    let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), null_mut(), 0) };
    let mut runtime = HookRuntime {
        record: false,
        stop: false,
        stop_available: false,
        hooks_ready: !keyboard.is_null() && !mouse.is_null(),
    };
    if runtime.hooks_ready {
        let _ = runtime.apply(false, true);
        let _ = runtime.apply(false, false);
    }
    let _ = ready.send(Ok(()));
    // SAFETY: message is initialized writable storage and each successful call returns a valid thread/window message.
    while unsafe { GetMessageW(&mut message, null_mut(), 0, 0) } > 0 {
        pump_hook_message(&message, &jobs, &mut runtime);
    }
    let _ = runtime.apply(false, false);
    // SAFETY: only handles successfully returned by installation are unhooked, after callbacks have stopped dispatching.
    if !keyboard.is_null() {
        unsafe {
            UnhookWindowsHookEx(keyboard);
        }
    }
    // SAFETY: mouse is likewise an installed, thread-owned live hook handle.
    if !mouse.is_null() {
        unsafe {
            UnhookWindowsHookEx(mouse);
        }
    }
    HOOK_THREAD.store(0, Ordering::Release);
}
/// Applies reservation jobs without callback work; reserved keyboard callbacks already queue control actions directly.
fn pump_hook_message(
    message: &MSG,
    jobs: &mpsc::Receiver<ReservationJob>,
    runtime: &mut HookRuntime,
) {
    if message.message == RESERVATION_MESSAGE {
        while let Ok(job) = jobs.try_recv() {
            let result = runtime.apply(job.record, job.stop);
            let _ = job.reply.send(result);
        }
    } else if message.message != WM_HOTKEY {
        // SAFETY: caller supplied an initialized message returned by GetMessageW; dispatch retains no local pointer.
        unsafe {
            TranslateMessage(message);
            DispatchMessageW(message);
        }
    }
}
/// Queues a control without waiting for a frontend callback; Stop freezes/cancels input before worker delivery.
fn queue_control(control: ControlJob) {
    let Some(session) = ENGINE.get() else {
        return;
    };
    match control.receive(session) {
        Ok(Some(job)) => {
            if let Some(sender) = CONTROL_JOBS.get() {
                let _ = sender.send(job);
            }
        }
        Ok(None) => {}
        Err(error) => session.report(error),
    }
}
/// Requests worker-owned cleanup following a native close request.
pub fn queue_stop() {
    queue_control(ControlJob::Stop);
}
/// Stops native hook pumping when the owned main window is destroyed.
pub fn shutdown_hooks() {
    let _ = wake_hook(WM_QUIT);
}
/// Returns whether the hook message thread remains live, for bounded progress-worker teardown.
pub fn hooks_running() -> bool {
    HOOK_THREAD.load(Ordering::Acquire) != 0
}
/// Checks external target focus; process-owned control windows are never captured.
fn own_window_focused() -> bool {
    // SAFETY: foreground HWND is OS-owned; process output points to initialized local storage for the synchronous getter.
    let foreground = unsafe { GetForegroundWindow() };
    let mut process = 0;
    if !foreground.is_null() {
        unsafe {
            GetWindowThreadProcessId(foreground, &mut process);
        }
    }
    // SAFETY: this by-value process identifier getter has no pointer or lifetime requirements.
    process == unsafe { GetCurrentProcessId() }
}

#[cfg(feature = "native-checks")]
static CHECK_KEY_DELIVERY: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "native-checks")]
const CHECK_KEY_DOWN: u32 = 1;
#[cfg(feature = "native-checks")]
const CHECK_KEY_UP: u32 = 2;
#[cfg(feature = "native-checks")]
const CHECK_KEY_COMPLETE: u32 = CHECK_KEY_UP;
#[cfg(feature = "native-checks")]
const CHECK_KEY_POLL: Duration = Duration::from_millis(10);

#[cfg(feature = "native-checks")]
/// Owns one isolated observation thread; dropping it stops the pump and releases its test hook before returning.
struct DeliveryObserver {
    thread: u32,
    worker: Option<std::thread::JoinHandle<()>>,
}
#[cfg(feature = "native-checks")]
impl DeliveryObserver {
    /// Starts a genuine low-level observer with a message pump, publishing its thread only after successful installation.
    fn start() -> Result<Self, String> {
        let (ready, receive) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || delivery_observer_loop(ready));
        let thread = receive
            .recv_timeout(HOOK_TIMEOUT)
            .map_err(|error| error.to_string())??;
        Ok(Self {
            thread,
            worker: Some(worker),
        })
    }
}
#[cfg(feature = "native-checks")]
impl Drop for DeliveryObserver {
    /// Requests shutdown of only this test-owned pump; joining ensures the static callback is uninstalled before later checks.
    fn drop(&mut self) {
        // SAFETY: startup publishes this thread only after creating its queue; WM_QUIT carries no pointers.
        unsafe {
            PostThreadMessageW(self.thread, WM_QUIT, 0, 0);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(feature = "native-checks")]
/// Observes OS-delivered tagged F9 down/up without consuming them; negative codes and unrelated input always chain normally.
unsafe extern "system" fn delivery_observer(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: Windows provides live callback storage for a nonnegative code; nothing is retained beyond this call.
        let input = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
        if input.dwExtraInfo == EXTRA_INFO && input.vkCode == u32::from(RECORD_KEY) {
            let bit = match message as u32 {
                WM_KEYDOWN => CHECK_KEY_DOWN,
                WM_KEYUP => CHECK_KEY_UP,
                _ => 0,
            };
            CHECK_KEY_DELIVERY.fetch_or(bit, Ordering::Release);
        }
    }
    // SAFETY: the original live hook arguments are forwarded without retaining the callback pointer.
    unsafe { CallNextHookEx(null_mut(), code, message, data) }
}
#[cfg(feature = "native-checks")]
/// Installs and pumps the test observer on its own thread; every successful installation is unhooked before thread exit.
fn delivery_observer_loop(ready: mpsc::SyncSender<Result<u32, String>>) {
    // SAFETY: zeroed MSG is writable output storage; peeking creates this thread's message queue before it is published.
    let mut message: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
    }
    // SAFETY: the callback is static and the hook is uninstalled on this thread before it exits.
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(delivery_observer), null_mut(), 0) };
    if hook.is_null() {
        let _ = ready.send(Err(HOOK_FAILED.into()));
        return;
    }
    // SAFETY: this getter has no borrowed pointers and identifies the thread owning the hook and initialized queue.
    if ready.send(Ok(unsafe { GetCurrentThreadId() })).is_err() {
        // SAFETY: startup was abandoned; this thread still owns the live hook and releases it before returning.
        unsafe {
            UnhookWindowsHookEx(hook);
        }
        return;
    }
    // SAFETY: message remains initialized synchronous output storage throughout this test-owned message pump.
    while unsafe { GetMessageW(&mut message, null_mut(), 0, 0) } > 0 {}
    // SAFETY: this thread still owns the valid hook returned above, after its message loop has stopped dispatching.
    unsafe {
        UnhookWindowsHookEx(hook);
    }
}
#[cfg(feature = "native-checks")]
/// Checks actual Windows-to-hook delivery while a test app window is focused, rather than directly calling our handler.
/// Sends only a tagged F9 release, avoiding injected holds even if dispatch fails; requires an interactive quiet desktop.
pub(crate) fn check_focused_key_delivery() -> Result<(), String> {
    if !own_window_focused() {
        return Err("The isolated check must own foreground focus before sending F9".into());
    }
    let _observer = DeliveryObserver::start()?;
    CHECK_KEY_DELIVERY.store(0, Ordering::Release);
    send(&[keyboard(RECORD_KEY, 0, false, false)])?;
    let deadline = std::time::Instant::now() + HOOK_TIMEOUT;
    while CHECK_KEY_DELIVERY.load(Ordering::Acquire) != CHECK_KEY_COMPLETE {
        if std::time::Instant::now() >= deadline {
            return Err(format!("F9 hook delivery missing with test app focused: observed mask={}, expected mask={CHECK_KEY_COMPLETE}", CHECK_KEY_DELIVERY.load(Ordering::Acquire)));
        }
        std::thread::sleep(CHECK_KEY_POLL);
    }
    if !own_window_focused() {
        return Err("The isolated check lost foreground focus during F9 delivery".into());
    }
    Ok(())
}
/// Returns whether a physical position is inside the current actual main-window frame.
fn own_point(point: POINT) -> bool {
    point_in_window(MAIN_HWND.load(Ordering::Acquire), point).unwrap_or(true)
}
/// Reads actual physical native bounds, propagating failure instead of assuming hidden library geometry.
fn point_in_window(hwnd: isize, point: POINT) -> Result<bool, String> {
    // SAFETY: zero RECT is valid initialized output storage; the HWND is retained by the owned live Tauri window.
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(hwnd as HWND, &mut rect) } == 0 {
        return Err(WINDOW_BOUNDS_ERROR.into());
    }
    Ok(
        point.x >= rect.left
            && point.x < rect.right
            && point.y >= rect.top
            && point.y < rect.bottom,
    )
}
/// Consumes physical reserved key sequences including repeats and trailing releases after reservation changes.
fn control_key(input: &KBDLLHOOKSTRUCT, down: bool) -> bool {
    if input.flags & LLKHF_INJECTED != 0 {
        return false;
    }
    let Ok(reserved) = RESERVED.lock() else {
        return false;
    };
    let Ok(mut sequences) = SEQUENCES.lock() else {
        return false;
    };
    let (consumed, control) = sequences.key(input.vkCode as u16, down, *reserved);
    drop(sequences);
    drop(reserved);
    if let Some(control) = control {
        queue_control(match control {
            Control::Record => ControlJob::Record,
            Control::Stop => ControlJob::Stop,
        });
    }
    consumed
}
/// Copies valid native key data into capture, or consumes reserved controls; negative hook codes always chain.
unsafe extern "system" fn keyboard_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: a nonnegative hook code supplies a KBDLLHOOKSTRUCT valid for this callback; no pointer is retained.
        let input = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
        if handle_keyboard(input, message) {
            return 1;
        }
    }
    // SAFETY: chaining passes Windows' original hook arguments without retaining their pointers.
    unsafe { CallNextHookEx(null_mut(), code, message, data) }
}
/// Handles a valid down/up message; unrelated messages and injected MacroLoom input are ignored.
fn handle_keyboard(input: &KBDLLHOOKSTRUCT, message: WPARAM) -> bool {
    let down = message == WM_KEYDOWN as usize || message == WM_SYSKEYDOWN as usize;
    let up = message == WM_KEYUP as usize || message == WM_SYSKEYUP as usize;
    if !down && !up {
        return false;
    }
    if control_key(input, down) {
        return true;
    }
    if RESERVED_CAPTURE_KEYS.contains(&(input.vkCode as u16)) {
        return false;
    }
    if input.dwExtraInfo == EXTRA_INFO || own_key_sequence(input.vkCode as u16, down) {
        return false;
    }
    if let Some(session) = ENGINE.get() {
        session.capture_with_layout(
            EventKind::Key {
                vk: input.vkCode as u16,
                scan: input.scanCode as u16,
                extended: input.flags & LLKHF_EXTENDED != 0,
                down,
            },
            &recording_keyboard_layout(),
        );
    }
    false
}
/// Reads only the foreground layout for an input-boundary compatibility check; monitor enumeration stays off hook callbacks.
fn recording_keyboard_layout() -> String {
    // SAFETY: OS-owned foreground HWND and layout are queried synchronously without retaining borrowed state.
    let thread = unsafe { GetWindowThreadProcessId(GetForegroundWindow(), null_mut()) };
    let layout = unsafe { GetKeyboardLayout(thread) };
    format!("{:08X}", layout as usize & KEYBOARD_LAYOUT_MASK)
}
/// Excludes the complete key gesture originating in a MacroLoom control, even if focus changes before release.
fn own_key_sequence(key: u16, down: bool) -> bool {
    let Ok(mut keys) = OWN_KEY_SEQUENCES.lock() else {
        return true;
    };
    keys.key(key, down, own_window_focused())
}

#[cfg(test)]
#[path = "windows_mouse_tests.rs"]
mod mouse_recording_tests;
#[cfg(test)]
#[path = "windows_recording_regression_tests.rs"]
mod recording_regression_tests;
/// Copies valid native mouse data before chaining; tagged injections and complete control-origin gestures are excluded.
unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: Windows supplies a live MSLLHOOKSTRUCT for nonnegative codes, copied before this callback returns.
        let input = unsafe { &*(data as *const MSLLHOOKSTRUCT) };
        handle_mouse(message as u32, input);
    }
    // SAFETY: original arguments are forwarded while Windows' callback storage remains valid.
    unsafe { CallNextHookEx(null_mut(), code, message, data) }
}
/// Classifies one native mouse callback without retaining the origin mutex while acquiring session ownership.
fn handle_mouse(message: u32, input: &MSLLHOOKSTRUCT) {
    if input.dwExtraInfo == EXTRA_INFO {
        return;
    }
    let kind = match OWN_MOUSE_SEQUENCES.lock() {
        Ok(mut mouse) => mouse.capture(message, input, own_point(input.pt)),
        Err(_) => None,
    };
    if let (Some(session), Some(kind)) = (ENGINE.get(), kind) {
        session.capture(kind);
    }
}
/// Converts supported positioned mouse transitions without inventing gestures or free movement playback.
fn mouse_kind(message: u32, input: &MSLLHOOKSTRUCT) -> Option<EventKind> {
    let point = input.pt;
    let transition = match message {
        WM_LBUTTONDOWN => Some((MouseButton::Left, true)),
        WM_LBUTTONUP => Some((MouseButton::Left, false)),
        WM_RBUTTONDOWN => Some((MouseButton::Right, true)),
        WM_RBUTTONUP => Some((MouseButton::Right, false)),
        _ => None,
    };
    if let Some((button, down)) = transition {
        return Some(EventKind::Mouse {
            button,
            down,
            x: point.x,
            y: point.y,
        });
    }
    if message == WM_MOUSEMOVE {
        return Some(EventKind::Move {
            x: point.x,
            y: point.y,
        });
    }
    if message == WM_MOUSEWHEEL {
        return Some(EventKind::Wheel {
            delta: i32::from((input.mouseData >> WHEEL_DATA_SHIFT) as i16),
            x: point.x,
            y: point.y,
        });
    }
    None
}
/// Installs a main-thread subclass for computer-sleep notification without replacing Tauri's window procedure.
pub fn watch_suspend(hwnd: isize) -> Result<(), String> {
    // SAFETY: called on the thread that owns the live main HWND; callback is static and retains no borrowed data.
    if unsafe { SetWindowSubclass(hwnd as HWND, Some(power_message), POWER_SUBCLASS, 0) } == 0 {
        return Err(POWER_HOOK_FAILED.into());
    }
    Ok(())
}
/// Signals interruption immediately on sleep, then lets the backend worker finish cleanup and retained capture.
unsafe extern "system" fn power_message(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message == WM_POWERBROADCAST && wparam == PBT_APMSUSPEND as usize {
        queue_control(ControlJob::Suspend);
    }
    if message == WM_DISPLAYCHANGE {
        if let Some(session) = ENGINE.get() {
            if let Err(error) = session.interrupt_recording(crate::recording::ENVIRONMENT_CHANGED) {
                session.report(error);
            }
        }
    }
    if message == WM_NCDESTROY {
        // SAFETY: this subclass identifier was installed on this same HWND; callback is removed before native destruction finishes.
        unsafe {
            RemoveWindowSubclass(hwnd, Some(power_message), POWER_SUBCLASS);
        }
        shutdown_hooks();
    }
    // SAFETY: forwards original arguments through the remaining valid subclass chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Reads current signed physical cursor coordinates; initialized zero coordinates are retained if the getter fails.
pub fn cursor_position() -> (i32, i32) {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: the OS writes to an initialized, live POINT on this stack.
    unsafe {
        GetCursorPos(&mut point);
    }
    (point.x, point.y)
}

/// Builds a physical virtual-desktop movement event tagged to exclude our own injected input from capture.
fn mouse_move(x: i32, y: i32) -> Result<INPUT, String> {
    // SAFETY: GetSystemMetrics takes only documented metric identifiers.
    let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    let dx = normalize_pixel(x, left, width)?;
    let dy = normalize_pixel(y, top, height)?;
    Ok(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                time: 0,
                dwExtraInfo: EXTRA_INFO,
            },
        },
    })
}

/// Targets the center of a signed pixel's absolute-input interval, avoiding one-pixel rounding errors at the interval edge.
/// Invalid metrics or coordinates fail rather than clamping a requested destination to another physical pixel.
fn normalize_pixel(pixel: i32, origin: i32, span: i32) -> Result<i32, String> {
    let offset = i64::from(pixel) - i64::from(origin);
    if span <= 0 || offset < 0 || offset >= i64::from(span) {
        return Err(INVALID_PHYSICAL_PIXEL.into());
    }
    Ok(((offset * NORMALIZED_RANGE + NORMALIZED_PIXEL_CENTER) / i64::from(span)) as i32)
}

/// Builds one tagged supported mouse-button transition without moving the cursor.
fn mouse_button(button: MouseButton, down: bool) -> INPUT {
    let flags = match (button, down) {
        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
    };
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: EXTRA_INFO,
            },
        },
    }
}

/// Builds a tagged vertical wheel event without normalizing its signed amount or changing button ownership.
fn mouse_wheel(delta: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: delta as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: EXTRA_INFO,
            },
        },
    }
}

/// Builds a tagged keyboard transition, preferring scan codes and preserving extended-key metadata.
fn keyboard(vk: u16, scan: u16, extended: bool, down: bool) -> INPUT {
    let mut flags = if scan != 0 { KEYEVENTF_SCANCODE } else { 0 };
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if !down {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: if scan != 0 { 0 } else { vk },
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: EXTRA_INFO,
            },
        },
    }
}

/// Sends initialized inputs and rejects partial/failed injection instead of claiming success.
fn send(inputs: &[INPUT]) -> Result<(), String> {
    // SAFETY: inputs is a live contiguous slice of initialized INPUT values;
    // its count and size describe exactly that slice for the duration of the call.
    let count = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    if count != inputs.len() as u32 {
        Err("Windows rejected an injected input event".into())
    } else {
        Ok(())
    }
}

/// Injects one supported event; positioned mouse dispatch splits movement from transition so successful holds are trackable.
pub fn send_event(kind: &EventKind) -> Result<(), String> {
    match kind {
        EventKind::Key {
            vk,
            scan,
            extended,
            down,
        } => send(&[keyboard(*vk, *scan, *extended, *down)]),
        EventKind::Mouse { button, down, x, y } => {
            send(&[mouse_move(*x, *y)?])?;
            send(&[mouse_button(*button, *down)])
        }
        EventKind::Move { x, y } => send(&[mouse_move(*x, *y)?]),
        EventKind::Wheel { delta, x, y } => {
            send(&[mouse_move(*x, *y)?])?;
            send(&[mouse_wheel(*delta)])
        }
    }
}

/// Rejects physical targets overlapping the actual compact frame before any movement or button injection.
pub fn send_session_event(hwnd: isize, kind: &EventKind) -> Result<(), String> {
    let target = match kind {
        EventKind::Key { .. } => None,
        EventKind::Mouse { x, y, .. }
        | EventKind::Move { x, y }
        | EventKind::Wheel { x, y, .. } => Some(POINT { x: *x, y: *y }),
    };
    if let Some(point) = target {
        if point_in_window(hwnd, point)? {
            return Err(COMPACT_COLLISION.into());
        }
    }
    send_event(kind)
}
/// Releases one session-owned injected input at its current position and propagates rejection for Retry cleanup.
pub fn release(held: Held) -> Result<(), String> {
    match held {
        Held::Key(vk, scan, extended) => send(&[keyboard(vk, scan, extended, false)]),
        Held::Button(button) => send(&[mouse_button(button, false)]),
    }
}

/// Holds a development-only global key until Enter; RAII always releases the reservation on normal/error exit.
pub fn hold_conflict_shortcut() -> Result<(), String> {
    if !cfg!(debug_assertions) {
        return Err(CONFLICT_HELPER_DISABLED.into());
    }
    let requested = std::env::args().nth(1).ok_or(CONFLICT_USAGE)?;
    let (id, key) = match requested.as_str() {
        CONFLICT_RECORD => (RECORD_ID, RECORD_KEY),
        CONFLICT_STOP => (STOP_ID, STOP_KEY),
        _ => return Err(CONFLICT_USAGE.into()),
    };
    if !update_registration(false, id, key, true)? {
        return Err(CONFLICT_HELPER_FAILED.into());
    }
    let _reservation = TestReservation(id);
    println!("Holding {requested} shortcut. Test MacroLoom, then press Enter here to release it.");
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    Ok(())
}
/// Test/helper-owned key on the current thread; never overlaps session-owned registration identifiers in another process.
struct TestReservation(i32);
impl Drop for TestReservation {
    /// Releases only this successfully reserved identifier before the helper's owning thread exits.
    fn drop(&mut self) {
        // SAFETY: constructed only after successful thread-local registration; null HWND matches that registration.
        unsafe {
            UnregisterHotKey(null_mut(), self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_KEY: u16 = 0x41;
    const CONTROL_MODIFIER: u16 = 0xa2;
    const SNAPSHOT_X: i32 = -20;
    const SNAPSHOT_Y: i32 = 100;
    const EXPECTED_SNAPSHOT: [u16; 3] = [CONTROL_MODIFIER, TEST_KEY, LEFT_MOUSE_KEY];
    const GUI_START_KEYS: [u16; 5] = [RECORD_KEY, STOP_KEY, 0x0d, 0x20, LEFT_MOUSE_KEY];
    #[test]
    /// Held-state snapshot orders modifiers before keys/buttons and never saves reserved controls or duplicate generic modifiers.
    fn held_snapshot_orders_inputs_and_excludes_control_gestures() {
        let inputs = ordered_initial_inputs(
            false,
            POINT {
                x: SNAPSHOT_X,
                y: SNAPSHOT_Y,
            },
            true,
            // Simulates simultaneous held keys, including generic Ctrl and the reserved Record key.
            |key| {
                [
                    CONTROL_MODIFIER,
                    GENERIC_MODIFIERS[1],
                    TEST_KEY,
                    RECORD_KEY,
                    STOP_KEY,
                    LEFT_MOUSE_KEY,
                ]
                .contains(&key)
            },
        );
        let actual: Vec<u16> = inputs
            .iter()
            .map(
                // Reduces events to their input identity while checking the saved initial mouse position.
                |input| match input {
                    EventKind::Key { vk, down, .. } => {
                        assert!(*down);
                        *vk
                    }
                    EventKind::Mouse { x, y, .. } => {
                        assert_eq!((*x, *y), (SNAPSHOT_X, SNAPSHOT_Y));
                        LEFT_MOUSE_KEY
                    }
                    EventKind::Move { .. } | EventKind::Wheel { .. } => {
                        panic!("Initial snapshot cannot contain movement or wheel input")
                    }
                },
            )
            .collect();
        assert_eq!(actual, EXPECTED_SNAPSHOT);
        assert!(ordered_initial_inputs(
            true,
            POINT {
                x: SNAPSHOT_X,
                y: SNAPSHOT_Y
            },
            false,
            // A GUI activation gesture must remain absent from the boundary snapshot.
            |key| GUI_START_KEYS.contains(&key)
        )
        .is_empty());
    }

    #[test]
    /// Negative monitor origins map to the same bounded absolute coordinates as positive desktop pixels.
    fn maps_negative_virtual_desktop_coordinates_to_absolute_input() {
        const VIRTUAL_ORIGIN: i32 = -1920;
        const VIRTUAL_SPAN: i32 = 3840;
        const FINAL_PIXEL: i32 = 1919;
        const FIRST_PIXEL_BIN_CENTER: i32 = 8;
        const FINAL_PIXEL_BIN_CENTER: i32 = 65527;
        const NEGATIVE_SEAM_PIXEL: i32 = -1;
        const PRIMARY_ORIGIN: i32 = 0;
        assert_eq!(
            normalize_pixel(VIRTUAL_ORIGIN, VIRTUAL_ORIGIN, VIRTUAL_SPAN)
                .expect("Leftmost physical pixel"),
            FIRST_PIXEL_BIN_CENTER
        );
        assert_eq!(
            normalize_pixel(FINAL_PIXEL, VIRTUAL_ORIGIN, VIRTUAL_SPAN)
                .expect("Rightmost physical pixel"),
            FINAL_PIXEL_BIN_CENTER
        );
        assert!(
            normalize_pixel(NEGATIVE_SEAM_PIXEL, VIRTUAL_ORIGIN, VIRTUAL_SPAN)
                .expect("Negative seam pixel")
                < normalize_pixel(PRIMARY_ORIGIN, VIRTUAL_ORIGIN, VIRTUAL_SPAN)
                    .expect("Primary seam pixel")
        );
        assert!(normalize_pixel(i32::MIN, VIRTUAL_ORIGIN, VIRTUAL_SPAN).is_err());
    }
    #[test]
    #[ignore = "Requires an interactive Windows desktop with F8 initially unreserved"]
    /// Actual OS conflict blocks required Stop, recovery acquires it, and terminal unregistration returns F8 to another owner.
    fn native_shortcut_conflict_recovers_and_releases_stop() {
        let (ready, installed) = mpsc::sync_channel(1);
        let (release, wait_release) = mpsc::sync_channel(1);
        let helper = std::thread::spawn(move || {
            let registered = update_registration(false, STOP_ID, STOP_KEY, true)
                .expect("Native fixture registration call");
            let _reservation = registered.then_some(TestReservation(STOP_ID));
            ready
                .send(registered)
                .expect("Fixture receiver remains alive");
            wait_release
                .recv()
                .expect("Fixture release is sent before join");
        });
        let installed = installed
            .recv_timeout(HOOK_TIMEOUT)
            .expect("Native fixture must become ready");
        if !installed {
            release.send(()).expect("Release fixture");
            helper.join().expect("Fixture thread");
            panic!("F8 is already reserved by another application; close it before this integration check");
        }
        let mut runtime = HookRuntime {
            record: false,
            stop: false,
            stop_available: false,
            hooks_ready: true,
        };
        assert!(
            !runtime
                .apply(false, true)
                .expect("Conflict is an availability outcome")
                .stop
        );
        release.send(()).expect("Release fixture");
        helper.join().expect("Fixture thread");
        assert!(
            runtime
                .apply(false, true)
                .expect("Retry acquires Stop")
                .stop
        );
        runtime
            .apply(false, false)
            .expect("Terminal cleanup releases Stop");
        assert!(update_registration(false, STOP_ID, STOP_KEY, true)
            .expect("Another owner can acquire released Stop"));
        let _reservation = TestReservation(STOP_ID);
    }
}

#[cfg(test)]
mod native_view_tests {
    use super::*;
    use crate::window_view::{ViewRequest, WindowView, MIN_CONTENT_WIDTH};
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetClientRect, IsZoomed, SendMessageW, SetForegroundWindow,
        ShowWindow, HTCAPTION, MA_ACTIVATE, MA_NOACTIVATE, SW_MAXIMIZE, WM_CANCELMODE,
        WM_MOUSEACTIVATE, WM_NCLBUTTONDOWN, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };

    const CLASS: &str = "MacroLoom native targeting fixture";
    const PIXEL_TARGET_CLASS: &str = "BUTTON";
    const MAIN_TITLE: &str = "MacroLoom compact native integration";
    const TARGET_TITLE: &str = "MacroLoom integration target";
    const FULL_WIDTH: i32 = 600;
    const FULL_HEIGHT: i32 = 480;
    const ORIGIN: i32 = 100;
    const CONTENT_HEIGHT: f64 = 156.0;
    const STATUS_UPDATES: usize = 20;
    // Keep the off-screen test origin within Win32's signed 16-bit window-position limit.
    const OFF_SCREEN: i32 = 30_000;
    const CONSTRAINT_FAILURE: &str = "Injected constraint failure after placement changed";
    const DRAG_DELTA_X: i32 = 64;
    const DRAG_DELTA_Y: i32 = 32;
    const CAPTION_OFFSET: i32 = 20;
    const CLIENT_DRAG_Y: i32 = 40;
    const PIXEL_SAMPLES: i32 = 64;
    const CURSOR_SETTLE: Duration = Duration::from_millis(10);
    const MAX_NATIVE_MESSAGES: usize = 100;
    const NATIVE_SETTLE: Duration = Duration::from_millis(50);
    const FRAME_MIDPOINT_DIVISOR: i32 = 2;
    const CURSOR_RESTORE_FAILED: &str =
        "Could not restore the native check's original cursor position";

    #[test]
    #[ignore = "Requires an interactive Windows desktop; injects one bounded title-bar drag on a disposable native window"]
    /// A real mouse gesture drives the compact timer/message path without activating the frame or leaving its button held.
    fn native_mouse_gesture_drags_compact_without_activation() {
        enable_physical_dpi();
        playback_inputs_released().expect("Release physical input before this isolated check");
        let _cursor = RestoreCursor(cursor_position());
        let windows = TestWindows::new();
        let mut view = WindowView::new(DesktopWindow::unconstrained(windows.main as isize));
        assert!(view
            .update(ViewRequest {
                compact: true,
                width: MIN_CONTENT_WIDTH,
                height: CONTENT_HEIGHT
            })
            .error
            .is_none());
        settle_native_messages();
        // SAFETY: observes the existing foreground without queuing a fixture activation that could complete during injected input.
        let target = unsafe { GetForegroundWindow() };
        assert!(!target.is_null());
        assert_ne!(target, windows.main);
        let before = rectangle(windows.main);
        let x = (before.0 + before.2) / FRAME_MIDPOINT_DIVISOR;
        let y = before.1 + CAPTION_OFFSET;
        // SAFETY: these by-value native hit tests verify the fixture owns the title-bar destination before any button injection.
        unsafe {
            assert_eq!(
                SendMessageW(
                    windows.main,
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_NCHITTEST,
                    0,
                    packed_point(POINT { x, y })
                ),
                HTCAPTION as isize
            );
            assert_eq!(
                windows_sys::Win32::UI::WindowsAndMessaging::WindowFromPoint(POINT { x, y }),
                windows.main
            );
        }
        send_event(&EventKind::Mouse {
            button: MouseButton::Left,
            down: true,
            x,
            y,
        })
        .expect("Owned title-bar press");
        let gesture = ReleaseTestButton;
        settle_native_messages();
        send_event(&EventKind::Move {
            x: x + DRAG_DELTA_X,
            y: y + DRAG_DELTA_Y,
        })
        .expect("Owned title-bar movement");
        settle_native_messages();
        drop(gesture);
        settle_native_messages();
        let moved = rectangle(windows.main);
        assert_eq!(
            (moved.0, moved.1),
            (before.0 + DRAG_DELTA_X, before.1 + DRAG_DELTA_Y)
        );
        // SAFETY: foreground observation never requests focus restoration or activation.
        assert_eq!(
            unsafe { GetForegroundWindow() },
            target,
            "main={:?}, fixture target={:?}",
            windows.main,
            windows.target
        );
        playback_inputs_released().expect("The simulated manual drag releases its button");
    }

    /// Owns only the left press injected by the isolated native drag check, including panic cleanup.
    struct ReleaseTestButton;
    impl Drop for ReleaseTestButton {
        /// Releases the fixture's injected button at its current position before any test-owned windows are destroyed.
        fn drop(&mut self) {
            if let Err(error) = release(Held::Button(MouseButton::Left)) {
                eprintln!("{error}");
            }
        }
    }

    /// Gives the disposable event thread enough bounded pump cycles to process input and the short drag timer.
    fn settle_native_messages() {
        let end = std::time::Instant::now() + NATIVE_SETTLE;
        while std::time::Instant::now() < end {
            pump_native_messages();
            std::thread::sleep(CURSOR_SETTLE);
        }
        pump_native_messages();
    }

    #[test]
    #[ignore = "Requires an interactive Windows desktop; moves the shared cursor only over disposable native target windows"]
    /// Actual virtual-desktop injection lands at each requested physical pixel, beyond merely monotonic normalized coordinates.
    fn native_positioned_movement_lands_on_exact_physical_pixels() {
        enable_physical_dpi();
        playback_inputs_released().expect("Release physical input before this isolated check");
        let cursor = RestoreCursor(cursor_position());
        let environment = recording_environment().expect("Physical display metadata");
        eprintln!("Physical targeting environment: {environment:?}");
        for display in &environment.displays {
            check_physical_landing(display);
        }
        drop(cursor);
    }

    /// Moves only disposable frames onto one unchanged monitor and checks physical samples below the compact toolbar.
    fn check_physical_landing(display: &crate::macro_format::Display) {
        let windows = TestWindows::with_target_class(PIXEL_TARGET_CLASS);
        for hwnd in [windows.main, windows.target] {
            // SAFETY: only test-owned windows move; destination pixels are on an enumerated current monitor and activation is forbidden.
            assert_ne!(
                unsafe {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        display.x + ORIGIN,
                        display.y + ORIGIN,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                    )
                },
                0
            );
        }
        let mut view = WindowView::new(DesktopWindow::unconstrained(windows.main as isize));
        assert!(view
            .update(ViewRequest {
                compact: true,
                width: MIN_CONTENT_WIDTH,
                height: CONTENT_HEIGHT
            })
            .error
            .is_none());
        windows.focus_target();
        // SAFETY: raises only this test-owned target without activation, ensuring every sampled mouse point belongs to the fixture.
        assert_ne!(
            unsafe {
                SetWindowPos(
                    windows.target,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE
                        | SWP_NOSIZE
                        | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOMOVE,
                )
            },
            0
        );
        let frame = rectangle(windows.main);
        let target = rectangle(windows.target);
        let y = frame.3 + CAPTION_OFFSET;
        assert!(y < target.3 - CAPTION_OFFSET);
        for offset in 0..PIXEL_SAMPLES {
            let x = target.0 + CAPTION_OFFSET + offset;
            let y = y + offset;
            assert!(y < target.3 - CAPTION_OFFSET);
            // SAFETY: value-only hit testing confirms the test owns the destination before any shared-cursor movement.
            assert_eq!(
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::WindowFromPoint(POINT { x, y })
                },
                windows.target
            );
            send_session_event(windows.main as isize, &EventKind::Move { x, y })
                .expect("Isolated positioned movement");
            pump_native_messages();
            std::thread::sleep(CURSOR_SETTLE);
            pump_native_messages();
            assert_eq!(
                cursor_position(),
                (x, y),
                "Physical cursor landing differs from the requested target"
            );
        }
    }

    /// Services the disposable target's input queue so Windows can process injected motion before physical-position assertions.
    fn pump_native_messages() {
        let mut message = MSG::default();
        for _ in 0..MAX_NATIVE_MESSAGES {
            // SAFETY: initialized local MSG and this test thread's queue; no other thread's window messages are dispatched.
            if unsafe {
                PeekMessageW(
                    &mut message,
                    null_mut(),
                    0,
                    0,
                    windows_sys::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                )
            } == 0
            {
                break;
            }
            // SAFETY: PeekMessageW initialized this by-value message; its target belongs to the current event thread.
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    /// Retains the pre-check shared-cursor position so both successful assertions and panics restore it.
    struct RestoreCursor((i32, i32));
    impl Drop for RestoreCursor {
        /// Restores only the physical pointer, without injecting buttons, keys or a target-window activation.
        fn drop(&mut self) {
            let (x, y) = self.0;
            // SAFETY: signed coordinates were read from the OS immediately before the bounded test.
            if unsafe { windows_sys::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y) } == 0 {
                eprintln!("{CURSOR_RESTORE_FAILED}");
            }
        }
    }

    #[test]
    #[ignore = "Requires an interactive Windows desktop; opens disposable native test windows"]
    /// A real compact frame accepts title-bar input without activating itself or restoring another window's focus.
    fn native_compact_titlebar_drag_preserves_focus() {
        enable_physical_dpi();
        let windows = TestWindows::new();
        let mut view = WindowView::new(DesktopWindow::unconstrained(windows.main as isize));
        assert!(view
            .update(ViewRequest {
                compact: true,
                width: MIN_CONTENT_WIDTH,
                height: CONTENT_HEIGHT,
            })
            .error
            .is_none());
        let target = windows.focus_target();
        let activation = ((WM_NCLBUTTONDOWN as isize) << MOUSE_WORD_SHIFT) | HTCAPTION as isize;
        // SAFETY: the test owns the live native window; WM_MOUSEACTIVATE takes only by-value native parameters.
        assert_eq!(
            unsafe {
                SendMessageW(
                    windows.main,
                    WM_MOUSEACTIVATE,
                    windows.main as usize,
                    activation,
                )
            },
            MA_NOACTIVATE as isize
        );
        // SAFETY: getter returns an OS-owned handle only; the target remains alive for this test.
        assert_eq!(unsafe { GetForegroundWindow() }, target);
        let before = rectangle(windows.main);
        let anchor = POINT {
            x: before.0 + CAPTION_OFFSET,
            y: before.1 + CAPTION_OFFSET,
        };
        assert_eq!(
            send_session_event(
                windows.main as isize,
                &EventKind::Move {
                    x: anchor.x,
                    y: anchor.y
                }
            )
            .err()
            .as_deref(),
            Some(COMPACT_COLLISION)
        );
        let mut client = POINT {
            x: 0,
            y: CLIENT_DRAG_Y,
        };
        // SAFETY: the live test window converts initialized local client coordinates synchronously.
        assert_ne!(
            unsafe { windows_sys::Win32::Graphics::Gdi::ClientToScreen(windows.main, &mut client) },
            0
        );
        let movement = packed_point(POINT {
            x: anchor.x + DRAG_DELTA_X - client.x,
            y: anchor.y + DRAG_DELTA_Y - client.y + CLIENT_DRAG_Y,
        });
        // SAFETY: controlled messages target only our test-owned frame; no shared cursor movement or input injection occurs.
        unsafe {
            SendMessageW(
                windows.main,
                WM_NCLBUTTONDOWN,
                HTCAPTION as usize,
                packed_point(anchor),
            );
            SendMessageW(windows.main, WM_MOUSEMOVE, 0, movement);
            SendMessageW(windows.main, WM_CANCELMODE, 0, 0);
        }
        let moved = rectangle(windows.main);
        assert_eq!(
            (moved.0, moved.1),
            (before.0 + DRAG_DELTA_X, before.1 + DRAG_DELTA_Y)
        );
        // SAFETY: foreground getter observes the still-live target without changing activation.
        assert_eq!(unsafe { GetForegroundWindow() }, target);
        assert_eq!(
            send_session_event(
                windows.main as isize,
                &EventKind::Move {
                    x: moved.0 + CAPTION_OFFSET,
                    y: moved.1 + CAPTION_OFFSET
                }
            )
            .err()
            .as_deref(),
            Some(COMPACT_COLLISION)
        );
        assert!(view
            .update(ViewRequest {
                compact: false,
                width: MIN_CONTENT_WIDTH,
                height: CONTENT_HEIGHT
            })
            .error
            .is_none());
        // SAFETY: default activation behavior is observed only through a by-value message on the owned restored window.
        assert_eq!(
            unsafe {
                SendMessageW(
                    windows.main,
                    WM_MOUSEACTIVATE,
                    windows.main as usize,
                    activation,
                )
            },
            MA_ACTIVATE as isize
        );
    }

    /// Packs bounded fixture coordinates into the signed-word representation used by native mouse messages.
    fn packed_point(point: POINT) -> LPARAM {
        (u32::from(point.x as u16) | (u32::from(point.y as u16) << MOUSE_WORD_SHIFT)) as isize
    }

    /// Builds a real native adapter with a one-shot constraint failure after full-placement mutation.
    fn faultable_adapter(hwnd: isize, fail_restore: Arc<AtomicBool>) -> DesktopWindow {
        let mut adapter = DesktopWindow::unconstrained(hwnd);
        adapter.constraints = Box::new(
            // Compact rollback succeeds; the first requested full constraint change deliberately fails.
            move |compact| {
                if !compact && fail_restore.swap(false, Ordering::SeqCst) {
                    return Err(CONSTRAINT_FAILURE.into());
                }
                Ok(())
            },
        );
        adapter
    }

    /// Owns disposable native test windows; production adapters never accept an external HWND through IPC.
    struct TestWindows {
        main: HWND,
        target: HWND,
        previous: HWND,
    }

    impl TestWindows {
        /// Creates a main frame and another application's focus stand-in on the calling native thread.
        fn new() -> Self {
            Self::with_target_class(CLASS)
        }
        /// Selects a nontransparent system class when a test must verify ownership of injected mouse destinations.
        fn with_target_class(target_class: &str) -> Self {
            // SAFETY: OS-owned foreground handle is retained only for best-effort cleanup.
            let previous = unsafe { GetForegroundWindow() };
            Self {
                main: create(MAIN_TITLE),
                target: create_class(TARGET_TITLE, target_class),
                previous,
            }
        }
        /// Makes the target foreground before measurements; setup may activate, product transitions may not.
        fn focus_target(&self) -> HWND {
            // SAFETY: target is owned by this test and remains live until Drop.
            unsafe {
                SetForegroundWindow(self.target);
            }
            settle_native_messages();
            // SAFETY: getter returns an OS-owned value with no borrowed pointer.
            let foreground = unsafe { GetForegroundWindow() };
            // Windows may deny test foreground setup; an already foreground external window is equally valid evidence.
            assert!(!foreground.is_null());
            assert_ne!(
                foreground, self.main,
                "An external window must have focus before the product transition"
            );
            foreground
        }
    }

    impl Drop for TestWindows {
        /// Releases only these test-owned HWNDs and best-effort restores the user's pre-test foreground.
        fn drop(&mut self) {
            // SAFETY: both HWNDs were created on this thread and remain owned until this cleanup; previous is never dereferenced.
            unsafe {
                DestroyWindow(self.main);
                DestroyWindow(self.target);
                SetForegroundWindow(self.previous);
            }
        }
    }

    /// Creates a disposable decorated frame of the registered fixture class; native ownership lasts until TestWindows drops.
    fn create(title: &str) -> HWND {
        create_class(title, CLASS)
    }

    /// Creates a decorated frame of the selected OS-owned class; initialized UTF-16 strings live through CreateWindowExW.
    fn create_class(title: &str, class: &str) -> HWND {
        if class == CLASS {
            register_frame_class();
        }
        let class: Vec<u16> = class.encode_utf16().chain(std::iter::once(0)).collect();
        let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: registered/system class, initialized terminated strings and the current executable module; no borrowed context survives the call.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                ORIGIN,
                ORIGIN,
                FULL_WIDTH,
                FULL_HEIGHT,
                null_mut(),
                null_mut(),
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(null_mut()),
                null_mut(),
            )
        };
        assert!(!hwnd.is_null());
        hwnd
    }

    /// Registers a real decorated-frame class once; control classes such as STATIC return transparent nonclient hit tests.
    fn register_frame_class() {
        static ATOM: OnceLock<u16> = OnceLock::new();
        ATOM.get_or_init(|| {
            let class: Vec<u16> = CLASS.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: queries this test executable's module; the class name is copied synchronously by RegisterClassW.
            let description = windows_sys::Win32::UI::WindowsAndMessaging::WNDCLASSW {
                lpfnWndProc: Some(windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW),
                hInstance: unsafe {
                    windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(null_mut())
                },
                lpszClassName: class.as_ptr(),
                ..Default::default()
            };
            // SAFETY: initialized class description and static system window procedure remain valid throughout this synchronous registration.
            let atom = unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::RegisterClassW(&description)
            };
            assert_ne!(atom, 0, "The disposable native frame class must register");
            atom
        });
    }

    /// Reads initialized physical bounds of a test-owned live HWND for independent native assertions.
    fn rectangle(hwnd: HWND) -> (i32, i32, i32, i32) {
        // SAFETY: RECT accepts zero initialization; caller supplies a live test-owned handle.
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        // SAFETY: local writable RECT remains live through the synchronous call.
        assert_ne!(unsafe { GetWindowRect(hwnd, &mut rect) }, 0);
        (rect.left, rect.top, rect.right, rect.bottom)
    }

    /// Independently checks every edge of the real native frame against its monitor's physical work area.
    fn assert_on_work_area(hwnd: HWND) {
        // SAFETY: initialized output values; caller owns the live native test window.
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = size_of::<MONITORINFO>() as u32;
        // SAFETY: synchronous getters write correctly sized local storage; monitor handle remains OS-owned.
        unsafe {
            assert_ne!(GetWindowRect(hwnd, &mut rect), 0);
            let monitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
            assert_ne!(GetMonitorInfoW(monitor, &mut info), 0);
        }
        assert!(rect.left >= info.rcWork.left && rect.top >= info.rcWork.top);
        assert!(rect.right <= info.rcWork.right && rect.bottom <= info.rcWork.bottom);
    }

    /// Verifies the public native adapter's actual bounds, frame, topmost, focus, and restoration on an interactive desktop.
    fn exercise(maximized: bool) {
        let windows = TestWindows::new();
        if maximized {
            // SAFETY: main is test-owned; activation is allowed during test setup before target focus is established.
            unsafe {
                ShowWindow(windows.main, SW_MAXIMIZE);
            }
        }
        let full = rectangle(windows.main);
        let target = windows.focus_target();
        let fail_restore = Arc::new(AtomicBool::new(false));
        let mut view = WindowView::new(faultable_adapter(
            windows.main as isize,
            fail_restore.clone(),
        ));
        let request = ViewRequest {
            compact: true,
            width: MIN_CONTENT_WIDTH,
            height: CONTENT_HEIGHT,
        };
        let result = view.update(request);
        assert!(result.error.is_none(), "{:?}", result.error);
        // SAFETY: getters operate on the test-owned main HWND and initialized output storage only.
        let mut client: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            assert_ne!(GetClientRect(windows.main, &mut client), 0);
        }
        // SAFETY: the test-owned main HWND is live; DPI/style getters return values only.
        let dpi = unsafe { GetDpiForWindow(windows.main) };
        let scale = f64::from(dpi) / f64::from(BASE_DPI);
        eprintln!("Native compact check: DPI {dpi}, maximized {maximized}");
        assert_eq!(
            client.right - client.left,
            (MIN_CONTENT_WIDTH * scale).ceil() as i32
        );
        assert_eq!(
            client.bottom - client.top,
            (CONTENT_HEIGHT * scale).ceil() as i32
        );
        // SAFETY: live test window; by-value native style getters retain no pointers.
        unsafe {
            assert_ne!(
                GetWindowLongW(windows.main, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST,
                0
            );
            assert_eq!(
                GetWindowLongW(windows.main, GWL_STYLE) as u32 & (WS_THICKFRAME | WS_MAXIMIZEBOX),
                0
            );
        }
        for _ in 0..STATUS_UPDATES {
            assert!(view.update(request).error.is_none());
        }
        // SAFETY: foreground getter returns an OS-owned value only.
        assert_eq!(unsafe { GetForegroundWindow() }, target);
        // SAFETY: moves only the owned compact test window; size and focus stay unchanged.
        unsafe {
            assert_ne!(
                SetWindowPos(
                    windows.main,
                    HWND_TOPMOST,
                    OFF_SCREEN,
                    OFF_SCREEN,
                    0,
                    0,
                    SWP_NOACTIVATE | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOSIZE,
                ),
                0
            );
        }
        let off_screen = rectangle(windows.main);
        assert_eq!((off_screen.0, off_screen.1), (OFF_SCREEN, OFF_SCREEN));
        assert!(view.refresh().error.is_none());
        let moved = rectangle(windows.main);
        assert!(moved.0 < OFF_SCREEN && moved.1 < OFF_SCREEN);
        assert_on_work_area(windows.main);
        fail_restore.store(true, Ordering::SeqCst);
        let failed_restore = view.update(ViewRequest {
            compact: false,
            ..request
        });
        assert!(failed_restore.compact);
        assert_eq!(failed_restore.error.as_deref(), Some(CONSTRAINT_FAILURE));
        assert_eq!(rectangle(windows.main), moved);
        // SAFETY: test-owned HWND remains live; getters return values without side effects.
        unsafe {
            assert_eq!(GetForegroundWindow(), target);
            assert_ne!(
                GetWindowLongW(windows.main, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST,
                0
            );
            assert_eq!(
                GetWindowLongW(windows.main, GWL_STYLE) as u32 & (WS_THICKFRAME | WS_MAXIMIZEBOX),
                0
            );
        }
        let restored = view.update(ViewRequest {
            compact: false,
            ..request
        });
        assert!(restored.error.is_none(), "{:?}", restored.error);
        assert_eq!(rectangle(windows.main), full);
        // SAFETY: test-owned HWND remains live and getters have no pointer/lifetime side effects.
        unsafe {
            assert_eq!(GetForegroundWindow(), target);
            assert_eq!(
                GetWindowLongW(windows.main, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST,
                0
            );
            assert_eq!(IsZoomed(windows.main) != 0, maximized);
        }
    }

    #[test]
    #[ignore = "Requires an interactive Windows desktop; opens disposable native test windows"]
    /// Normal and maximized transitions preserve external focus through repeated updates and restoration.
    fn native_compact_bounds_focus_and_maximized_restoration() {
        enable_physical_dpi();
        exercise(false);
        exercise(true);
    }
}
