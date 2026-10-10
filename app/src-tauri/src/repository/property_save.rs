//! Atomic metadata-only updates; the disk commit remains authoritative if cache publication fails.

use super::{
    digest, read_bounded, sorted_summaries, Entry, LibraryState, Repository, DIAGNOSTIC_FILE,
    DIAGNOSTIC_STORAGE, OUTSIDE_LIBRARY, RESTART_GUIDANCE,
};
use crate::macro_format::{
    self, fields, MacroDocument, PlaybackProperties, ValidationError, MAX_FILE_BYTES,
};
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Rejection for property writes while discovery/deletion or unavailable storage prevents mutation.
pub const SAVE_UNAVAILABLE: &str = "Property saving is unavailable";
/// Persistent warning used only when the file has already committed successfully.
pub const SAVED_REFRESH_FAILED: &str =
    "Saved, but the library could not refresh. Restart MacroLoom to reload the saved properties.";
const TEMP_PREFIX: &str = ".macroloom-properties";
const TEMP_EXTENSION: &str = "tmp";
const CONFLICT: &str = "Macro file changed";
const UNEDITABLE_METADATA: &str = "Playback extension metadata cannot be edited";
const TIMESTAMP_OVERFLOW: &str = "Cannot advance the update timestamp";
const TIMESTAMP_INCREMENT_MS: i64 = 1;
const DRAFT_RESTART_NOTICE: &str = "The open draft will be lost on restart.";
const TOO_LARGE: &str = "Updated macro exceeds the 16 MiB limit";
const INITIAL_SAVE_SEQUENCE: u64 = 0;
const SAVE_SEQUENCE_INCREMENT: u64 = 1;
const LIBRARY_REVISION_INCREMENT: u64 = 1;
static NEXT_SAVE: AtomicU64 = AtomicU64::new(INITIAL_SAVE_SEQUENCE);

/// Commit receipt distinguishes a no-op, a committed update, and cache trouble after commit.
pub struct PropertySave {
    pub changed: bool,
    pub name: String,
    pub library: Option<LibraryState>,
    pub warning: String,
}

/// Validated serialized replacement and its precomputed cache entry; all fallible preparation precedes disk commit.
struct PreparedSave {
    bytes: Vec<u8>,
    entry: Entry,
}
impl PreparedSave {
    /// Applies only editable metadata, advances the timestamp, and checks the fully serialized document before staging.
    fn new(
        mut document: MacroDocument,
        name: &str,
        playback: &PlaybackProperties,
        path: PathBuf,
    ) -> Result<Self, ValidationError> {
        document.name = name.into();
        update_playback(&mut document.playback, playback);
        document.updated_at = next_timestamp(&document.updated_at)?;
        let bytes = serde_json::to_vec_pretty(&document).map_err(storage_error)?;
        check_size(&bytes)?;
        let validated = macro_format::decode(&bytes)?;
        let entry = Entry::from_document(path, validated, digest(&bytes))?;
        Ok(Self { bytes, entry })
    }
    /// Stages and flushes a replacement, then rechecks external changes immediately before atomic replacement.
    fn commit(self, directory: &Path, fingerprint: &[u8]) -> Result<Entry, ValidationError> {
        let staged = StagedFile::write(directory, &self.bytes)?;
        unchanged_bytes(&self.entry.path, fingerprint)?;
        staged.commit(&self.entry.path)?;
        Ok(self.entry)
    }
}

