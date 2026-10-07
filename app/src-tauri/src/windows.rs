use crate::window_view::{ContentSize, Position, WindowAdapter};
use crate::{Engine, EventKind, Held, MouseButton};
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::{Arc, OnceLock};
use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, SendInput, UnregisterHotKey, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
    KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOD_NOREPEAT,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, VK_F8, VK_F9,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetSystemMetrics, GetWindowLongW, GetWindowPlacement, GetWindowRect, GetWindowThreadProcessId,
    SetWindowLongW, SetWindowPlacement, SetWindowPos, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, GWL_EXSTYLE, GWL_STYLE, HWND_NOTOPMOST, HWND_TOPMOST, KBDLLHOOKSTRUCT,
    MSG, MSLLHOOKSTRUCT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_SHOWWINDOW,
    SW_SHOWNOACTIVATE, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWPLACEMENT, WM_HOTKEY, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP, WS_MAXIMIZE, WS_MAXIMIZEBOX, WS_THICKFRAME,
};

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();
const RECORD_HOTKEY: i32 = 1;
const STOP_HOTKEY: i32 = 2;
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

/// Enables per-monitor physical-coordinate handling before creating any application windows.
pub fn enable_physical_dpi() {
    // SAFETY: this process-wide Win32 call takes a documented constant, with no borrowed pointers.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[derive(Clone, Copy)]
/// Full native placement, physical visible bounds, and original frame style retained through compact movement.
pub struct FullWindowPlacement {
    bounds: RECT,
    placement: WINDOWPLACEMENT,
    style: i32,
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

    /// Changes frame bits with explicit Win32 error handling; no pointers or activation requests are used.
    fn style(&self, style: i32) -> Result<(), String> {
        // SAFETY: HWND remains owned by the live main window; GWL_STYLE accepts a value, not a borrowed pointer.
        unsafe {
            SetLastError(0);
        }
        // SAFETY: the same live HWND and documented style index are valid throughout this event-thread operation.
        let previous = unsafe { SetWindowLongW(self.hwnd as HWND, GWL_STYLE, style) };
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
        let mut placement = saved.placement;
        placement.showCmd = SW_SHOWNOACTIVATE as u32;
        // SAFETY: live HWND; captured initialized placement is local and its show command forbids activation.
        if unsafe { SetWindowPlacement(self.hwnd as HWND, &placement) } == 0 {
            return Err(WINDOW_RESTORE_ERROR.into());
        }
        (self.constraints)(compact)?;
        self.style(saved.style)?;
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
        Ok(FullWindowPlacement {
            bounds,
            placement,
            style,
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
        self.style(style as i32)?;
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
        self.place(self.content_bounds(position, size, style as i32)?, true)
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

pub struct WindowState {
    hwnd: isize,
    original: Option<RECT>,
}

impl WindowState {
    pub fn new(hwnd: isize) -> Self {
        Self {
            hwnd,
            original: None,
        }
    }

    pub fn compact(&mut self, compact: bool) -> Result<(), String> {
        let hwnd = self.hwnd as HWND;
        // SAFETY: hwnd comes from Tauri's live main window. RECT storage is initialized
        // before reading it; SetWindowPos receives values, and failures are propagated.
        unsafe {
            if compact {
                if self.original.is_some() {
                    return Ok(());
                }
                let mut rect = std::mem::zeroed();
                if GetWindowRect(hwnd, &mut rect) == 0 {
                    return Err("Could not read window bounds".into());
                }
                let width = rect.right - rect.left;
                let height = 230 * (GetDpiForWindow(hwnd) as i32).max(96) / 96;
                if SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    rect.left,
                    rect.top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
                ) == 0
                {
                    return Err("Could not enter compact window state".into());
                }
                self.original = Some(rect);
            } else if let Some(rect) = self.original {
                if SetWindowPos(
                    hwnd,
                    HWND_NOTOPMOST,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
                ) == 0
                {
                    return Err("Could not restore window state".into());
                }
                self.original = None;
            }
        }
        Ok(())
    }
}

pub fn start_hooks(engine: Arc<Engine>) {
    let _ = ENGINE.set(engine.clone());
    // SAFETY: hooks use static callbacks; message storage is initialized and all
    // successfully installed hooks/hotkeys are released on message-loop exit.
    std::thread::spawn(move || unsafe {
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), null_mut(), 0);
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), null_mut(), 0);
        let f9 = RegisterHotKey(null_mut(), RECORD_HOTKEY, MOD_NOREPEAT, VK_F9 as u32) != 0;
        let f8 = RegisterHotKey(null_mut(), STOP_HOTKEY, MOD_NOREPEAT, VK_F8 as u32) != 0;
        engine.set_hotkeys(f9, f8 && !keyboard.is_null() && !mouse.is_null());
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            handle_hook_message(&engine, &msg);
        }
        if f9 {
            UnregisterHotKey(null_mut(), RECORD_HOTKEY);
        }
        if f8 {
            UnregisterHotKey(null_mut(), STOP_HOTKEY);
        }
        if !keyboard.is_null() {
            UnhookWindowsHookEx(keyboard);
        }
        if !mouse.is_null() {
            UnhookWindowsHookEx(mouse);
        }
    });
}

