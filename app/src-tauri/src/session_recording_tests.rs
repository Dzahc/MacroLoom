//! Recording/save behavior through session commands with real repository commits and deterministic desktop effects.
use super::*;
use crate::recording::test_support::{self, Folder};
use crate::repository::recording_save::RecordingCommit;
const FAILURE: &str = "Simulated storage failure";
const CONFIRMATION: &str = "recording-test";
const TRAILING_WAIT: Duration = Duration::from_millis(5);
const INITIAL_EVENT_COUNT: usize = 2;
const FINAL_EVENT_COUNT: usize = 4;
const EXPECTED_KEY_DOWN: &str = "Initial held key must be saved as a press";
const EXPECTED_MOUSE_DOWN: &str = "Initial held button must be saved as a press";
const ENVIRONMENT_CHECK_TIMEOUT: Duration = Duration::from_secs(2);
/// One delayed platform observation, substituted only at the external environment boundary.
type EnvironmentCheck = Box<dyn FnOnce() -> Result<(), String> + Send>;

/// Desktop-effect fixture; input, window activation and disk-failure flags never replace session internals.
struct Desktop {
    folder: Folder,
    fail_save: AtomicBool,
    fail_start: AtomicBool,
    changed: AtomicBool,
    compact: AtomicBool,
    initial: Vec<EventKind>,
    environment_check: Mutex<Option<EnvironmentCheck>>,
}
impl Desktop {
    /// Creates a writable real library and optionally held boundary inputs, with no physical input effects.
    fn new(initial: Vec<EventKind>) -> Arc<Self> {
        Arc::new(Self {
            folder: Folder::new(),
            fail_save: AtomicBool::new(false),
            fail_start: AtomicBool::new(false),
            changed: AtomicBool::new(false),
            compact: AtomicBool::new(false),
            initial,
            environment_check: Mutex::new(None),
        })
    }
}
impl Platform for Desktop {
    /// Checks storage before allocating the identity, mirroring the native preflight boundary.
    fn recording(&self) -> Result<Option<Recording>, String> {
        if self.fail_start.load(Ordering::Acquire) {
            return Err(FAILURE.into());
        }
        self.folder.repository.recording_preflight()?;
        Recording::new(test_support::environment()).map(Some)
    }
    /// Returns known held inputs without querying or changing the test runner's real desktop.
    fn initial_inputs(&self) -> Vec<EventKind> {
        self.initial.clone()
    }
    /// Makes the live environment change at an explicit test-controlled boundary.
    fn check_environment(
        &self,
        _: &crate::macro_format::RecordingEnvironment,
    ) -> Result<(), String> {
        let check = self
            .environment_check
            .lock()
            .expect("Environment fixture lock")
            .take();
        if let Some(check) = check {
            return check();
        }
        if self.changed.load(Ordering::Acquire) {
            Err(crate::recording::ENVIRONMENT_CHANGED.into())
        } else {
            Ok(())
        }
    }
    /// Uses the real commit path and verifies that native restoration precedes storage work.
    fn save_recording(&self, recording: &mut Recording) -> Result<RecordingCommit, String> {
        assert!(!self.compact.load(Ordering::Acquire));
        if self.fail_save.load(Ordering::Acquire) {
            return Err(FAILURE.into());
        }
        self.folder.repository.save_recording(recording)
    }
    /// Supplies the external operation token; phase exclusion itself stays session-owned.
    fn reserve(&self) -> Result<Box<dyn Send>, String> {
        Ok(Box::new(()))
    }
    /// The fixture has no external editor/deletion owner.
    fn available(&self) -> bool {
        true
    }
    /// Makes both fixed shortcuts available without registering anything with Windows.
    fn shortcuts(&self, _: bool, _: bool) -> Result<Availability, String> {
        Ok(Availability {
            record: true,
            stop: true,
        })
    }
    /// Tracks actual view state so commits can assert full-view saving.
    fn view(&self, compact: bool) -> Result<(), String> {
        self.compact.store(compact, Ordering::Release);
        Ok(())
    }
    /// Uses signed physical fixture coordinates, without reading the real mouse.
    fn cursor(&self) -> (i32, i32) {
        (test_support::TEST_X, test_support::TEST_Y)
    }
    /// Ordinary playback is outside these recording tests.
    fn inject(&self, _: &EventKind) -> Result<(), String> {
        Ok(())
    }
    /// Cleanup has no real desktop input to release.
    fn release(&self, _: Held) -> Result<(), String> {
        Ok(())
    }
    /// Tests query the same durable snapshots that disconnected native clients can query.
    fn publish(&self, _: Snapshot) {}
}
#[test]
/// A delayed failure for recording A must not stop/save recording B after A's ownership has ended.
fn regression_stale_environment_check_cannot_interrupt_a_new_recording() {
    let desktop = Desktop::new(vec![test_support::key(true)]);
    let session = Session::new(desktop.clone());
    session.record().expect("Start recording A");
    let (entered, observed) = std::sync::mpsc::sync_channel(1);
    let (release, resume) = std::sync::mpsc::sync_channel(1);
    *desktop
        .environment_check
        .lock()
        .expect("Install environment fixture") = Some(Box::new(
        // Holds A's observation while the main test stops A and starts B through real session commands.
        move || {
            entered.send(()).expect("Observation receiver remains live");
            resume
                .recv_timeout(ENVIRONMENT_CHECK_TIMEOUT)
                .expect("Delayed observation released");
            Err(crate::recording::ENVIRONMENT_CHANGED.into())
        },
    ));
    let observer = session.clone();
    let check = std::thread::spawn(
        // Runs the same worker refresh that checks environment outside the state lock.
        move || observer.refresh(),
    );
    observed
        .recv_timeout(ENVIRONMENT_CHECK_TIMEOUT)
        .expect("A's environment check reached the barrier");
    session.stop().expect("Stop and save A");
    session.record().expect("Start recording B");
    release.send(()).expect("Release A's delayed failure");
    let result = check
        .join()
        .expect("Environment worker completes")
        .expect("Refresh succeeds");
    assert_eq!(
        result.phase,
        Phase::Recording,
        "A's stale environment failure interrupted B"
    );
    assert_eq!(
        session.snapshot().expect("B's live state").phase,
        Phase::Recording
    );
    session.stop().expect("Stop B after regression assertion");
}
/// Builds an initially held supported button at a signed physical screen position.
fn button(down: bool) -> EventKind {
    EventKind::Mouse {
        button: crate::MouseButton::Left,
        down,
        x: test_support::TEST_X,
        y: test_support::TEST_Y,
    }
}