/// Owns only a uniquely created staging file; cleanup never touches the prior committed macro.
struct StagedFile(PathBuf);
impl StagedFile {
    /// Writes and flushes bytes beside the destination; failure removes the partial staging file.
    fn write(directory: &Path, bytes: &[u8]) -> Result<Self, ValidationError> {
        let sequence = NEXT_SAVE.fetch_add(SAVE_SEQUENCE_INCREMENT, Ordering::Relaxed);
        let path = directory.join(format!(
            "{TEMP_PREFIX}-{}-{sequence}.{TEMP_EXTENSION}",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(storage_error)?;
        let staged = Self(path);
        file.write_all(bytes).map_err(storage_error)?;
        file.sync_all().map_err(storage_error)?;
        Ok(staged)
    }
    /// Atomically replaces the destination on the same filesystem; failed rename preserves the prior file.
    fn commit(&self, destination: &Path) -> Result<(), ValidationError> {
        fs::rename(&self.0, destination).map_err(storage_error)
    }
}
impl Drop for StagedFile {
    /// Best-effort removes the owned non-JSON temporary path after success or any precommit failure.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl Repository {
    /// Validates the complete external property draft, checks the cached file digest, and atomically commits on a worker.
    /// ID, filename, recording, events and unknown metadata remain unchanged. Equal normalized properties perform no write.
    /// Errors mean no commit occurred; cache failures after commit return a successful receipt with a warning instead.
    pub fn save_properties(
        &self,
        id: &str,
        name: &str,
        playback: &PlaybackProperties,
    ) -> Result<PropertySave, ValidationError> {
        let name = name.trim();
        validate_properties(name, playback)?;
        let (path, fingerprint) = self.save_association(id)?;
        let bytes = unchanged_bytes(&path, &fingerprint)?;
        let document = macro_format::decode(&bytes)?;
        if same_properties(&document.name, &document.playback, name, playback) {
            return Ok(self.save_receipt(false, name, self.snapshot()));
        }
        let prepared = PreparedSave::new(document, name, playback, path)?;
        let entry = prepared.commit(&self.directory, &fingerprint)?;
        Ok(self.save_receipt(true, name, self.install_saved(id, entry)))
    }
    /// Captures only a loaded writable association; no untrusted path or concurrent deletion can reach disk.
    fn save_association(&self, id: &str) -> Result<(PathBuf, Vec<u8>), ValidationError> {
        let state = self.snapshot().map_err(storage_error)?;
        if state.loading || !state.writable || state.deleting.is_some() {
            return Err(storage_error(SAVE_UNAVAILABLE));
        }
        let (path, fingerprint) = self.association(id).map_err(storage_error)?;
        if path.parent() != Some(self.directory.as_path()) {
            return Err(storage_error(OUTSIDE_LIBRARY));
        }
        Ok((path, fingerprint))
    }
    /// Installs metadata prepared before commit without another fallible disk read; poison preserves the commit receipt.
    fn install_saved(&self, id: &str, entry: Entry) -> Result<LibraryState, String> {
        let mut inner = self.lock()?;
        inner.entries.insert(id.into(), entry);
        inner.state.macros = sorted_summaries(&inner.entries);
        inner.state.revision += LIBRARY_REVISION_INCREMENT;
        Ok(inner.state.clone())
    }
    /// Produces success independently of cache availability; never translates a committed write into an ordinary failure.
    fn save_receipt(
        &self,
        changed: bool,
        name: &str,
        library: Result<LibraryState, String>,
    ) -> PropertySave {
        let warning = if library.is_err() {
            SAVED_REFRESH_FAILED.into()
        } else {
            String::new()
        };
        PropertySave {
            changed,
            name: name.into(),
            library: library.ok(),
            warning,
        }
    }
}

/// Maps fallible I/O/serialization/precondition failures to a persistent storage diagnostic.
fn storage_error(error: impl std::fmt::Display) -> ValidationError {
    ValidationError::new(DIAGNOSTIC_STORAGE, error.to_string())
}
/// Validates every editable setting regardless of active repeat mode; frontend extension injection is rejected.
fn validate_properties(name: &str, playback: &PlaybackProperties) -> Result<(), ValidationError> {
    macro_format::validate_name(name)?;
    macro_format::validate_playback(playback)?;
    if !playback.extra.is_empty() {
        return Err(ValidationError::new(fields::PLAYBACK, UNEDITABLE_METADATA));
    }
    Ok(())
}
/// Returns the same bytes compared and decoded; missing or changed files reject no-op and changed saves alike.
fn unchanged_bytes(path: &Path, fingerprint: &[u8]) -> Result<Vec<u8>, ValidationError> {
    let bytes = read_bounded(path).map_err(|error| {
        ValidationError::new(
            error.field,
            format!(
                "{}. {RESTART_GUIDANCE} {DRAFT_RESTART_NOTICE}",
                error.message
            ),
        )
    })?;
    if digest(&bytes) != fingerprint {
        return Err(ValidationError::new(
            DIAGNOSTIC_FILE,
            format!("{CONFLICT}. {RESTART_GUIDANCE} {DRAFT_RESTART_NOTICE}"),
        ));
    }
    Ok(bytes)
}
/// Compares all saved settings including valid inactive repeat values, ignoring independent extension metadata.
fn same_properties(
    old_name: &str,
    old: &PlaybackProperties,
    name: &str,
    new: &PlaybackProperties,
) -> bool {
    old_name == name
        && old.speed == new.speed
        && std::mem::discriminant(&old.repeat_mode) == std::mem::discriminant(&new.repeat_mode)
        && old.total_runs == new.total_runs
        && old.interval_ms == new.interval_ms
}
/// Replaces editable values while retaining all unknown playback metadata from the validated file.
fn update_playback(old: &mut PlaybackProperties, new: &PlaybackProperties) {
    old.speed = new.speed;
    old.repeat_mode = new.repeat_mode.clone();
    old.total_runs = new.total_runs;
    old.interval_ms = new.interval_ms;
}
/// Advances update time by at least one millisecond even under clock rollback or a future stored timestamp.
fn next_timestamp(previous: &str) -> Result<String, ValidationError> {
    let previous = DateTime::parse_from_rfc3339(previous)
        .map_err(storage_error)?
        .with_timezone(&Utc);
    let minimum = previous
        .checked_add_signed(TimeDelta::milliseconds(TIMESTAMP_INCREMENT_MS))
        .ok_or_else(|| ValidationError::new(fields::UPDATED_AT, TIMESTAMP_OVERFLOW))?;
    let now: DateTime<Utc> = SystemTime::now().into();
    Ok(now
        .max(minimum)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}
/// Rejects oversized reserialization before creating a staging file or touching the original.
fn check_size(bytes: &[u8]) -> Result<(), ValidationError> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ValidationError::new(DIAGNOSTIC_FILE, TOO_LARGE));
    }
    Ok(())
}