unsafe fn handle_hook_message(engine: &Engine, message: &MSG) {
    // SAFETY: the caller supplies an initialized MSG returned by GetMessageW.
    if message.message == WM_HOTKEY {
        if message.wParam == RECORD_HOTKEY as usize {
            let _ = engine.start_recording();
        }
        if message.wParam == STOP_HOTKEY as usize {
            let _ = engine.stop();
        }
    } else {
        TranslateMessage(message);
        DispatchMessageW(message);
    }
}

fn own_window_focused() -> bool {
    // SAFETY: foreground HWND is OS-owned; process output points to live local storage.
    unsafe {
        let foreground = GetForegroundWindow();
        let mut process = 0;
        if !foreground.is_null() {
            GetWindowThreadProcessId(foreground, &mut process);
        }
        process == GetCurrentProcessId()
    }
}

fn own_point(point: POINT) -> bool {
    // SAFETY: hwnd is held by Tauri and rect points to initialized local storage.
    unsafe {
        let Some(engine) = ENGINE.get() else {
            return false;
        };
        let hwnd = engine.window.lock().unwrap().hwnd as HWND;
        let mut rect = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect) != 0
            && point.x >= rect.left
            && point.x < rect.right
            && point.y >= rect.top
            && point.y < rect.bottom
    }
}

unsafe extern "system" fn keyboard_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    // SAFETY: for a nonnegative hook code Windows supplies a KBDLLHOOKSTRUCT
    // valid for this callback. We never retain its pointer, and always chain the hook.
    if code >= 0 && !own_window_focused() {
        let input = &*(data as *const KBDLLHOOKSTRUCT);
        if is_capture_key(input) {
            let down = message == WM_KEYDOWN as usize || message == WM_SYSKEYDOWN as usize;
            let up = message == WM_KEYUP as usize || message == WM_SYSKEYUP as usize;
            if down || up {
                if let Some(engine) = ENGINE.get() {
                    engine.capture(EventKind::Key {
                        vk: input.vkCode as u16,
                        scan: input.scanCode as u16,
                        extended: input.flags & 1 != 0,
                        down,
                    });
                }
            }
        }
    }
    CallNextHookEx(null_mut(), code, message, data)
}

fn is_capture_key(input: &KBDLLHOOKSTRUCT) -> bool {
    input.dwExtraInfo != EXTRA_INFO && input.vkCode != VK_F8 as u32 && input.vkCode != VK_F9 as u32
}

unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    // SAFETY: a nonnegative mouse-hook code supplies a live MSLLHOOKSTRUCT.
    // Its data is copied into owned events before this callback returns.
    if code >= 0 {
        let input = &*(data as *const MSLLHOOKSTRUCT);
        if input.dwExtraInfo != EXTRA_INFO && !own_point(input.pt) {
            let kind = match message as u32 {
                WM_LBUTTONDOWN => Some(EventKind::Mouse {
                    button: MouseButton::Left,
                    down: true,
                    x: input.pt.x,
                    y: input.pt.y,
                }),
                WM_LBUTTONUP => Some(EventKind::Mouse {
                    button: MouseButton::Left,
                    down: false,
                    x: input.pt.x,
                    y: input.pt.y,
                }),
                WM_RBUTTONDOWN => Some(EventKind::Mouse {
                    button: MouseButton::Right,
                    down: true,
                    x: input.pt.x,
                    y: input.pt.y,
                }),
                WM_RBUTTONUP => Some(EventKind::Mouse {
                    button: MouseButton::Right,
                    down: false,
                    x: input.pt.x,
                    y: input.pt.y,
                }),
                WM_MOUSEMOVE => Some(EventKind::Move {
                    x: input.pt.x,
                    y: input.pt.y,
                }),
                _ => None,
            };
            if let (Some(engine), Some(kind)) = (ENGINE.get(), kind) {
                engine.capture(kind);
            }
        }
    }
    CallNextHookEx(null_mut(), code, message, data)
}

