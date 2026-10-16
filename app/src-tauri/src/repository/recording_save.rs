//! Non-clobbering recording commits and repeatable storage preflight.
use super::{digest, probe_writable, sorted_summaries, Entry, LibraryState, Repository};
use crate::{macro_format, recording::Recording};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

const STORAGE_GUIDANCE: &str = "Cannot write the macros folder. Move MacroLoom to a writable folder or restore its permissions, then retry.";
const COMMIT_ATTEMPTS: usize = 128;
const FIRST_SUFFIX: usize = 2;
const NAME_COLLISION: &str = "Could not reserve a unique recording filename; retry.";
const REFRESH_WARNING: &str =
    "Saved, but the library could not refresh. Restart MacroLoom to reload it.";

/// A successful disk commit remains successful even if cache publication becomes unavailable.
pub struct RecordingCommit {
    pub library: Option<LibraryState>,
    pub warning: String,
}
/// Owns only its unique temporary file; errors never remove a committed recording.
struct StagedRecording(PathBuf);
impl Drop for StagedRecording {
    /// Removes leftover staging data after a failed write or a successful atomic move.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
impl Repository {
    /// Probes actual write/delete access before every recording, allowing repaired storage to recover without restart.
    pub fn recording_preflight(&self) -> Result<(), String> {
        let state = self.snapshot()?;
        if state.loading || state.deleting.is_some() {
            return Err(super::DELETE_UNAVAILABLE.into());
        }
        let result =
            fs::create_dir_all(&self.directory).and_then(|()| probe_writable(&self.directory));
        self.lock()?.state.writable = result.is_ok();
        result.map_err(|error| format!("{STORAGE_GUIDANCE} {}: {error}", self.directory.display()))
    }
    /// Commits one frozen macro without replacing any existing path; retries retain identity and creation time.
    pub fn save_recording(&self, recording: &mut Recording) -> Result<RecordingCommit, String> {
        self.save_recording_using(recording, crate::windows::commit_new_recording)
    }
    /// Runs the same commit protocol with a filesystem move boundary, enabling deterministic mid-save failure evidence.
    fn save_recording_using(
        &self,
        recording: &mut Recording,
        commit: impl Fn(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<RecordingCommit, String> {
        self.recording_preflight()?;
        self.recording_name(recording)?;
        let bytes = serde_json::to_vec(&recording.document).map_err(|error| error.to_string())?;
        let document = macro_format::decode(&bytes).map_err(validation_message)?;
        let path = self.commit_recording(recording, &bytes, commit)?;
        // Decode/cache preparation cannot fail after commit: decode above validated these exact bytes.
        let entry = Entry::from_document(path, document, digest(&bytes));
        let library = entry
            .map_err(validation_message)
            .and_then(|entry| self.install_recording(entry));
        Ok(RecordingCommit {
            warning: if library.is_err() {
                REFRESH_WARNING.into()
            } else {
                String::new()
            },
            library: library.ok(),
        })
    }
    /// Reserves a unique generated display name independently of its filename, preserving it through Retry.
    fn recording_name(&self, recording: &mut Recording) -> Result<(), String> {
        if recording.filename.is_some() {
            return Ok(());
        }
        let state = self.snapshot()?;
        let original = recording.document.name.clone();
        let mut suffix = FIRST_SUFFIX;
        while state
            .macros
            .iter()
            .any(|item| item.name.eq_ignore_ascii_case(&recording.document.name))
        {
            recording.document.name = format!("{original}_{suffix}");
            suffix += 1;
        }
        recording.filename = Some(self.available_filename(&recording.document.name)?);
        Ok(())
    }
    /// Installs committed metadata and its digest without another disk read; library entries appear only after commit.
    fn install_recording(&self, entry: Entry) -> Result<LibraryState, String> {
        let mut inner = self.lock()?;
        inner.entries.insert(entry.summary.id.clone(), entry);
        inner.state.macros = sorted_summaries(&inner.entries);
        inner.state.revision += 1;
        Ok(inner.state.clone())
    }
    /// Stages and flushes complete bytes once; atomic move collisions choose another path, never overwrite.
    fn commit_recording(
        &self,
        recording: &mut Recording,
        bytes: &[u8],
        commit: impl Fn(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<PathBuf, String> {
        let staged = self.stage_recording(bytes)?;
        for _ in 0..COMMIT_ATTEMPTS {
            let filename = recording.filename.as_ref().ok_or(NAME_COLLISION)?;
            let destination = self.directory.join(filename);
            // Idempotent recovery also handles a commit acknowledged only after a lost response.
            if fs::read(&destination).is_ok_and(|existing| existing == bytes) {
                return Ok(destination);
            }
            match commit(&staged.0, &destination) {
                Ok(()) => return Ok(destination),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    recording.filename = Some(self.available_filename(&recording.document.name)?);
                }
                Err(error) => return Err(format!("{STORAGE_GUIDANCE} {error}")),
            }
        }
        Err(NAME_COLLISION.into())
    }
    /// Creates an owned non-JSON staging path and syncs its bytes before the destination becomes visible.
    fn stage_recording(&self, bytes: &[u8]) -> Result<StagedRecording, String> {
        let path = self
            .directory
            .join(format!(".macroloom-recording-{}.tmp", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        let staged = StagedRecording(path);
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        Ok(staged)
    }
}
/// Preserves the failing field in recording validation errors without adding a global error-formatting policy.
fn validation_message(error: macro_format::ValidationError) -> String {
    format!("{}: {}", error.field, error.message)
}

#[cfg(test)]
#[path = "recording_save_tests.rs"]
mod tests;
