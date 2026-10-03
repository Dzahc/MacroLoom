use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_EVENTS: usize = 100_000;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub const MAX_NAME_CHARS: usize = 120;
pub const SPEEDS: [f64; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];
const UUID_LENGTH: usize = 36;
const UUID_DASHES: [usize; 4] = [8, 13, 18, 23];
const MAX_VIRTUAL_KEY: u16 = 254;
const NANOS_PER_MILLISECOND: u32 = 1_000_000;
const MAX_TIMESTAMP_PRECISION: usize = 3;
const PLATFORM: &str = "windows";
const COORDINATE_SPACE: &str = "screen_physical_pixels";
const BUTTON_COUNT: usize = 2;
const LEFT_BUTTON_INDEX: usize = 0;
const RIGHT_BUTTON_INDEX: usize = 1;
const EVENT_COMMON_FIELDS: [&str; 2] = ["atMs", "type"];
const KEY_FIELDS: [&str; 2] = ["key", "native"];
const BUTTON_FIELDS: [&str; 3] = ["button", "x", "y"];
const MOVE_FIELDS: [&str; 2] = ["x", "y"];
const WHEEL_FIELDS: [&str; 4] = ["axis", "delta", "x", "y"];
const ROOT_FIELDS: [&str; 9] = [
    "schemaVersion",
    "id",
    "name",
    "createdAt",
    "updatedAt",
    "recording",
    "durationMs",
    "playback",
    "events",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Validated version-one document; events are retained only while an action owns this snapshot.
pub struct MacroDocument {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub recording: RecordingEnvironment,
    pub duration_ms: u64,
    pub playback: PlaybackProperties,
    pub events: Vec<RecordedEvent>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Playback settings copied into an action snapshot; native execution remains a separate boundary.
pub struct PlaybackProperties {
    pub speed: f64,
    pub repeat_mode: RepeatMode,
    pub total_runs: u64,
    pub interval_ms: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
/// Supported repeat modes, with fixed counts including the first run.
pub enum RepeatMode {
    Once,
    Fixed,
    Indefinite,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Recording metadata describes the original environment, independently of the current desktop.
pub struct RecordingEnvironment {
    pub platform: String,
    pub coordinate_space: String,
    pub keyboard_layout: String,
    pub displays: Vec<Display>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// One display uses signed physical origin coordinates and positive dimensions/scale.
pub struct Display {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Windows key data retained alongside the normalized label for later native playback.
pub struct NativeKey {
    pub scan_code: u16,
    pub virtual_key: u16,
    pub extended: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
/// Only left and right buttons are supported by version one.
pub enum Button {
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Ordered input data; deserialization never calls native input APIs.
pub enum EventData {
    KeyDown {
        key: String,
        native: NativeKey,
    },
    KeyUp {
        key: String,
        native: NativeKey,
    },
    MouseDown {
        button: Button,
        x: i32,
        y: i32,
    },
    MouseUp {
        button: Button,
        x: i32,
        y: i32,
    },
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseWheel {
        axis: WheelAxis,
        delta: i32,
        x: i32,
        y: i32,
    },
}

impl EventData {
    /// Returns fields consumed by this payload variant so extension metadata survives a later save.
    fn fields(&self) -> &'static [&'static str] {
        match self {
            Self::KeyDown { .. } | Self::KeyUp { .. } => &KEY_FIELDS,
            Self::MouseDown { .. } | Self::MouseUp { .. } => &BUTTON_FIELDS,
            Self::MouseMove { .. } => &MOVE_FIELDS,
            Self::MouseWheel { .. } => &WHEEL_FIELDS,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
/// Version one accepts vertical wheel events only.
pub enum WheelAxis {
    Vertical,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Millisecond offset and payload retain their array position for equal-time ordering.
pub struct RecordedEvent {
    pub at_ms: u64,
    #[serde(flatten)]
    pub data: EventData,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize)]
/// Actionable field/part diagnostic returned for malformed external data.
pub struct ValidationError {
    pub field: String,
    pub message: String,
}
impl ValidationError {
    /// Creates a diagnostic without modifying its source; field is a JSON-style path or file part.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Decodes external JSON into a document; failures identify the containing field and never execute data.
pub fn decode(bytes: &[u8]) -> Result<MacroDocument, ValidationError> {
    let root: Value = serde_json::from_slice(bytes)
        .map_err(|error| ValidationError::new("JSON", error.to_string()))?;
    let document = document_from_value(&root)?;
    validate(&document)?;
    Ok(document)
}

/// Reads typed root fields after checking the format version; unknown metadata fields are ignored.
fn document_from_value(root: &Value) -> Result<MacroDocument, ValidationError> {
    let schema_version: u32 = field(root, "schemaVersion")?;
    if schema_version != SCHEMA_VERSION {
        return Err(ValidationError::new(
            "schemaVersion",
            "unsupported schema version",
        ));
    }
    let (id, name) = identity(root)?;
    Ok(MacroDocument {
        schema_version,
        id,
        name,
        created_at: field(root, "createdAt")?,
        updated_at: field(root, "updatedAt")?,
        recording: field(root, "recording")?,
        duration_ms: field(root, "durationMs")?,
        playback: field(root, "playback")?,
        events: events_field(root)?,
        extra: extra_metadata(root),
    })
}

/// Reads stable identity and display name together, identifying either missing or mistyped field.
fn identity(root: &Value) -> Result<(String, String), ValidationError> {
    Ok((field(root, "id")?, field(root, "name")?))
}

/// Reads one required typed field; errors identify the root field and serde's nested part description.
fn field<T: serde::de::DeserializeOwned>(root: &Value, name: &str) -> Result<T, ValidationError> {
    let value = root
        .get(name)
        .ok_or_else(|| ValidationError::new(name, "required field is missing"))?;
    serde_json::from_value(value.clone())
        .map_err(|error| ValidationError::new(name, error.to_string()))
}

/// Retains unrecognized root metadata for later property writes without copying known event data.
fn extra_metadata(root: &Value) -> Map<String, Value> {
    root.as_object().map_or_else(Map::new, |object| {
        object
            .iter()
            .filter(|(key, _)| !ROOT_FIELDS.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    })
}

/// Bounds event count before constructing typed events and identifies malformed payloads by array index.
fn events_field(root: &Value) -> Result<Vec<RecordedEvent>, ValidationError> {
    let values = root
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| ValidationError::new("events", "required array is missing or invalid"))?;
    require(
        !values.is_empty() && values.len() <= MAX_EVENTS,
        "events",
        "must contain 1 to 100,000 events",
    )?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| decode_event(value, index))
        .collect()
}

/// Decodes one event and retains unrecognized fields without colliding with validated payload fields.
fn decode_event(value: &Value, index: usize) -> Result<RecordedEvent, ValidationError> {
    let mut event: RecordedEvent = serde_json::from_value(value.clone())
        .map_err(|error| ValidationError::new(format!("events[{index}]"), error.to_string()))?;
    let fields = event.data.fields();
    event.extra = value.as_object().map_or_else(Map::new, |object| {
        object
            .iter()
            .filter(|(key, _)| {
                !EVENT_COMMON_FIELDS.contains(&key.as_str()) && !fields.contains(&key.as_str())
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    });
    Ok(event)
}

/// Applies data-only semantic validation; each helper reports a precise field before any native use.
fn validate(document: &MacroDocument) -> Result<(), ValidationError> {
    require(
        valid_id(&document.id),
        "id",
        "must be a canonical lowercase UUID",
    )?;
    validate_name(&document.name)?;
    validate_dates(document)?;
    require(
        document.duration_ms <= MAX_SAFE_INTEGER,
        "durationMs",
        "exceeds the safe millisecond range",
    )?;
    validate_playback(&document.playback)?;
    validate_environment(&document.recording)?;
    validate_events(&document.events, document.duration_ms)
}

/// Checks canonical UUID syntax without constraining its generation version.
fn valid_id(id: &str) -> bool {
    if id.len() != UUID_LENGTH {
        return false;
    }
    id.bytes().enumerate().all(|(index, byte)| {
        if UUID_DASHES.contains(&index) {
            byte == b'-'
        } else {
            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
        }
    })
}

/// Enforces trimmed 1–120-character display names without controls, independently of filename syntax.
fn validate_name(name: &str) -> Result<(), ValidationError> {
    let length = name.chars().count();
    require(
        length > 0
            && length <= MAX_NAME_CHARS
            && name == name.trim()
            && !name.chars().any(char::is_control),
        "name",
        "must be trimmed, 1–120 characters, with no control characters",
    )
}

/// Requires valid UTC timestamps and chronological creation/update order; future dates remain data.
fn validate_dates(document: &MacroDocument) -> Result<(), ValidationError> {
    let created = timestamp(&document.created_at, "createdAt")?;
    let updated = timestamp(&document.updated_at, "updatedAt")?;
    require(
        created <= updated,
        "updatedAt",
        "must not precede createdAt",
    )
}

/// Parses UTC RFC 3339 with at most millisecond precision, returning the normalized instant for sorting.
pub(crate) fn timestamp(value: &str, field: &str) -> Result<i64, ValidationError> {
    let date = DateTime::<FixedOffset>::parse_from_rfc3339(value)
        .map_err(|error| ValidationError::new(field, error.to_string()))?;
    let precision = value.find('.').map_or(0, |position| {
        value[position + 1..]
            .chars()
            .take_while(char::is_ascii_digit)
            .count()
    });
    require(
        date.offset().local_minus_utc() == 0
            && precision <= MAX_TIMESTAMP_PRECISION
            && date.timestamp_subsec_nanos() % NANOS_PER_MILLISECOND == 0,
        field,
        "must be UTC RFC 3339 with at most millisecond precision",
    )?;
    Ok(date.timestamp_millis())
}

/// Validates presets, positive finite-run counts and integral millisecond intervals.
fn validate_playback(properties: &PlaybackProperties) -> Result<(), ValidationError> {
    require(
        SPEEDS.contains(&properties.speed),
        "playback.speed",
        "must be a supported speed preset",
    )?;
    require(
        properties.total_runs > 0 && properties.total_runs <= MAX_SAFE_INTEGER,
        "playback.totalRuns",
        "must be a positive safe integer",
    )?;
    require(
        properties.interval_ms <= MAX_SAFE_INTEGER,
        "playback.intervalMs",
        "exceeds the safe millisecond range",
    )
}

/// Validates recorded metadata structurally; current desktop compatibility belongs to playback.
fn validate_environment(environment: &RecordingEnvironment) -> Result<(), ValidationError> {
    require(
        environment.platform == PLATFORM,
        "recording.platform",
        "unsupported recording platform",
    )?;
    require(
        environment.coordinate_space == COORDINATE_SPACE,
        "recording.coordinateSpace",
        "must use physical screen pixels",
    )?;
    require(
        !environment.keyboard_layout.is_empty()
            && !environment.keyboard_layout.chars().any(char::is_control),
        "recording.keyboardLayout",
        "must contain keyboard layout metadata",
    )?;
    require(
        !environment.displays.is_empty(),
        "recording.displays",
        "must contain at least one display",
    )?;
    for (index, display) in environment.displays.iter().enumerate() {
        validate_display(display, index)?;
    }
    Ok(())
}

/// Checks positive display dimensions and finite scale while permitting negative origins.
fn validate_display(display: &Display, index: usize) -> Result<(), ValidationError> {
    let prefix = format!("recording.displays[{index}]");
    require(
        display.width > 0 && display.width <= i32::MAX as u32,
        &format!("{prefix}.width"),
        "must be a positive physical dimension",
    )?;
    require(
        display.height > 0 && display.height <= i32::MAX as u32,
        &format!("{prefix}.height"),
        "must be a positive physical dimension",
    )?;
    require(
        display.scale_factor.is_finite() && display.scale_factor > 0.0,
        &format!("{prefix}.scaleFactor"),
        "must be finite and positive",
    )
}

/// Preserves equal-time array order and checks drag press state; end holds and unmatched releases are legal.
fn validate_events(events: &[RecordedEvent], duration_ms: u64) -> Result<(), ValidationError> {
    let mut previous = 0;
    let mut held = [false; BUTTON_COUNT];
    for (index, event) in events.iter().enumerate() {
        require(
            event.at_ms >= previous && event.at_ms <= duration_ms,
            &format!("events[{index}].atMs"),
            "must be nondecreasing and within durationMs",
        )?;
        validate_event(&event.data, index, &mut held)?;
        previous = event.at_ms;
    }
    Ok(())
}

/// Validates payload semantics and tracks only supported recorded mouse presses; key repeats are legal.
fn validate_event(
    data: &EventData,
    index: usize,
    held: &mut [bool; BUTTON_COUNT],
) -> Result<(), ValidationError> {
    let prefix = format!("events[{index}]");
    match data {
        EventData::KeyDown { key, native } | EventData::KeyUp { key, native } => {
            validate_key(key, native, &prefix)?
        }
        EventData::MouseDown { button, .. } => held[button_index(*button)] = true,
        EventData::MouseUp { button, .. } => held[button_index(*button)] = false,
        EventData::MouseMove { .. } => require(
            held.iter().any(|pressed| *pressed),
            &format!("{prefix}.type"),
            "mouse_move requires a recorded held button",
        )?,
        EventData::MouseWheel { .. } => {}
    }
    Ok(())
}

/// Checks a normalized label and Windows virtual-key range; native scan-code width is enforced by serde.
fn validate_key(key: &str, native: &NativeKey, prefix: &str) -> Result<(), ValidationError> {
    require(
        !key.is_empty() && !key.chars().any(char::is_control),
        &format!("{prefix}.key"),
        "must contain a normalized key label",
    )?;
    require(
        native.virtual_key > 0 && native.virtual_key <= MAX_VIRTUAL_KEY,
        &format!("{prefix}.native.virtualKey"),
        "must be a supported Windows virtual-key value",
    )
}

/// Maps the two supported buttons to owned press-state slots.
fn button_index(button: Button) -> usize {
    match button {
        Button::Left => LEFT_BUTTON_INDEX,
        Button::Right => RIGHT_BUTTON_INDEX,
    }
}
/// Converts a failed invariant into an actionable diagnostic; successful checks leave data untouched.
fn require(condition: bool, field: &str, message: &str) -> Result<(), ValidationError> {
    if condition {
        Ok(())
    } else {
        Err(ValidationError::new(field, message))
    }
}
