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
    pub(crate) fn index(self) -> usize {
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
    Wheel {
        delta: i32,
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
    pending_move: bool,
}
impl Capture {
    /// Determines whether an event belongs to the recorded stream before bounded storage accounting.
    pub fn will_record(&self, kind: &EventKind) -> bool {
        match kind {
            EventKind::Move { .. } => self.buttons.iter().any(|held| *held),
            EventKind::Mouse {
                button,
                down: false,
                ..
            } => self.buttons[button.index()],
            EventKind::Key {
                vk, down: false, ..
            } => self.keys.contains_key(vk),
            _ => true,
        }
    }
    /// Returns a consecutive provisional tail eligible for replacement at `at`; pauses preserve the old endpoint.
    pub fn replacement(&self, at: Duration, kind: &EventKind) -> Option<&Event> {
        let tail = self.events.last()?;
        if !self.pending_move || at.saturating_sub(tail.at) >= MOVE_SAMPLE {
            return None;
        }
        if let EventKind::Move { .. } = kind {
            Some(tail)
        } else {
            None
        }
    }
    /// Retains input at its observed time, keeping a bounded provisional final movement between sampled points.
    pub fn append(&mut self, at: Duration, kind: EventKind) {
        if !self.will_record(&kind) {
            return;
        }
        if let EventKind::Move { .. } = kind {
            self.movement(at, kind);
            return;
        }
        // The provisional tail is already in order and budgeted; a transition makes it immutable.
        self.pending_move = false;
        self.last_move = None;
        self.observe(&kind);
        self.events.push(Event { at, kind });
    }
    /// Updates observed key/button ownership after acceptance; release locations remain the actual observed pixels.
    fn observe(&mut self, kind: &EventKind) {
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
                if !down {
                    self.last_release = Some((*x, *y));
                }
            }
            EventKind::Move { .. } | EventKind::Wheel { .. } => {}
        }
    }
    /// Replaces only a consecutive provisional movement, preserving the first point and latest original timestamp.
    fn movement(&mut self, at: Duration, kind: EventKind) {
        let sampled = self
            .last_move
            .is_none_or(|previous| at.saturating_sub(previous) >= MOVE_SAMPLE);
        if self.replacement(at, &kind).is_some() {
            self.events.pop();
        } else {
            self.drag_moves += 1;
        }
        self.events.push(Event { at, kind });
        self.pending_move = !sampled;
        if sampled {
            self.last_move = Some(at);
        }
    }
    /// Freezes duration without inventing releases or changing physical input; playback owns end-of-run cleanup.
    pub fn finish(&mut self, duration: Duration, _position: (i32, i32)) {
        self.duration = duration;
        self.pending_move = false;
        self.last_move = None;
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
        EventKind::Move { .. } | EventKind::Wheel { .. } => {}
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
