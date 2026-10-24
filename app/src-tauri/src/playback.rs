//! Saved-input conversion, environment compatibility and absolute-deadline playback at one-times speed.
use crate::input::{Capture, Event, EventKind, MouseButton};
use crate::macro_format::{Button, EventData, MacroDocument, NativeKey, RecordingEnvironment};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const ENVIRONMENT_CHANGED: &str =
    "Playback stopped because the keyboard layout or display layout changed. Restore the recorded environment and retry.";
pub const INPUT_HELD: &str = "Release held keys and mouse buttons, then start playback again.";
pub const COMPLETE: &str = "Playback complete.";
const DEADLINE_RANGE: &str = "The recording duration exceeds the monotonic clock range.";

/// Copies validated events verbatim into a playback stream, retaining ties, repeats and trailing duration.
/// Saved speed/repeat properties remain in the owned document; ML-17 executes once at one-times speed.
pub fn capture(document: &MacroDocument) -> Capture {
    let mut capture = Capture::default();
    capture.duration = Duration::from_millis(document.duration_ms);
    for event in &document.events {
        if let EventData::MouseMove { .. } = event.data {
            capture.drag_moves += 1;
        }
        capture.events.push(Event {
            at: Duration::from_millis(event.at_ms),
            kind: event_kind(&event.data),
        });
    }
    capture
}

/// Converts one validated v1 payload without capture-time filtering or movement coalescing.
fn event_kind(data: &EventData) -> EventKind {
    match data {
        EventData::KeyDown { native, .. } => key_event(native, true),
        EventData::KeyUp { native, .. } => key_event(native, false),
        EventData::MouseDown { button, x, y } => button_event(*button, *x, *y, true),
        EventData::MouseUp { button, x, y } => button_event(*button, *x, *y, false),
        EventData::MouseMove { x, y } => EventKind::Move { x: *x, y: *y },
        EventData::MouseWheel { delta, x, y, .. } => EventKind::Wheel {
            delta: *delta,
            x: *x,
            y: *y,
        },
    }
}

/// Copies validated Windows native key metadata into one exact down/up transition.
fn key_event(native: &NativeKey, down: bool) -> EventKind {
    EventKind::Key {
        vk: native.virtual_key,
        scan: native.scan_code,
        extended: native.extended,
        down,
    }
}

/// Copies a supported positioned button transition without synthesizing movement or release events.
fn button_event(button: Button, x: i32, y: i32, down: bool) -> EventKind {
    let button = match button {
        Button::Left => MouseButton::Left,
        Button::Right => MouseButton::Right,
    };
    EventKind::Mouse { button, down, x, y }
}

/// Compares semantic environment values, ignoring extension metadata and monitor enumeration order.
/// Display power alone is absent from this comparison; the same desktop can play with dark screens.
pub fn compatible(expected: &RecordingEnvironment, actual: &RecordingEnvironment) -> bool {
    expected.platform == actual.platform
        && expected.coordinate_space == actual.coordinate_space
        && expected.keyboard_layout == actual.keyboard_layout
        && display_geometry(expected) == display_geometry(actual)
}

/// Returns canonical signed monitor geometry and scale, excluding unknown preserved metadata.
fn display_geometry(environment: &RecordingEnvironment) -> Vec<(i32, i32, u32, u32, u64)> {
    let mut displays: Vec<_> = environment
        .displays
        .iter()
        .map(|display| {
            (
                display.x,
                display.y,
                display.width,
                display.height,
                display.scale_factor.to_bits(),
            )
        })
        .collect();
    displays.sort_unstable();
    displays
}

/// A monotonic run clock; deterministic checks substitute time while retaining the real scheduler.
pub trait Clock {
    /// Waits for a run-relative deadline; returns false when cancellation interrupts the wait.
    fn wait(&self, at: Duration, cancel: &AtomicBool) -> Result<bool, String>;
}

/// Production clock shares the exact epoch published in the session snapshot.
pub struct MonotonicClock(pub Instant);
impl Clock for MonotonicClock {
    /// Waits cancellably against the absolute epoch, so dispatch overhead never accumulates as extra delays.
    fn wait(&self, at: Duration, cancel: &AtomicBool) -> Result<bool, String> {
        let deadline = self.0.checked_add(at).ok_or(DEADLINE_RANGE)?;
        Ok(crate::input::wait_until(deadline, cancel))
    }
}

/// Dispatches each stored event once in order and preserves the trailing wait.
/// Cancellation prevents subsequent dispatch; injection errors return immediately for owner cleanup.
pub fn run(
    capture: &Capture,
    clock: &impl Clock,
    cancel: &AtomicBool,
    mut dispatch: impl FnMut(&EventKind) -> Result<(), String>,
) -> Result<(), String> {
    for event in &capture.events {
        if !clock.wait(event.at, cancel)? || cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        dispatch(&event.kind)?;
    }
    clock.wait(capture.duration, cancel)?;
    Ok(())
}

#[cfg(test)]
#[path = "playback_tests.rs"]
mod tests;
