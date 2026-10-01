mod windows;

use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    phase: &'static str,
    event_count: usize,
    drag_move_count: usize,
    last_release: Option<(i32, i32)>,
    last_replay_release: Option<(i32, i32)>,
    duration_ms: u64,
    message: String,
    capture_gap_min_ms: Option<u64>,
    dispatch_lateness_p95_ms: Option<u64>,
    dispatch_lateness_max_ms: Option<u64>,
    stop_response_ms: Option<u64>,
    f9_available: bool,
    f8_available: bool,
}

#[derive(Clone)]
pub struct Event {
    pub at: Duration,
    pub kind: EventKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
}

impl MouseButton {
    const ALL: [Self; 2] = [Self::Left, Self::Right];

    fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

#[derive(Clone)]
pub enum EventKind {
    Key {
        vk: u16,
        scan: u16,
        extended: bool,
        down: bool,
    },
    Mouse {
        button: MouseButton,
        down: bool,
        x: i32,
        y: i32,
    },
    Move {
        x: i32,
        y: i32,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Recording,
    Playing,
}

struct Inner {
    phase: Phase,
    start: Option<Instant>,
    events: Vec<Event>,
    duration: Duration,
    message: String,
    held_buttons: [bool; 2],
    drag_move_count: usize,
    last_release: Option<(i32, i32)>,
    last_replay_release: Option<(i32, i32)>,
    held_keys: HashMap<u16, (u16, bool)>,
    last_move_at: Option<Instant>,
    last_capture_at: Option<Instant>,
    capture_gap_min: Option<Duration>,
    lateness: Vec<Duration>,
    stop_response: Option<Duration>,
    cancel: Option<Arc<AtomicBool>>,
    stop_requested: Option<Instant>,
    f9_available: bool,
    f8_available: bool,
}

pub struct Engine {
    app: AppHandle,
    inner: Mutex<Inner>,
    window: Mutex<windows::WindowState>,
}

impl Engine {
    fn new(app: AppHandle) -> Result<Arc<Self>, String> {
        let window = app
            .get_webview_window("main")
            .ok_or("Main window is missing")?;
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        Ok(Arc::new(Self {
            app,
            inner: Mutex::new(Inner {
                phase: Phase::Idle,
                start: None,
                events: Vec::new(),
                duration: Duration::ZERO,
                message: "Ready. F9 records in another application.".into(),
                held_buttons: [false; 2],
                drag_move_count: 0,
                last_release: None,
                last_replay_release: None,
                held_keys: HashMap::new(),
                last_move_at: None,
                last_capture_at: None,
                capture_gap_min: None,
                lateness: Vec::new(),
                stop_response: None,
                cancel: None,
                stop_requested: None,
                f9_available: false,
                f8_available: false,
            }),
            window: Mutex::new(windows::WindowState::new(hwnd)),
        }))
    }

    fn snapshot(&self) -> Snapshot {
        let inner = self.inner.lock().unwrap();
        let mut sorted = inner.lateness.clone();
        sorted.sort();
        let p95 = if sorted.is_empty() {
            None
        } else {
            Some(sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)].as_millis() as u64)
        };
        Snapshot {
            phase: match inner.phase {
                Phase::Idle => "idle",
                Phase::Recording => "recording",
                Phase::Playing => "playing",
            },
            event_count: inner.events.len(),
            drag_move_count: inner.drag_move_count,
            last_release: inner.last_release,
            last_replay_release: inner.last_replay_release,
            duration_ms: if inner.phase == Phase::Recording {
                inner.start.unwrap().elapsed().as_millis() as u64
            } else {
                inner.duration.as_millis() as u64
            },
            message: inner.message.clone(),
            capture_gap_min_ms: inner.capture_gap_min.map(|d| d.as_millis() as u64),
            dispatch_lateness_p95_ms: p95,
            dispatch_lateness_max_ms: sorted.last().map(|d| d.as_millis() as u64),
            stop_response_ms: inner.stop_response.map(|d| d.as_millis() as u64),
            f9_available: inner.f9_available,
            f8_available: inner.f8_available,
        }
    }

    fn publish(&self) {
        let _ = self.app.emit("prototype-state", self.snapshot());
    }

