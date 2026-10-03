use crate::macro_format::{
    self, MacroDocument, PlaybackProperties, ValidationError, MAX_FILE_BYTES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

pub const MACROS_DIRECTORY: &str = "macros";
/// Diagnostic part for directory creation or enumeration failures.
pub const DIAGNOSTIC_DIRECTORY: &str = "directory";
/// Diagnostic part for directory write-access failures.
pub const DIAGNOSTIC_STORAGE: &str = "storage";
/// Diagnostic part for file read and size-limit failures.
pub const DIAGNOSTIC_FILE: &str = "file";
const JSON_EXTENSION: &str = "json";
const RESTART_GUIDANCE: &str = "Restart MacroLoom to reload external changes.";
const LOCK_FAILURE: &str = "Library state is unavailable; restart MacroLoom.";
const INVALID_FILENAME_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const DEVICE_NAMES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
const DEVICE_DIGITS: [char; 12] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '¹', '²', '³'];
const DEFAULT_STEM: &str = "macro";
const FIRST_COLLISION_SUFFIX: u64 = 2;
static NEXT_PROBE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Small library snapshot; event arrays remain on disk until an action loads them.
pub struct MacroSummary {
    pub id: String,
    pub name: String,
    pub duration_ms: u64,
    pub created_at: String,
    pub playback: PlaybackProperties,
}

