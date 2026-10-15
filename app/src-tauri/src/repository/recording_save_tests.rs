//! Recording commit evidence through the real repository and non-clobbering filesystem boundary.
use super::*;
use crate::input::Capture;
use crate::recording::test_support::{self, Folder};
use std::time::Duration;
const TEST_NAME: &str = "Macro recording fixture";
const COLLISION_FILE: &str = "collision.json";
const PRIOR_BYTES: &[u8] = b"previous valid file remains untouched";
const EXPECTED_ONE_FILE: usize = 1;
const TEST_DURATION: Duration = Duration::from_millis(50);
const COMMIT_FAILURE: &str = "Injected atomic commit failure";

/// Builds a nonempty frozen recording, retaining a held key without a synthetic release.
fn recording() -> Recording {
    let mut capture = Capture::default();
    capture.append(Duration::ZERO, test_support::key(true));
    capture.finish(TEST_DURATION, (test_support::TEST_X, test_support::TEST_Y));
    let mut recording = Recording::new(test_support::environment()).expect("Fixture metadata");
    recording.document.name = TEST_NAME.into();
    recording.freeze(&capture);
    recording
}
#[test]
/// Repeating a commit of the same frozen capture creates exactly one file and the same selected identity.
fn retries_commit_the_same_identity_once() {
    let folder = Folder::new();
    let mut recording = recording();
    let original = recording.document.clone();
    folder
        .repository
        .save_recording(&mut recording)
        .expect("Commit");
    folder
        .repository
        .save_recording(&mut recording)
        .expect("Idempotent retry");
    let state = folder.repository.snapshot().expect("Committed cache");
    assert_eq!(state.macros.len(), EXPECTED_ONE_FILE);
    assert_eq!(state.macros[0].id, original.id);
    let document = folder
        .repository
        .read(&original.id)
        .expect("Committed bytes");
    assert_eq!(document.created_at, original.created_at);
    assert_eq!(document.duration_ms, original.duration_ms);
    assert_eq!(document.events.len(), EXPECTED_ONE_FILE);
    assert_eq!(
        fs::read_dir(&folder.path).expect("Directory").count(),
        EXPECTED_ONE_FILE
    );
}
#[test]
/// A path collision never overwrites the winner and a second generated macro gets a different ID and display name.
fn collisions_preserve_existing_files_and_generated_names_are_unique() {
    let folder = Folder::new();
    let mut first = recording();
    first.filename = Some(COLLISION_FILE.into());
    fs::write(folder.path.join(COLLISION_FILE), PRIOR_BYTES).expect("External winner");
    folder
        .repository
        .save_recording(&mut first)
        .expect("Resolve collision");
    assert_eq!(
        fs::read(folder.path.join(COLLISION_FILE)).expect("Prior file"),
        PRIOR_BYTES
    );
    let mut second = recording();
    folder
        .repository
        .save_recording(&mut second)
        .expect("Next recording");
    assert_ne!(first.document.id, second.document.id);
    assert_ne!(first.document.name, second.document.name);
}
#[test]
/// Failure after staging leaves the draft and every prior file intact; removing the obstruction permits Retry.
fn failed_commit_retains_draft_and_cleans_only_its_temporary_file() {
    let folder = Folder::new();
    fs::write(folder.path.join(COLLISION_FILE), PRIOR_BYTES).expect("Existing file");
    let mut recording = recording();
    let identity = recording.document.id.clone();
    assert!(folder
        .repository
        .save_recording_using(
            &mut recording,
            // Fails after staging and syncing, exactly at the OS commit boundary.
            |staged, _| {
                assert!(staged.exists());
                Err(std::io::Error::new(
                    ErrorKind::PermissionDenied,
                    COMMIT_FAILURE,
                ))
            }
        )
        .is_err());
    assert!(folder
        .repository
        .snapshot()
        .expect("Cache")
        .macros
        .is_empty());
    assert_eq!(
        fs::read_dir(&folder.path).expect("Directory").count(),
        EXPECTED_ONE_FILE
    );
    assert_eq!(
        fs::read(folder.path.join(COLLISION_FILE)).expect("Prior file unchanged"),
        PRIOR_BYTES
    );
    folder
        .repository
        .save_recording(&mut recording)
        .expect("Retry");
    assert_eq!(recording.document.id, identity);
}
#[test]
/// A storage path that becomes a regular file fails preflight and recovers after the directory is restored.
fn preflight_rechecks_actual_storage_and_empty_documents_never_commit() {
    let folder = Folder::new();
    let recording = Recording::new(test_support::environment()).expect("Metadata");
    assert!(folder
        .repository
        .save_recording(&mut recording.clone())
        .is_err());
    fs::remove_dir(&folder.path).expect("Empty fixture");
    fs::write(&folder.path, PRIOR_BYTES).expect("Unwritable folder path");
    assert!(folder.repository.recording_preflight().is_err());
    assert_eq!(
        fs::read(&folder.path).expect("Obstruction retained"),
        PRIOR_BYTES
    );
    fs::remove_file(&folder.path).expect("Remove fixture obstruction");
    folder
        .repository
        .recording_preflight()
        .expect("Recovered directory");
}