    fn set_hotkeys(&self, f9: bool, f8: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.f9_available = f9;
        inner.f8_available = f8;
        if !f8 {
            inner.message =
                "F8 Stop is unavailable. Close the application using it and restart MacroLoom."
                    .into();
        } else if !f9 {
            inner.message = "F9 is unavailable; use Record in this window.".into();
        }
        drop(inner);
        self.publish();
    }

    fn start_recording(&self) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.phase != Phase::Idle {
                return Err("A session is already active".into());
            }
            if !inner.f8_available {
                return Err("F8 Stop is unavailable".into());
            }
            inner.phase = Phase::Recording;
            inner.start = Some(Instant::now());
            inner.events.clear();
            inner.duration = Duration::ZERO;
            inner.held_buttons = [false; 2];
            inner.drag_move_count = 0;
            inner.last_release = None;
            inner.last_replay_release = None;
            inner.held_keys.clear();
            inner.last_move_at = None;
            inner.last_capture_at = None;
            inner.capture_gap_min = None;
            inner.lateness.clear();
            inner.stop_response = None;
            inner.message = "Recording. Click the target field before typing. F8 stops.".into();
        }
        if let Err(error) = self.window.lock().unwrap().compact(true) {
            self.inner.lock().unwrap().phase = Phase::Idle;
            return Err(error);
        }
        self.publish();
        Ok(())
    }

    fn capture(&self, kind: EventKind) {
        let mut inner = self.inner.lock().unwrap();
        if inner.phase != Phase::Recording {
            return;
        }
        let now = Instant::now();
        let at = now.duration_since(inner.start.unwrap());
        if let Some(previous) = inner.last_capture_at {
            let gap = now.duration_since(previous);
            inner.capture_gap_min = Some(inner.capture_gap_min.map_or(gap, |old| old.min(gap)));
        }
        inner.last_capture_at = Some(now);
        match &kind {
            EventKind::Key {
                vk,
                scan,
                extended,
                down,
            } => {
                if *down {
                    inner.held_keys.insert(*vk, (*scan, *extended));
                } else {
                    inner.held_keys.remove(vk);
                }
            }
            EventKind::Mouse { button, down, .. } => {
                inner.held_buttons[button.index()] = *down;
                if *down {
                    inner.last_move_at = None;
                }
                if let EventKind::Mouse {
                    down: false, x, y, ..
                } = &kind
                {
                    inner.last_release = Some((*x, *y));
                }
            }
            EventKind::Move { .. } => {
                if !inner.held_buttons.iter().any(|held| *held) {
                    return;
                }
                if inner
                    .last_move_at
                    .is_some_and(|t| now.duration_since(t) < Duration::from_millis(8))
                {
                    return;
                }
                inner.last_move_at = Some(now);
                inner.drag_move_count += 1;
            }
        }
        inner.events.push(Event { at, kind });
    }

    fn stop(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        match inner.phase {
            Phase::Idle => return Ok(()),
            Phase::Playing => {
                inner.stop_requested = Some(Instant::now());
                if let Some(cancel) = &inner.cancel {
                    cancel.store(true, Ordering::Release);
                }
                inner.message = "Stopping playback…".into();
                drop(inner);
                self.publish();
                return Ok(());
            }
            Phase::Recording => {}
        }
        let at = inner.start.unwrap().elapsed();
        inner.duration = at;
        let (x, y) = windows::cursor_position();
        for (vk, (scan, extended)) in inner.held_keys.clone() {
            inner.events.push(Event {
                at,
                kind: EventKind::Key {
                    vk,
                    scan,
                    extended,
                    down: false,
                },
            });
        }
        for button in MouseButton::ALL {
            if inner.held_buttons[button.index()] {
                inner.events.push(Event {
                    at,
                    kind: EventKind::Mouse {
                        button,
                        down: false,
                        x,
                        y,
                    },
                });
                inner.last_release = Some((x, y));
            }
        }
        inner.phase = Phase::Idle;
        inner.start = None;
        inner.message = if inner.events.is_empty() {
            "No input captured. Try F9 while the target has focus.".into()
        } else {
            "Recording in memory. Put the target back at its original position, then Play.".into()
        };
        drop(inner);
        self.window.lock().unwrap().compact(false)?;
        self.publish();
        Ok(())
    }

    fn play(self: &Arc<Self>) -> Result<(), String> {
        let (events, duration, cancel) = {
            let mut inner = self.inner.lock().unwrap();
            if inner.phase != Phase::Idle {
                return Err("A session is already active".into());
            }
            if !inner.f8_available {
                return Err("F8 Stop is unavailable".into());
            }
            if inner.events.is_empty() {
                return Err("Record some input first".into());
            }
            let cancel = Arc::new(AtomicBool::new(false));
            inner.phase = Phase::Playing;
            inner.cancel = Some(cancel.clone());
            inner.stop_requested = None;
            inner.lateness.clear();
            inner.message = "Playing. F8 cancels.".into();
            (inner.events.clone(), inner.duration, cancel)
        };
        if let Err(error) = self.window.lock().unwrap().compact(true) {
            let mut inner = self.inner.lock().unwrap();
            inner.phase = Phase::Idle;
            inner.cancel = None;
            return Err(error);
        }
        self.publish();
        let engine = self.clone();
        std::thread::spawn(move || {
            let start = Instant::now();
            let mut held = Vec::new();
            let mut lateness = Vec::new();
            let mut error = None;
            let mut last_replay_release = None;
            for event in &events {
                if !wait_until(start + event.at, &cancel) {
                    break;
                }
                lateness.push(Instant::now().saturating_duration_since(start + event.at));
                if let Err(e) = windows::send_event(&event.kind) {
                    error = Some(e);
                    break;
                }
                if let EventKind::Mouse { down: false, .. } = &event.kind {
                    last_replay_release = Some(windows::cursor_position());
                }
                match &event.kind {
                    EventKind::Key {
                        vk,
                        scan,
                        extended,
                        down,
                    } => update_held(&mut held, Held::Key(*vk, *scan, *extended), *down),
                    EventKind::Mouse { button, down, .. } => {
                        update_held(&mut held, Held::Button(*button), *down)
                    }
                    EventKind::Move { .. } => {}
                }
            }
            if !cancel.load(Ordering::Acquire) && error.is_none() {
                let _ = wait_until(start + duration, &cancel);
            }
            for item in held.into_iter().rev() {
                let _ = windows::release(item);
            }
            let mut inner = engine.inner.lock().unwrap();
            inner.stop_response = inner.stop_requested.map(|time| time.elapsed());
            inner.phase = Phase::Idle;
            inner.cancel = None;
            inner.lateness = lateness;
            inner.last_replay_release = last_replay_release;
            inner.message = match error {
                Some(e) => format!("Playback failed: {e}"),
                None if cancel.load(Ordering::Acquire) => {
                    "Playback cancelled and held input released.".into()
                }
                None => "Playback complete.".into(),
            };
            drop(inner);
            if let Err(e) = engine.window.lock().unwrap().compact(false) {
                engine.inner.lock().unwrap().message = format!("Could not restore window: {e}");
            }
            engine.publish();
        });
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Key(u16, u16, bool),
    Button(MouseButton),
}