#[derive(Clone, Debug, Serialize)]
/// Persistent startup diagnostic; each failure is displayed once as a dismissible toast.
pub struct LoadFailure {
    pub file: String,
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Revisioned backend state supports progressive discovery and event/command race reconciliation.
pub struct LibraryState {
    pub revision: u64,
    pub loading: bool,
    pub writable: bool,
    pub macros: Vec<MacroSummary>,
    pub failures: Vec<LoadFailure>,
}

/// File association and digest are internal; frontend requests use IDs, never arbitrary paths.
struct Entry {
    summary: MacroSummary,
    path: PathBuf,
    fingerprint: Vec<u8>,
    created_ms: i64,
}
impl Entry {
    /// Reduces a validated action document to metadata while retaining its actual backing-file association.
    fn from_document(
        path: PathBuf,
        document: MacroDocument,
        fingerprint: Vec<u8>,
    ) -> Result<Self, ValidationError> {
        let created_ms =
            macro_format::timestamp(&document.created_at, macro_format::fields::CREATED_AT)?;
        let mut playback = document.playback;
        // Extra property metadata is preserved in action documents, but never retained by the library cache.
        playback.extra.clear();
        let summary = MacroSummary {
            id: document.id,
            name: document.name,
            duration_ms: document.duration_ms,
            created_at: document.created_at,
            playback,
        };
        Ok(Self {
            summary,
            path,
            fingerprint,
            created_ms,
        })
    }
}
/// Short-lived synchronized metadata; disk I/O and JSON validation occur outside this lock.
struct Inner {
    started: bool,
    state: LibraryState,
    entries: HashMap<String, Entry>,
}
/// Executable-relative library repository with progressive loading and lazy action snapshots.
pub struct Repository {
    directory: PathBuf,
    inner: Mutex<Inner>,
}

impl Repository {
    /// Constructs an unloaded repository for a trusted directory; does no disk I/O.
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            inner: Mutex::new(Inner {
                started: false,
                state: LibraryState {
                    revision: 0,
                    loading: true,
                    writable: false,
                    macros: Vec::new(),
                    failures: Vec::new(),
                },
                entries: HashMap::new(),
            }),
        }
    }
    /// Resolves storage from an executable path, independently of the process working directory.
    pub fn beside_executable(executable: &Path) -> Result<Self, String> {
        let parent = executable
            .parent()
            .ok_or("Executable directory is unavailable")?;
        Ok(Self::new(parent.join(MACROS_DIRECTORY)))
    }
    /// Returns a metadata-only snapshot; poisoning is surfaced as a recoverable command error.
    pub fn snapshot(&self) -> Result<LibraryState, String> {
        Ok(self.lock()?.state.clone())
    }
    /// Suggests an initial filename without reserving or overwriting it; save must commit without clobbering.
    /// Display names remain independent and later metadata renames keep their existing file association.
    pub fn available_filename(&self, name: &str) -> Result<String, String> {
        let stem = filename_stem(name);
        let names: Vec<_> = fs::read_dir(&self.directory)
            .map_err(|error| error.to_string())?
            .map(|item| item.map(|entry| entry.file_name().to_string_lossy().to_lowercase()))
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?;
        let mut candidate = format!("{stem}.{JSON_EXTENSION}");
        let mut suffix = FIRST_COLLISION_SUFFIX;
        while names.contains(&candidate.to_lowercase()) {
            candidate = format!("{stem}_{suffix}.{JSON_EXTENSION}");
            suffix = suffix
                .checked_add(1)
                .ok_or("Filename suffix limit exceeded")?;
        }
        Ok(candidate)
    }
    /// Loads once on a worker, preserving valid entries and reporting each independent failure immediately.
    /// Callback receives immutable snapshots outside the state lock; repeated calls reuse existing state.
    pub fn load(&self, mut publish: impl FnMut(LibraryState)) {
        if !self.begin() {
            return;
        }
        match self.discover(&mut publish) {
            Ok(paths) => {
                for path in paths {
                    self.load_file(path);
                    self.publish(&mut publish);
                }
            }
            Err(error) => self.failure(&self.directory, error),
        }
        if let Ok(mut inner) = self.lock() {
            inner.state.loading = false;
            inner.state.revision += 1;
        }
        self.publish(&mut publish);
    }
    /// Reads the selected file lazily, comparing the same bytes decoded for the action against startup.
    /// Missing/changed files reject the action with restart guidance; native input is never executed here.
    pub fn read(&self, id: &str) -> Result<MacroDocument, String> {
        let (path, fingerprint) = self.association(id)?;
        let bytes = read_bounded(&path)
            .map_err(|error| format!("{}: {}. {RESTART_GUIDANCE}", error.field, error.message))?;
        if digest(&bytes) != fingerprint {
            return Err(format!("Macro file changed. {RESTART_GUIDANCE}"));
        }
        macro_format::decode(&bytes).map_err(|error| format!("{}: {}", error.field, error.message))
    }
    /// Refreshes cached properties/digest after a trusted backend consumer successfully commits an internal save.
    /// Keeps the actual filename and prior cache on failure; this is not exposed as an external-change bypass command.
    pub fn refresh_after_save(&self, id: &str) -> Result<LibraryState, String> {
        let (path, _) = self.association(id)?;
        let bytes = read_bounded(&path).map_err(|error| error.message)?;
        let document = macro_format::decode(&bytes)
            .map_err(|error| format!("{}: {}", error.field, error.message))?;
        if document.id != id {
            return Err("Saved macro ID changed".into());
        }
        let entry =
            Entry::from_document(path, document, digest(&bytes)).map_err(|error| error.message)?;
        let mut inner = self.lock()?;
        inner.entries.insert(id.into(), entry);
        inner.state.macros = sorted_summaries(&inner.entries);
        inner.state.revision += 1;
        Ok(inner.state.clone())
    }
    /// Acquires metadata synchronization; no caller may retain this guard during disk operations.
    fn lock(&self) -> Result<MutexGuard<'_, Inner>, String> {
        self.inner.lock().map_err(|_| LOCK_FAILURE.into())
    }
    /// Claims the one startup scan; poison or an existing scan leaves state untouched.
    fn begin(&self) -> bool {
        let Ok(mut inner) = self.lock() else {
            return false;
        };
        if inner.started {
            return false;
        }
        inner.started = true;
        true
    }
    /// Creates storage then enumerates committed JSON files in deterministic filename order.
    fn discover(
        &self,
        publish: &mut impl FnMut(LibraryState),
    ) -> Result<Vec<PathBuf>, ValidationError> {
        fs::create_dir_all(&self.directory)
            .map_err(|error| ValidationError::new(DIAGNOSTIC_DIRECTORY, error.to_string()))?;
        match probe_writable(&self.directory) {
            Ok(()) => {
                if let Ok(mut inner) = self.lock() {
                    inner.state.writable = true;
                }
            }
            Err(error) => self.failure(
                &self.directory,
                ValidationError::new(
                    DIAGNOSTIC_STORAGE,
                    format!("Writes are unavailable: {error}"),
                ),
            ),
        }
        self.publish(publish);
        let reader = fs::read_dir(&self.directory)
            .map_err(|error| ValidationError::new(DIAGNOSTIC_DIRECTORY, error.to_string()))?;
        let mut paths = Vec::new();
        for item in reader {
            match item {
                Ok(item) => {
                    let path = item.path();
                    if is_json(&path) {
                        paths.push(path);
                    }
                }
                Err(error) => {
                    self.failure(
                        &self.directory,
                        ValidationError::new(DIAGNOSTIC_DIRECTORY, error.to_string()),
                    );
                    self.publish(publish);
                }
            }
        }
        paths.sort_by(|left, right| {
            left.to_string_lossy()
                .to_lowercase()
                .cmp(&right.to_string_lossy().to_lowercase())
                .then_with(|| left.cmp(right))
        });
        Ok(paths)
    }
    /// Validates one file before inserting its stable ID; invalid files never reserve identities.
    fn load_file(&self, path: PathBuf) {
        let result = read_bounded(&path).and_then(|bytes| {
            let document = macro_format::decode(&bytes)?;
            Ok((document, digest(&bytes)))
        });
        match result {
            Ok((document, fingerprint)) => self.insert(path, document, fingerprint),
            Err(error) => self.failure(&path, error),
        }
    }
    /// Retains only metadata and the actual file path; a later duplicate leaves the first valid entry intact.
    fn insert(&self, path: PathBuf, document: MacroDocument, fingerprint: Vec<u8>) {
        let Ok(mut inner) = self.lock() else {
            return;
        };
        if inner.entries.contains_key(&document.id) {
            drop(inner);
            self.failure(
                &path,
                ValidationError::new(
                    macro_format::fields::ID,
                    "duplicate ID; the first valid file was retained",
                ),
            );
            return;
        }
        let entry = match Entry::from_document(path.clone(), document, fingerprint) {
            Ok(entry) => entry,
            Err(error) => {
                drop(inner);
                self.failure(&path, error);
                return;
            }
        };
        inner.entries.insert(entry.summary.id.clone(), entry);
        inner.state.macros = sorted_summaries(&inner.entries);
        inner.state.revision += 1;
    }
    /// Records a file/field failure without modifying the file or hiding successful entries.
    fn failure(&self, path: &Path, error: ValidationError) {
        if let Ok(mut inner) = self.lock() {
            let file = diagnostic_file(path, &error.field);
            inner.state.failures.push(LoadFailure {
                file,
                field: error.field,
                message: error.message,
            });
            inner.state.revision += 1;
        }
    }
    /// Publishes a metadata snapshot outside synchronization; poisoned state cannot be published.
    fn publish(&self, callback: &mut impl FnMut(LibraryState)) {
        if let Ok(state) = self.snapshot() {
            callback(state);
        }
    }
    /// Captures a trusted association briefly; unknown IDs cannot be used to access other paths.
    fn association(&self, id: &str) -> Result<(PathBuf, Vec<u8>), String> {
        let inner = self.lock()?;
        let entry = inner
            .entries
            .get(id)
            .ok_or("Selected macro is unavailable")?;
        Ok((entry.path.clone(), entry.fingerprint.clone()))
    }
}

