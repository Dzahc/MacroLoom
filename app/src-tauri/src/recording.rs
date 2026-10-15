//! Frozen recording identity, bounded serialization and conversion into the versioned format.
use crate::input::{Capture, Event, EventKind, MouseButton};
use crate::macro_format::{
    self, Button, EventData, MacroDocument, NativeKey, PlaybackProperties, RecordedEvent,
    RecordingEnvironment, RepeatMode,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use std::time::{Duration, SystemTime};

pub const SAVING: &str = "Saving…";
pub const LIMIT_REACHED: &str = "Recording stopped at the storage limit.";
pub const ENVIRONMENT_CHANGED: &str =
    "Recording stopped because the keyboard layout or displays changed.";
pub const CAPTURE_INTERRUPTED: &str = "Recording was interrupted.";
pub const SAVE_FAILED: &str = "Could not save recording. Retry or explicitly Discard.";
pub const EMPTY_RECORDING: &str = "No input recorded";
const PLATFORM: &str = "windows";
const COORDINATES: &str = "screen_physical_pixels";
const DEFAULT_SPEED: f64 = 1.0;
const DEFAULT_RUNS: u64 = 1;
const NAME_PREFIX: &str = "Macro";
const NAME_TIME_FORMAT: &str = "%Y-%m-%d %H-%M-%S";
// Reserve space for the final duration, an interrupt reason and collision suffixes.
const DOCUMENT_HEADROOM: usize = 1024;
const EVENT_SEPARATOR_BYTES: usize = 1;

#[derive(Clone)]
/// Backend-owned draft; ID/time/events survive every failed save and explicit Retry.
pub struct Recording {
    pub document: MacroDocument,
    pub filename: Option<String>,
    pub reason: String,
    pub bytes: usize,
}
impl Recording {
    /// Allocates one identity and creation time after storage preflight; does not write to disk.
    pub fn new(environment: RecordingEnvironment) -> Result<Self, String> {
        let now: DateTime<Utc> = SystemTime::now().into();
        let timestamp = now.to_rfc3339_opts(SecondsFormat::Millis, true);
        let document = MacroDocument {
            schema_version: macro_format::SCHEMA_VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            name: format!("{NAME_PREFIX} {}", now.format(NAME_TIME_FORMAT)),
            created_at: timestamp.clone(),
            updated_at: timestamp,
            recording: environment,
            duration_ms: 0,
            playback: PlaybackProperties {
                speed: DEFAULT_SPEED,
                repeat_mode: RepeatMode::Once,
                total_runs: DEFAULT_RUNS,
                interval_ms: 0,
                extra: Default::default(),
            },
            events: Vec::new(),
            extra: Default::default(),
        };
        let bytes = serde_json::to_vec(&document)
            .map_err(|error| error.to_string())?
            .len()
            + DOCUMENT_HEADROOM;
        if bytes as u64 > macro_format::MAX_FILE_BYTES {
            return Err(LIMIT_REACHED.into());
        }
        Ok(Self {
            document,
            filename: None,
            reason: String::new(),
            bytes,
        })
    }
    /// Accounts for one observed serialized event before accepting it; rejection leaves the valid prefix intact.
    pub fn accept(&mut self, at: Duration, kind: &EventKind, count: usize) -> bool {
        if count >= macro_format::MAX_EVENTS {
            return false;
        }
        let Ok(bytes) = serde_json::to_vec(&saved_event(at, kind)) else {
            return false;
        };
        let size = self.bytes + bytes.len() + EVENT_SEPARATOR_BYTES;
        if size as u64 > macro_format::MAX_FILE_BYTES {
            return false;
        }
        self.bytes = size;
        true
    }
    /// Freezes only observed events and duration; held-input cleanup belongs to playback.
    pub fn freeze(&mut self, capture: &Capture) {
        self.document.duration_ms = milliseconds(capture.duration);
        self.document.events = capture.events.iter().map(convert_event).collect();
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Durable outcome retained in snapshots so disconnects cannot turn a failed save into success.
pub struct RecordingOutcome {
    pub sequence: u64,
    pub id: String,
    pub name: String,
    pub success: bool,
    pub message: String,
    pub library: Option<crate::repository::LibraryState>,
}

/// Creates Windows physical-coordinate metadata without assuming the current display layout matches a saved macro.
pub fn environment(layout: String, displays: Vec<macro_format::Display>) -> RecordingEnvironment {
    RecordingEnvironment {
        platform: PLATFORM.into(),
        coordinate_space: COORDINATES.into(),
        keyboard_layout: layout,
        displays,
        extra: Default::default(),
    }
}
/// Floors monotonic offsets consistently; equal millisecond times retain vector order.
pub fn milliseconds(duration: Duration) -> u64 {
    duration.as_millis() as u64
}
/// Converts a retained event without changing its position in the recording.
fn convert_event(event: &Event) -> RecordedEvent {
    saved_event(event.at, &event.kind)
}
/// Serializes one native event with explicit native key data and signed physical positions.
pub fn saved_event(at: Duration, kind: &EventKind) -> RecordedEvent {
    let data = match *kind {
        EventKind::Key {
            vk,
            scan,
            extended,
            down,
        } => key_data(vk, scan, extended, down),
        EventKind::Mouse { button, down, x, y } => mouse_data(button, down, x, y),
        EventKind::Move { x, y } => EventData::MouseMove { x, y },
    };
    RecordedEvent {
        at_ms: milliseconds(at),
        data,
        extra: Default::default(),
    }
}
/// Keeps a stable Windows key label alongside the scan code used for native replay.
fn key_data(vk: u16, scan: u16, extended: bool, down: bool) -> EventData {
    let key = format!("VK_{vk:02X}");
    let native = NativeKey {
        scan_code: scan,
        virtual_key: vk,
        extended,
        extra: Default::default(),
    };
    if down {
        EventData::KeyDown { key, native }
    } else {
        EventData::KeyUp { key, native }
    }
}
/// Converts the supported button set without inventing a click or release.
fn mouse_data(button: MouseButton, down: bool, x: i32, y: i32) -> EventData {
    let button = match button {
        MouseButton::Left => Button::Left,
        MouseButton::Right => Button::Right,
    };
    if down {
        EventData::MouseDown { button, x, y }
    } else {
        EventData::MouseUp { button, x, y }
    }
}
#[cfg(test)]
#[path = "recording_test_support.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "recording_format_tests.rs"]
mod tests;