pub fn cursor_position() -> (i32, i32) {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: the OS writes to an initialized, live POINT on this stack.
    unsafe {
        GetCursorPos(&mut point);
    }
    (point.x, point.y)
}

fn mouse_move(x: i32, y: i32) -> INPUT {
    // SAFETY: GetSystemMetrics takes only documented metric identifiers.
    let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(1);
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(1);
    let dx = normalize_pixel(x, left, width);
    let dy = normalize_pixel(y, top, height);
    INPUT {
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
    }
}

fn normalize_pixel(pixel: i32, origin: i32, span: i32) -> i32 {
    ((pixel as i64 - origin as i64) * 65535 / (span - 1).max(1) as i64).clamp(0, 65535) as i32
}

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

pub fn send_event(kind: &EventKind) -> Result<(), String> {
    match kind {
        EventKind::Key {
            vk,
            scan,
            extended,
            down,
        } => send(&[keyboard(*vk, *scan, *extended, *down)]),
        EventKind::Mouse { button, down, x, y } => {
            send(&[mouse_move(*x, *y), mouse_button(*button, *down)])
        }
        EventKind::Move { x, y } => send(&[mouse_move(*x, *y)]),
    }
}

pub fn release(held: Held) -> Result<(), String> {
    match held {
        Held::Key(vk, scan, extended) => send(&[keyboard(vk, scan, extended, false)]),
        Held::Button(button) => send(&[mouse_button(button, false)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_negative_virtual_desktop_coordinates_to_absolute_input() {
        assert_eq!(normalize_pixel(-1920, -1920, 3840), 0);
        assert_eq!(normalize_pixel(1919, -1920, 3840), 65535);
        assert!(normalize_pixel(-1, -1920, 3840) < normalize_pixel(0, -1920, 3840));
    }

    #[test]
    fn capture_excludes_control_keys_and_our_injected_input() {
        let input = |vk_code, extra| KBDLLHOOKSTRUCT {
            vkCode: vk_code,
            scanCode: 0,
            flags: 0,
            time: 0,
            dwExtraInfo: extra,
        };
        assert!(is_capture_key(&input(65, 0)));
        assert!(!is_capture_key(&input(VK_F8 as u32, 0)));
        assert!(!is_capture_key(&input(VK_F9 as u32, 0)));
        assert!(!is_capture_key(&input(65, EXTRA_INFO)));
    }
}

#[cfg(test)]
mod native_view_tests {
    use super::*;
    use crate::window_view::{ViewRequest, WindowView, MIN_CONTENT_WIDTH};
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetClientRect, IsZoomed, SetForegroundWindow, ShowWindow,
        SW_MAXIMIZE, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };

    const CLASS: &str = "STATIC";
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
            // SAFETY: OS-owned foreground handle is retained only for best-effort cleanup.
            let previous = unsafe { GetForegroundWindow() };
            Self {
                main: create(MAIN_TITLE),
                target: create(TARGET_TITLE),
                previous,
            }
        }
        /// Makes the target foreground before measurements; setup may activate, product transitions may not.
        fn focus_target(&self) -> HWND {
            // SAFETY: target is owned by this test and remains live until Drop.
            unsafe {
                SetForegroundWindow(self.target);
            }
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

    /// Creates a disposable decorated STATIC-class frame; all string pointers remain live through the synchronous call.
    fn create(title: &str) -> HWND {
        let class: Vec<u16> = CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: OS-provided STATIC class; terminated strings are initialized and live; no parent/menu/instance/context is borrowed.
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
                null_mut(),
                null_mut(),
            )
        };
        assert!(!hwnd.is_null());
        hwnd
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