/// Uses a unique non-JSON probe to test actual directory write/delete access, including Windows ACLs.
fn probe_writable(directory: &Path) -> Result<(), std::io::Error> {
    let sequence = NEXT_PROBE.fetch_add(1, Ordering::Relaxed);
    let path = directory.join(format!(".macroloom-{}-{sequence}.tmp", std::process::id()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    drop(file);
    fs::remove_file(path)
}

/// Names affected files concisely while retaining the full location for directory/storage failures.
fn diagnostic_file(path: &Path, field: &str) -> String {
    if field == DIAGNOSTIC_DIRECTORY || field == DIAGNOSTIC_STORAGE {
        return path.display().to_string();
    }
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Checks the committed extension only; temporary `.json.tmp` files are excluded.
fn is_json(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension
            .to_string_lossy()
            .eq_ignore_ascii_case(JSON_EXTENSION)
    })
}
/// Reads at most the configured limit plus one byte, detecting growth without unbounded allocation.
fn read_bounded(path: &Path) -> Result<Vec<u8>, ValidationError> {
    let file = File::open(path)
        .map_err(|error| ValidationError::new(DIAGNOSTIC_FILE, error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ValidationError::new(DIAGNOSTIC_FILE, error.to_string()))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ValidationError::new(
            DIAGNOSTIC_FILE,
            "exceeds the 16 MiB limit",
        ));
    }
    Ok(bytes)
}
/// Returns a content digest without keeping source bytes or events in repository metadata.
fn digest(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}
/// Sorts normalized creation instants descending, then IDs ascending, independently of filenames.
fn sorted_summaries(entries: &HashMap<String, Entry>) -> Vec<MacroSummary> {
    let mut ordered: Vec<_> = entries.values().collect();
    ordered.sort_by(|left, right| {
        right
            .created_ms
            .cmp(&left.created_ms)
            .then_with(|| left.summary.id.cmp(&right.summary.id))
    });
    ordered
        .into_iter()
        .map(|entry| entry.summary.clone())
        .collect()
}

/// Derives a Windows-safe stem while retaining display metadata and replacing each whitespace character.
fn filename_stem(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_whitespace()
                || character.is_control()
                || INVALID_FILENAME_CHARS.contains(&character)
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    let mut stem = sanitized.trim_end_matches(['.', ' ']).to_string();
    if stem.is_empty() {
        stem = DEFAULT_STEM.into();
    }
    if reserved_stem(&stem) {
        stem.insert(0, '_');
    }
    stem
}

/// Detects device names even before an additional extension and under case-insensitive Windows naming.
fn reserved_stem(stem: &str) -> bool {
    let base = stem.split('.').next().unwrap_or(stem).to_uppercase();
    if DEVICE_NAMES.contains(&base.as_str()) {
        return true;
    }
    let suffix = base
        .strip_prefix("COM")
        .or_else(|| base.strip_prefix("LPT"));
    suffix.is_some_and(|value| {
        value.chars().count() == 1
            && value
                .chars()
                .any(|character| DEVICE_DIGITS.contains(&character))
    })
}
