//! Owned temporary capture data and playback-held input bookkeeping.
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const MOVE_SAMPLE: Duration = Duration::from_millis(8);
pub const WAIT_SLICE: Duration = Duration::from_millis(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Supported physical mouse buttons, with independently tracked held state.
pub enum MouseButton {
    Left,
    Right,
}
impl MouseButton {
    pub const ALL: [Self; 2] = [Self::Left, Self::Right];
    /// Returns the capture-state slot for this button.
    fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

#[derive(Clone)]
/// One observed input at a monotonic offset; equal offsets retain vector order.
pub struct Event {
    pub at: Duration,
    pub kind: EventKind,
}
#[derive(Clone)]
/// Native input data uses signed physical pixels and Windows scan-code metadata.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Input successfully pressed by this playback session, pending a successful release.
pub enum Held {
    Key(u16, u16, bool),
    Button(MouseButton),
}

#[derive(Clone, Default)]
/// Temporary recording and observed held states; replacing it requires a validated new start.
pub struct Capture {
    pub events: Vec<Event>,
    pub duration: Duration,
    pub drag_moves: usize,
    pub last_release: Option<(i32, i32)>,
    keys: HashMap<u16, (u16, bool)>,
    buttons: [bool; 2],
    last_move: Option<Duration>,
}
impl Capture {
    /// Appends observed input at `at`, coalescing free/closely spaced drag motion and preserving held transitions.
    pub fn append(&mut self, at: Duration, kind: EventKind) {
        if !self.observe(at, &kind) {
            return;
        }
        self.events.push(Event { at, kind });
    }
    /// Updates capture state; returns false for free motion or motion inside the sample interval.
    fn observe(&mut self, at: Duration, kind: &EventKind) -> bool {
        match kind {
            EventKind::Key {
                vk,
                scan,
                extended,
                down,
            } => {
                if *down {
                    self.keys.insert(*vk, (*scan, *extended));
                } else {
                    self.keys.remove(vk);
                }
            }
            EventKind::Mouse { button, down, x, y } => {
                self.buttons[button.index()] = *down;
                self.last_move = None;
                if !down {
                    self.last_release = Some((*x, *y));
                }
            }
            EventKind::Move { .. } => return self.movement(at),
        }
        true
    }
    /// Retains held-button movement at bounded density without creating free-cursor paths.
    fn movement(&mut self, at: Duration) -> bool {
        if !self.buttons.iter().any(|held| *held) {
            return false;
        }
        if self
            .last_move
            .is_some_and(|previous| at.saturating_sub(previous) < MOVE_SAMPLE)
        {
            return false;
        }
        self.last_move = Some(at);
        self.drag_moves += 1;
        true
    }
    /// Freezes duration and appends releases for recorded holds; never releases the user's physical input.
    pub fn finish(&mut self, duration: Duration, position: (i32, i32)) {
        self.duration = duration;
        for (vk, (scan, extended)) in self.keys.drain() {
            self.events.push(Event {
                at: duration,
                kind: EventKind::Key {
                    vk,
                    scan,
                    extended,
                    down: false,
                },
            });
        }
        for button in MouseButton::ALL {
            if self.buttons[button.index()] {
                self.events.push(Event {
                    at: duration,
                    kind: EventKind::Mouse {
                        button,
                        down: false,
                        x: position.0,
                        y: position.1,
                    },
                });
                self.last_release = Some(position);
            }
        }
        self.buttons = [false; 2];
    }
}
/// Updates successfully injected holds; a repeated press retains one entry and releases remove only matching ownership.
pub fn track_held(held: &mut Vec<Held>, kind: &EventKind) {
    match kind {
        EventKind::Key {
            vk,
            scan,
            extended,
            down,
        } => update_held(held, Held::Key(*vk, *scan, *extended), *down),
        EventKind::Mouse { button, down, .. } => update_held(held, Held::Button(*button), *down),
        EventKind::Move { .. } => {}
    }
}
/// Adds/removes a single held input after the corresponding injection succeeds.
fn update_held(held: &mut Vec<Held>, item: Held, down: bool) {
    if down {
        if !held.contains(&item) {
            held.push(item);
        }
    } else {
        held.retain(|value| *value != item);
    }
}
/// Waits to an absolute monotonic deadline; cancellation remains observable during long waits.
pub fn wait_until(deadline: Instant, cancel: &std::sync::atomic::AtomicBool) -> bool {
    use std::sync::atomic::Ordering;
    while !cancel.load(Ordering::Acquire) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return true;
        }
        std::thread::sleep(remaining.min(WAIT_SLICE));
    }
    false
}
