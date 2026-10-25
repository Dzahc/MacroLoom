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
const FIRST_SAVE: u64 = 1;

#[test]
/// Both empty and nonempty recordings retain the explicit sleep reason after wake without restarting or duplicating a macro.
fn sleep_recording_outcome_retains_the_interruption_reason() {
    for initial in [Vec::new(), vec![test_support::key(true)]] {
        let empty = initial.is_empty();
        let desktop = Desktop::new(initial);
        let session = Session::new(desktop.clone());
        session.record().expect("Record before sleep");
        let stopped = session
            .suspend()
            .expect("Freeze and save interrupted capture");
        assert_eq!(stopped.phase, Phase::Idle);
        assert!(stopped.message.contains(RECORDING_INTERRUPTED));
        assert_eq!(stopped.recording_outcome.is_none(), empty);
        assert_eq!(
            session.refresh().expect("Wake refresh").message,
            stopped.message
        );
        assert_eq!(
            desktop
                .folder
                .repository
                .snapshot()
                .expect("Library")
                .macros
                .len(),
            usize::from(!empty)
        );
    }
}

#[test]
/// Failed close saves retain their receipt across repeated Close/suspend; Keep open restores ordinary recovery without exit.
fn close_failure_requires_an_explicit_exit_recovery_choice() {
    let desktop = Desktop::new(vec![test_support::key(true)]);
    let session = Session::new(desktop.clone());
    desktop.fail_save.store(true, Ordering::Release);
    session.record().expect("Start recording");
    session.request_close().expect("Close freezes capture");
    let failed = session.stop().expect("Save fails safely");
    let receipt = failed.recording_outcome.expect("Failure receipt");
    assert!(failed.close_requested && failed.save_failed);
    session.request_close().expect("Repeated Close");
    let unchanged = session.suspend().expect("Sleep after failure");
    let retained = unchanged.recording_outcome.expect("Same failure receipt");
    assert_eq!(retained.sequence, receipt.sequence);
    assert_eq!(retained.id, receipt.id);
    assert!(session.discard_recording().is_err());
    let kept = session.keep_open().expect("Cancel exit, retain recording");
    assert!(!kept.close_requested && kept.unsaved && kept.save_failed);
    desktop.fail_save.store(false, Ordering::Release);
    let saved = session.retry_recording().expect("Ordinary Retry");
    assert!(!saved.close_requested && !saved.unsaved);
    assert_eq!(
        saved.recording_outcome.expect("Saved receipt").id,
        receipt.id
    );
}

