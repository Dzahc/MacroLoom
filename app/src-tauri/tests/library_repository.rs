use macroloom_lib::macro_format::{MAX_EVENTS, MAX_FILE_BYTES, SCHEMA_VERSION};
use macroloom_lib::repository::Repository;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

const ID: &str = "68833c94-3d18-4fb2-8a59-0cc72382b616";
const FILE: &str = "Fill_form.json";
const FUTURE_SCHEMA: u32 = SCHEMA_VERSION + 1;
const INVALID_FILE: &str = "invalid.json";
const SECOND_ID: &str = "78833c94-3d18-4fb2-8a59-0cc72382b616";
const NEWER_ID: &str = "88833c94-3d18-4fb2-8a59-0cc72382b616";
const NEWER_DATE: &str = "2026-09-29T21:32:08Z";
const UNKNOWN_METADATA: &str = "preserve this";
const EVENT_METADATA_FIELD: &str = "note";
const BOUNDARY_EVENTS: &str = r#"[
    {"atMs":0,"type":"mouse_up","button":"right","x":-20,"y":100},
    {"atMs":0,"type":"key_down","key":"KeyA","native":{"scanCode":30,"virtualKey":65,"extended":false}},
    {"atMs":0,"type":"key_down","key":"KeyA","native":{"scanCode":30,"virtualKey":65,"extended":false}}
]"#;
const DOCUMENT_TEMPLATE: &str = r#"{
  "schemaVersion":__SCHEMA_VERSION__,"id":"68833c94-3d18-4fb2-8a59-0cc72382b616",
  "name":"Fill form","createdAt":"2026-09-28T21:32:08Z","updatedAt":"2026-09-28T21:32:08Z",
  "recording":{"platform":"windows","coordinateSpace":"screen_physical_pixels",
    "keyboardLayout":"00000409","displays":[{"x":-1920,"y":0,"width":1920,"height":1080,"scaleFactor":1.0}]},
  "durationMs":1600,"playback":{"speed":1.0,"repeatMode":"once","totalRuns":1,"intervalMs":0},
  "events":[{"atMs":100,"type":"mouse_down","button":"left","x":-20,"y":100},
    {"atMs":200,"type":"mouse_move","x":-10,"y":100}]
}"#;
const SCHEMA_PLACEHOLDER: &str = "__SCHEMA_VERSION__";
const TEST_DIRECTORY: &str = "macroloom-repository-tests";
static DOCUMENT: LazyLock<String> = LazyLock::new(
    // Builds the valid fixture with the supported source schema version.
    || DOCUMENT_TEMPLATE.replace(SCHEMA_PLACEHOLDER, &SCHEMA_VERSION.to_string()),
);
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

