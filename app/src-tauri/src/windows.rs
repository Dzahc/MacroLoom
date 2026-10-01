use crate::{Engine, EventKind, Held, MouseButton};
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::{Arc, OnceLock};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, SendInput, UnregisterHotKey, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
    KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOD_NOREPEAT,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, VK_F8, VK_F9,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetSystemMetrics, GetWindowRect, GetWindowThreadProcessId, SetWindowPos, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HWND_NOTOPMOST, HWND_TOPMOST, KBDLLHOOKSTRUCT, MSG,
    MSLLHOOKSTRUCT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_SHOWWINDOW, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_HOTKEY,
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();
const RECORD_HOTKEY: i32 = 1;
const STOP_HOTKEY: i32 = 2;
const EXTRA_INFO: usize = 0x4D4C_3038;

pub fn enable_physical_dpi() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
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
    std::thread::spawn(move || unsafe {
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), null_mut(), 0);
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), null_mut(), 0);
        let f9 = RegisterHotKey(null_mut(), RECORD_HOTKEY, MOD_NOREPEAT, VK_F9 as u32) != 0;
        let f8 = RegisterHotKey(null_mut(), STOP_HOTKEY, MOD_NOREPEAT, VK_F8 as u32) != 0;
        engine.set_hotkeys(f9, f8 && !keyboard.is_null() && !mouse.is_null());
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                if msg.wParam == RECORD_HOTKEY as usize {
                    let _ = engine.start_recording();
                }
                if msg.wParam == STOP_HOTKEY as usize {
                    let _ = engine.stop();
                }
            } else {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
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

fn own_window_focused() -> bool {
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
    if code >= 0 && !own_window_focused() {
        let input = &*(data as *const KBDLLHOOKSTRUCT);
        if input.dwExtraInfo != EXTRA_INFO
            && input.vkCode != VK_F8 as u32
            && input.vkCode != VK_F9 as u32
        {
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

unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
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
    unsafe {
        GetCursorPos(&mut point);
    }
    (point.x, point.y)
}

fn mouse_move(x: i32, y: i32) -> INPUT {
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

#[cfg(test)]
mod tests {
    use super::normalize_pixel;

    #[test]
    fn maps_negative_virtual_desktop_coordinates_to_absolute_input() {
        assert_eq!(normalize_pixel(-1920, -1920, 3840), 0);
        assert_eq!(normalize_pixel(1919, -1920, 3840), 65535);
        assert!(normalize_pixel(-1, -1920, 3840) < normalize_pixel(0, -1920, 3840));
    }
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