/// Desktop-effect fixture; input, window activation and disk-failure flags never replace session internals.
struct Desktop {
    folder: Folder,
    fail_save: AtomicBool,
    fail_start: AtomicBool,
    shortcut_failure: AtomicBool,
    changed: AtomicBool,
    compact: AtomicBool,
    initial: Vec<EventKind>,
    environment_check: Mutex<Option<EnvironmentCheck>>,
    save_check: Mutex<Option<EnvironmentCheck>>,
}
impl Desktop {
    /// Creates a writable real library and optionally held boundary inputs, with no physical input effects.
    fn new(initial: Vec<EventKind>) -> Arc<Self> {
        Arc::new(Self {
            folder: Folder::new(),
            fail_save: AtomicBool::new(false),
            fail_start: AtomicBool::new(false),
            shortcut_failure: AtomicBool::new(false),
            changed: AtomicBool::new(false),
            compact: AtomicBool::new(false),
            initial,
            environment_check: Mutex::new(None),
            save_check: Mutex::new(None),
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
        let check = self.save_check.lock().expect("Save fixture lock").take();
        if let Some(check) = check {
            check()?;
        }
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
    /// Makes both shortcuts available or reports a controlled native failure without touching Windows registrations.
    fn shortcuts(&self, _: bool, _: bool) -> Result<Availability, String> {
        if self.shortcut_failure.load(Ordering::Acquire) {
            return Err(FAILURE.into());
        }
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
/// A committed interrupted recording retains its save outcome across native shortcut failure and recovery after wake.
fn wake_shortcut_error_retains_the_saved_interruption_outcome() {
    let desktop = Desktop::new(vec![test_support::key(true)]);
    let session = Session::new(desktop.clone());
    session.record().expect("Record before suspend");
    let interrupted = session.suspend().expect("Save interrupted recording");
    let outcome = interrupted.recording_outcome.expect("Committed outcome");
    assert!(outcome.success);
    desktop.shortcut_failure.store(true, Ordering::Release);
    session.report(session.refresh().err().expect("Automatic shortcut error"));
    let failed = session.snapshot().expect("Outcome and native error");
    assert!(failed.message.contains(&outcome.message));
    assert!(failed.message.contains(FAILURE));
    desktop.shortcut_failure.store(false, Ordering::Release);
    assert_eq!(
        session.refresh().expect("Native recovery").message,
        outcome.message
    );
}

#[test]
/// Close joins an in-flight save; suspend/repeated Close cannot create another attempt or release exit before its result.
fn close_and_suspend_join_one_pending_save() {
    let desktop = Desktop::new(vec![test_support::key(true)]);
    let session = Session::new(desktop.clone());
    let (entered, observed) = std::sync::mpsc::sync_channel(1);
    let (release, resume) = std::sync::mpsc::sync_channel(1);
    *desktop.save_check.lock().expect("Save fixture") = Some(Box::new(move || {
        entered.send(()).expect("Save observer");
        resume
            .recv_timeout(ENVIRONMENT_CHECK_TIMEOUT)
            .expect("Release save");
        Ok(())
    }));
    session.record().expect("Start recording");
    let worker_session = session.clone();
    let save = std::thread::spawn(move || worker_session.stop());
    observed
        .recv_timeout(ENVIRONMENT_CHECK_TIMEOUT)
        .expect("Save is pending");
    session.request_close().expect("Close joins save");
    session.request_close().expect("Repeated Close joins save");
    let suspended = session.suspend().expect("Sleep while saving");
    assert_eq!(suspended.phase, Phase::Saving);
    assert!(!session.finish_exit().expect("Cannot exit before commit"));
    assert!(session.discard_exit().is_err());
    release.send(()).expect("Resume pending commit");
    let saved = save.join().expect("Save worker").expect("Save completes");
    let outcome = saved.recording_outcome.expect("One receipt");
    assert!(outcome.success && saved.close_requested && !saved.unsaved);
    assert_eq!(outcome.sequence, FIRST_SAVE);
    assert!(session.finish_exit().expect("Exit after commit"));
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

#[test]
/// Session Stop commits wheel data and the final observed drag point, leaving held-button cleanup to future playback.
fn stopping_a_mouse_recording_commits_its_wheel_and_final_held_position() {
    const FINAL_X: i32 = test_support::TEST_X + 30;
    const FINAL_Y: i32 = test_support::TEST_Y - 20;
    const WHEEL_DELTA: i32 = -45;
    const EXPECTED_EVENTS: usize = 4;
    const ENDPOINT_INDEX: usize = 2;
    let desktop = Desktop::new(vec![button(true)]);
    let session = Session::new(desktop.clone());
    session.record().expect("Record mouse actions");
    session.capture(EventKind::Move {
        x: test_support::TEST_X,
        y: FINAL_Y,
    });
    session.capture(EventKind::Move {
        x: FINAL_X,
        y: FINAL_Y,
    });
    session.capture(EventKind::Wheel {
        delta: WHEEL_DELTA,
        x: FINAL_X,
        y: FINAL_Y,
    });
    let stopped = session.stop().expect("Save held drag");
    let outcome = stopped.recording_outcome.expect("Save receipt");
    assert!(outcome.success);
    let document = desktop
        .folder
        .repository
        .read(&outcome.id)
        .expect("Committed mouse recording");
    assert_eq!(document.events.len(), EXPECTED_EVENTS);
    let crate::macro_format::EventData::MouseMove { x, y } = document.events[ENDPOINT_INDEX].data
    else {
        panic!("Expected final observed drag endpoint");
    };
    assert_eq!((x, y), (FINAL_X, FINAL_Y));
    let crate::macro_format::EventData::MouseWheel { delta, x, y, .. } =
        document.events.last().expect("Wheel event").data
    else {
        panic!("Expected saved positioned wheel");
    };
    assert_eq!(delta, WHEEL_DELTA);
    assert_eq!((x, y), (FINAL_X, FINAL_Y));
}