/// Owns an isolated test library; cleanup is restricted to its named temporary root.
struct LibraryFolder(PathBuf);
impl LibraryFolder {
    /// Creates a unique empty directory and returns its cleanup owner; setup failure fails the test.
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(TEST_DIRECTORY)
            .join(format!("{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path).expect("test directory must be writable");
        Self(path)
    }
    /// Writes fixture bytes under the owned library; disk failures fail the test.
    fn write(&self, name: &str, bytes: &str) {
        fs::write(self.0.join(name), bytes).expect("fixture must be writable");
    }
}
impl Drop for LibraryFolder {
    /// Removes only the owned directory after verifying it remains beneath the temporary test root.
    fn drop(&mut self) {
        assert!(self
            .0
            .starts_with(std::env::temp_dir().join(TEST_DIRECTORY)));
        fs::remove_dir_all(&self.0).expect("test directory cleanup must succeed");
    }
}

#[test]
/// Loads an arbitrarily named macro, independently reports malformed JSON, and ignores temporary saves.
fn mixed_library_preserves_files_and_publishes_valid_entry() {
    let folder = LibraryFolder::new();
    folder.write(FILE, DOCUMENT.as_str());
    folder.write("broken.json", "{");
    folder.write("pending.json.tmp", "{");
    let repository = Repository::new(folder.0.clone());
    let mut progress = Vec::new();
    repository.load(|snapshot| progress.push(snapshot));
    let snapshot = repository.snapshot().expect("state must remain available");
    assert_eq!(snapshot.macros.len(), 1);
    assert_eq!(snapshot.macros[0].id, ID);
    assert!(progress
        .iter()
        .any(|state| state.loading && !state.macros.is_empty()));
    assert_eq!(snapshot.failures.len(), 1);
    assert_eq!(snapshot.failures[0].file, "broken.json");
    assert_eq!(
        fs::read_to_string(folder.0.join("broken.json")).unwrap(),
        "{"
    );
    assert_eq!(repository.read(ID).unwrap().name, "Fill form");
}

#[test]
/// Unsupported data remains untouched while a valid neighbor stays available and errors name the field.
fn unsupported_schema_does_not_hide_neighbor() {
    let folder = LibraryFolder::new();
    folder.write(FILE, DOCUMENT.as_str());
    let mut unsupported: Value = serde_json::from_str(DOCUMENT.as_str()).unwrap();
    unsupported["schemaVersion"] = json!(FUTURE_SCHEMA);
    let bytes = unsupported.to_string();
    folder.write("future.json", &bytes);
    let repository = Repository::new(folder.0.clone());
    repository.load(|_| {});
    let state = repository.snapshot().unwrap();
    assert_eq!(state.macros.len(), 1);
    assert_eq!(state.failures[0].field, "schemaVersion");
    assert_eq!(
        fs::read_to_string(folder.0.join("future.json")).unwrap(),
        bytes
    );
}

#[test]
/// Checks semantic file validation through discovery, including time order, drag state and properties.
fn malformed_fields_are_reported_without_loading_or_modifying_the_file() {
    let cases = [
        ("/id", json!("not-an-id"), "id"),
        ("/name", json!("  "), "name"),
        ("/createdAt", json!("2026-02-30T00:00:00Z"), "createdAt"),
        ("/updatedAt", json!("2025-09-28T21:32:08Z"), "updatedAt"),
        ("/playback/speed", json!(3), "playback.speed"),
        ("/playback/totalRuns", json!(0), "playback.totalRuns"),
        (
            "/recording/displays/0/scaleFactor",
            json!(0),
            "recording.displays[0].scaleFactor",
        ),
        ("/events/0/atMs", json!(1700), "events[0].atMs"),
        ("/events/1/atMs", json!(0), "events[1].atMs"),
        ("/events/0/type", json!("mouse_move"), "events[0].type"),
        ("/events", json!([]), "events"),
    ];
    for (pointer, value, expected_field) in cases {
        let folder = LibraryFolder::new();
        let mut document: Value = serde_json::from_str(DOCUMENT.as_str()).unwrap();
        *document.pointer_mut(pointer).unwrap() = value;
        let bytes = document.to_string();
        folder.write(INVALID_FILE, &bytes);
        let repository = Repository::new(folder.0.clone());
        repository.load(|_| {});
        let state = repository.snapshot().unwrap();
        assert!(state.macros.is_empty(), "{pointer} must be rejected");
        assert_eq!(state.failures[0].field, expected_field);
        assert_eq!(
            fs::read_to_string(folder.0.join(INVALID_FILE)).unwrap(),
            bytes
        );
    }
}

#[test]
/// Resolves storage beside a supplied executable, creates it and preserves ordering after restart.
fn executable_relative_empty_library_is_created_and_writable() {
    let folder = LibraryFolder::new();
    let executable = folder.0.join("macroloom.exe");
    let repository = Repository::beside_executable(&executable).unwrap();
    repository.load(|_| {});
    let state = repository.snapshot().unwrap();
    assert!(!state.loading);
    assert!(state.writable);
    assert!(state.macros.is_empty());
    assert!(folder
        .0
        .join(macroloom_lib::repository::MACROS_DIRECTORY)
        .is_dir());
}

#[test]
/// A filename-independent ID association detects external edits/deletion without changing cached properties.
fn lazy_action_snapshot_rejects_changed_or_missing_file() {
    let folder = LibraryFolder::new();
    folder.write(FILE, DOCUMENT.as_str());
    let repository = Repository::new(folder.0.clone());
    repository.load(|_| {});
    assert_eq!(repository.read(ID).unwrap().events.len(), 2);
    folder.write(
        FILE,
        &DOCUMENT.as_str().replace("Fill form", "Edited externally"),
    );
    assert!(repository.read(ID).unwrap_err().contains("Restart"));
    assert_eq!(repository.snapshot().unwrap().macros[0].name, "Fill form");
    fs::remove_file(folder.0.join(FILE)).unwrap();
    assert!(repository.read(ID).unwrap_err().contains("Restart"));
}

#[test]
/// Retains the first valid duplicate, normalizes timestamp sorting and reloads identical committed rows.
fn deterministic_duplicates_and_restart_ordering() {
    let folder = LibraryFolder::new();
    folder.write("a.json", DOCUMENT.as_str());
    folder.write(
        "z.json",
        &DOCUMENT.as_str().replace("Fill form", "Duplicate"),
    );
    folder.write("tie.json", &DOCUMENT.as_str().replace(ID, SECOND_ID));
    folder.write(
        "newest.json",
        &DOCUMENT
            .as_str()
            .replace(ID, NEWER_ID)
            .replace("2026-09-28T21:32:08Z", NEWER_DATE),
    );
    let repository = Repository::new(folder.0.clone());
    repository.load(|_| {});
    let state = repository.snapshot().unwrap();
    let ids: Vec<_> = state
        .macros
        .iter()
        .map(|summary| summary.id.as_str())
        .collect();
    assert_eq!(ids, [NEWER_ID, ID, SECOND_ID]);
    assert_eq!(state.macros[1].name, "Fill form");
    assert_eq!(state.failures[0].file, "z.json");
    let restarted = Repository::new(folder.0.clone());
    restarted.load(|_| {});
    assert_eq!(
        serde_json::to_value(state.macros).unwrap(),
        serde_json::to_value(restarted.snapshot().unwrap().macros).unwrap()
    );
}

#[test]
/// Suggests sanitized, case-insensitive collision-free initial filenames without altering display metadata.
fn initial_filename_uses_display_name_and_avoids_collisions() {
    let folder = LibraryFolder::new();
    folder.write("Fill_form.json", DOCUMENT.as_str());
    folder.write("fill_FORM_2.json", DOCUMENT.as_str());
    let repository = Repository::new(folder.0.clone());
    assert_eq!(
        repository.available_filename("Fill form").unwrap(),
        "Fill_form_3.json"
    );
    assert_eq!(
        repository.available_filename("Report: weekly").unwrap(),
        "Report__weekly.json"
    );
    assert_eq!(repository.available_filename("CON").unwrap(), "_CON.json");
    assert_eq!(repository.available_filename("..").unwrap(), "macro.json");
}

#[test]
/// Successful internal property commits refresh metadata/digest while retaining the existing filename.
fn internal_save_refreshes_properties_and_lazy_snapshot_without_renaming() {
    let folder = LibraryFolder::new();
    folder.write(FILE, DOCUMENT.as_str());
    let repository = Repository::new(folder.0.clone());
    repository.load(|_| {});
    let updated = DOCUMENT.as_str().replace("Fill form", "Renamed macro");
    folder.write(FILE, &updated);
    repository.refresh_after_save(ID).unwrap();
    assert_eq!(
        repository.snapshot().unwrap().macros[0].name,
        "Renamed macro"
    );
    assert_eq!(repository.read(ID).unwrap().name, "Renamed macro");
    assert!(folder.0.join(FILE).exists());
    folder.write(FILE, "{");
    assert!(repository.refresh_after_save(ID).is_err());
    assert_eq!(
        repository.snapshot().unwrap().macros[0].name,
        "Renamed macro"
    );
}

#[test]
/// Enforces byte/event limits independently while retaining supported zero-time events and extra metadata.
fn bounded_files_and_boundary_events_are_validated() {
    let folder = LibraryFolder::new();
    let mut document: Value = serde_json::from_str(DOCUMENT.as_str()).unwrap();
    document["durationMs"] = json!(0);
    document["extensionMetadata"] = json!(UNKNOWN_METADATA);
    document["events"] = serde_json::from_str(BOUNDARY_EVENTS).unwrap();
    folder.write(FILE, &document.to_string());
    let repository = Repository::new(folder.0.clone());
    repository.load(|_| {});
    let loaded = repository.read(ID).unwrap();
    assert_eq!(loaded.duration_ms, 0);
    assert_eq!(loaded.events.len(), 3);
    assert_eq!(loaded.extra["extensionMetadata"], UNKNOWN_METADATA);
    let key = document["events"][1].clone();
    document["events"] = Value::Array(std::iter::repeat_n(key, MAX_EVENTS + 1).collect());
    folder.write(INVALID_FILE, &document.to_string());
    fs::write(
        folder.0.join("oversized.json"),
        std::iter::repeat_n(b' ', MAX_FILE_BYTES as usize + 1).collect::<Vec<_>>(),
    )
    .unwrap();
    let restarted = Repository::new(folder.0.clone());
    restarted.load(|_| {});
    let state = restarted.snapshot().unwrap();
    assert_eq!(state.macros.len(), 1);
    assert_eq!(state.failures.len(), 2);
    assert!(state
        .failures
        .iter()
        .any(|failure| failure.field == "events"));
    assert!(state.failures.iter().any(|failure| failure.field == "file"));
}

#[test]
/// An unavailable directory ends discovery with an actionable error and no silent storage fallback.
fn unavailable_directory_preserves_prior_file() {
    let folder = LibraryFolder::new();
    folder.write(FILE, DOCUMENT.as_str());
    let repository = Repository::new(folder.0.join(FILE));
    repository.load(|_| {});
    let state = repository.snapshot().unwrap();
    assert!(!state.loading);
    assert!(!state.writable);
    assert!(state.macros.is_empty());
    assert_eq!(state.failures[0].field, "directory");
    assert_eq!(
        fs::read_to_string(folder.0.join(FILE)).unwrap(),
        DOCUMENT.as_str()
    );
}

#[test]
/// A later property-save consumer can serialize prepared event metadata without erasing unknown fields.
fn prepared_document_preserves_unknown_event_metadata() {
    let folder = LibraryFolder::new();
    let mut document: Value = serde_json::from_str(DOCUMENT.as_str()).unwrap();
    document["events"][0][EVENT_METADATA_FIELD] = json!(UNKNOWN_METADATA);
    folder.write(FILE, &document.to_string());
    let repository = Repository::new(folder.0.clone());
    repository.load(|_| {});
    let prepared = serde_json::to_value(repository.read(ID).unwrap()).unwrap();
    assert_eq!(
        prepared["events"][0][EVENT_METADATA_FIELD],
        UNKNOWN_METADATA
    );
}

#[test]
/// Library summaries exclude unknown property metadata while lazy action documents retain it for saving.
fn cached_properties_do_not_retain_extension_metadata() {
    let folder = LibraryFolder::new();
    let mut document: Value = serde_json::from_str(DOCUMENT.as_str()).unwrap();
    document["playback"][EVENT_METADATA_FIELD] = json!(UNKNOWN_METADATA);
    folder.write(FILE, &document.to_string());
    let repository = Repository::new(folder.0.clone());
    repository.load(|_| {});
    let summaries = serde_json::to_value(repository.snapshot().unwrap().macros).unwrap();
    assert!(summaries[0]["playback"].get(EVENT_METADATA_FIELD).is_none());
    let prepared = serde_json::to_value(repository.read(ID).unwrap()).unwrap();
    assert_eq!(prepared["playback"][EVENT_METADATA_FIELD], UNKNOWN_METADATA);
}