#[test]
/// Held inputs begin at zero, repetitions survive, and Stop never invents end releases or loses trailing wait.
fn held_snapshot_and_observed_events_are_saved_without_synthetic_releases() {
    let desktop = Desktop::new(vec![test_support::key(true), button(true)]);
    let session = Session::new(desktop.clone());
    assert_eq!(
        session.record().expect("Record").event_count,
        INITIAL_EVENT_COUNT
    );
    session.capture(test_support::key(true));
    session.capture(button(false));
    std::thread::sleep(TRAILING_WAIT);
    let stopped = session.stop().expect("Stop and save");
    assert_eq!(stopped.phase, Phase::Idle);
    assert!(!stopped.unsaved);
    let outcome = stopped.recording_outcome.expect("Commit receipt");
    assert!(outcome.success);
    let document = desktop
        .folder
        .repository
        .read(&outcome.id)
        .expect("Saved recording");
    assert_eq!(document.events.len(), FINAL_EVENT_COUNT);
    assert_eq!(document.events[0].at_ms, 0);
    assert_eq!(document.events[1].at_ms, 0);
    assert!(document.duration_ms > document.events.last().expect("Last event").at_ms);
    let crate::macro_format::EventData::KeyDown { native, .. } = &document.events[0].data else {
        panic!("{EXPECTED_KEY_DOWN}");
    };
    assert_eq!(native.virtual_key, test_support::TEST_KEY);
    let crate::macro_format::EventData::MouseDown { x, y, .. } = &document.events[1].data else {
        panic!("{EXPECTED_MOUSE_DOWN}");
    };
    assert_eq!((*x, *y), (test_support::TEST_X, test_support::TEST_Y));
}
#[test]
/// Failure blocks all new operations, retains the frozen identity, and Retry publishes success only after one commit.
fn failed_save_retains_capture_and_retry_commits_one_macro() {
    let desktop = Desktop::new(Vec::new());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture(test_support::key(true));
    desktop.fail_save.store(true, Ordering::Release);
    let failed = session.stop().expect("Recoverable save failure");
    assert!(failed.save_failed && failed.unsaved);
    assert_eq!(failed.phase, Phase::Saving);
    let prior = failed.recording_outcome.expect("Failure receipt");
    assert!(!prior.success);
    assert!(desktop
        .folder
        .repository
        .snapshot()
        .expect("Library")
        .macros
        .is_empty());
    assert!(session.record().is_err());
    assert!(session.confirmation(CONFIRMATION, true).is_err());
    desktop.fail_save.store(false, Ordering::Release);
    let saved = session.retry_recording().expect("Retry");
    let outcome = saved.recording_outcome.expect("Success receipt");
    assert!(outcome.success);
    assert_eq!(outcome.id, prior.id);
    assert!(!saved.unsaved);
    assert!(session.retry_recording().is_err());
    assert_eq!(
        desktop
            .folder
            .repository
            .snapshot()
            .expect("Library")
            .macros
            .len(),
        1
    );
}
#[test]
/// Empty input creates no file/outcome, while storage preflight prevents capture and allows an immediate repaired start.
fn empty_recording_and_failed_preflight_never_publish_success() {
    let desktop = Desktop::new(Vec::new());
    let session = Session::new(desktop.clone());
    desktop.fail_start.store(true, Ordering::Release);
    assert!(session.record().is_err());
    assert_eq!(session.snapshot().expect("State").phase, Phase::Idle);
    desktop.fail_start.store(false, Ordering::Release);
    session.record().expect("Repaired storage");
    let empty = session.stop().expect("Empty Stop");
    assert_eq!(empty.message, crate::recording::EMPTY_RECORDING);
    assert!(empty.recording_outcome.is_none());
    assert!(desktop
        .folder
        .repository
        .snapshot()
        .expect("Library")
        .macros
        .is_empty());
}
#[test]
/// Environment changes automatically stop and save the valid prefix, retaining an explanation without resuming.
fn environment_change_saves_the_prefix_and_explains_the_stop() {
    let desktop = Desktop::new(vec![test_support::key(true)]);
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    desktop.changed.store(true, Ordering::Release);
    let saved = session.refresh().expect("Environmental auto stop");
    assert_eq!(saved.phase, Phase::Idle);
    assert!(saved
        .message
        .contains(crate::recording::ENVIRONMENT_CHANGED));
    assert!(saved.recording_outcome.expect("Commit").success);
}
#[test]
/// The first keyboard event under a changed layout is excluded before the background worker saves the valid prefix.
fn changed_layout_cannot_append_an_incompatible_keyboard_event() {
    const CHANGED_LAYOUT: &str = "0000040C";
    let desktop = Desktop::new(vec![test_support::key(true)]);
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture_with_layout(test_support::key(false), CHANGED_LAYOUT);
    let stopped = session.refresh().expect("Save valid prefix");
    let outcome = stopped.recording_outcome.expect("Saved prefix");
    let document = desktop
        .folder
        .repository
        .read(&outcome.id)
        .expect("Committed prefix");
    assert_eq!(document.events.len(), 1);
    assert_eq!(
        document.recording.keyboard_layout,
        test_support::TEST_LAYOUT
    );
}
#[test]
/// Failed captures remain recoverable until the explicit Discard action; close/suspend use the same automatic save path.
fn discard_is_explicit_and_sleep_or_close_commits_the_capture() {
    let desktop = Desktop::new(vec![test_support::key(true)]);
    let session = Session::new(desktop.clone());
    desktop.fail_save.store(true, Ordering::Release);
    session.record().expect("Record");
    session.stop().expect("Failed save");
    assert!(
        !session
            .discard_recording()
            .expect("Explicit Discard")
            .unsaved
    );
    desktop.fail_save.store(false, Ordering::Release);
    session.record().expect("Next record");
    assert!(
        session
            .suspend()
            .expect("Sleep")
            .recording_outcome
            .expect("Saved prefix")
            .success
    );
    session.record().expect("Record before close");
    assert!(session.request_close().expect("Close freezes recording"));
    let closed = session.stop().expect("Save before exit");
    assert!(closed.close_requested && !closed.unsaved);
}
#[test]
/// Accepting the maximum number of events never saves an invalid oversized array or admits events after the limit.
fn event_limit_automatically_saves_a_valid_prefix() {
    let desktop = Desktop::new(Vec::new());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    for _ in 0..=crate::macro_format::MAX_EVENTS {
        session.capture(test_support::key(true));
    }
    let stopped = session.refresh().expect("Complete automatic stop");
    assert_eq!(stopped.phase, Phase::Idle);
    let outcome = stopped.recording_outcome.expect("Saved prefix");
    assert!(outcome.success);
    assert!(outcome.message.contains(crate::recording::LIMIT_REACHED));
    let document = desktop
        .folder
        .repository
        .read(&outcome.id)
        .expect("Valid prefix");
    assert_eq!(document.events.len(), crate::macro_format::MAX_EVENTS);
}