fn update_held(held: &mut Vec<Held>, item: Held, down: bool) {
    if down {
        if !held.contains(&item) {
            held.push(item);
        }
    } else {
        held.retain(|value| *value != item);
    }
}

fn wait_until(deadline: Instant, cancel: &AtomicBool) -> bool {
    while !cancel.load(Ordering::Acquire) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return true;
        }
        std::thread::sleep(remaining.min(Duration::from_millis(2)));
    }
    false
}

#[tauri::command]
fn snapshot(engine: State<'_, Arc<Engine>>) -> Snapshot {
    engine.snapshot()
}

#[tauri::command]
fn start_recording(engine: State<'_, Arc<Engine>>) -> Result<Snapshot, String> {
    engine.start_recording()?;
    Ok(engine.snapshot())
}

#[tauri::command]
fn stop(engine: State<'_, Arc<Engine>>) -> Result<Snapshot, String> {
    engine.stop()?;
    Ok(engine.snapshot())
}

#[tauri::command]
fn play(engine: State<'_, Arc<Engine>>) -> Result<Snapshot, String> {
    engine.play()?;
    Ok(engine.snapshot())
}

pub fn run() {
    windows::enable_physical_dpi();
    tauri::Builder::default()
        .setup(|app| {
            let engine = Engine::new(app.handle().clone())?;
            windows::start_hooks(engine.clone());
            app.manage(engine);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            start_recording,
            stop,
            play
        ])
        .run(tauri::generate_context!())
        .expect("failed to run MacroLoom prototype");
}
