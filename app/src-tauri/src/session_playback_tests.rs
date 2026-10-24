//! Saved-library playback through session commands and real repository reads, substituting only desktop effects.
use super::*;
use crate::input::{Event, MouseButton};
use crate::recording::test_support::{self, Folder};
use crate::shortcuts::Availability;
use std::sync::atomic::AtomicUsize;

const FAILURE: &str = "Simulated native rejection";
const END: Duration = Duration::from_millis(40);
const LONG_WAIT: Duration = Duration::from_secs(60);
const TIMEOUT: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(1);
const NONDEFAULT_SPEED: f64 = 2.0;
const NONDEFAULT_RUNS: u64 = 3;
const WHEEL: i32 = -30;
const PREPARATION_ELAPSED_MS: u64 = 0;
const FIRST_INJECTION: usize = 1;
const REJECTED_BUTTON: usize = 2;
const THROUGH_MOVEMENT: usize = 4;
const MOVE_OFFSET: i32 = 50;
const FILE_CHANGE: &[u8] = b"external file modification";
type Callback = Box<dyn FnOnce() + Send>;

/// Desktop-effects substitute with an isolated real library; it never sends input to the owner's desktop.
struct Desktop {
    folder: Folder,
    id: String,
    events: Mutex<Vec<EventKind>>,
    releases: Mutex<Vec<Held>>,
    snapshots: Mutex<Vec<Snapshot>>,
    physical_hold: AtomicBool,
    environment_changed: AtomicBool,
    compact: AtomicBool,
    fail_injection: AtomicUsize,
    fail_release: AtomicBool,
    callback_at: AtomicUsize,
    on_entry: Mutex<Option<Callback>>,
    on_inject: Mutex<Option<Callback>>,
}
impl Desktop {
    /// Persists a known stream with nondefault properties; playback must snapshot it and execute only once at 1x.
    fn new(events: Vec<Event>, duration: Duration) -> Arc<Self> {
        let folder = Folder::new();
        let mut recording = Recording::new(test_support::environment()).expect("Fixture recording");
        let mut capture = Capture::default();
        capture.events = events;
        capture.duration = duration;
        recording.freeze(&capture);
        recording.document.playback.speed = NONDEFAULT_SPEED;
        recording.document.playback.repeat_mode = crate::macro_format::RepeatMode::Fixed;
        recording.document.playback.total_runs = NONDEFAULT_RUNS;
        folder
            .repository
            .save_recording(&mut recording)
            .expect("Fixture commit");
        Arc::new(Self {
            folder,
            id: recording.document.id,
            events: Mutex::new(Vec::new()),
            releases: Mutex::new(Vec::new()),
            snapshots: Mutex::new(Vec::new()),
            physical_hold: AtomicBool::new(false),
            environment_changed: AtomicBool::new(false),
            compact: AtomicBool::new(false),
            fail_injection: AtomicUsize::new(usize::MAX),
            fail_release: AtomicBool::new(false),
            callback_at: AtomicUsize::new(FIRST_INJECTION),
            on_entry: Mutex::new(None),
            on_inject: Mutex::new(None),
        })
    }
    /// Starts the same selected-ID, arrival-bound workflow invoked by real Play and double click.
    fn play(&self, session: &Arc<Session>) -> Result<Snapshot, String> {
        session.play_macro_requested(session.start_request()?.ok_or(ACTIVE_SESSION)?, &self.id)
    }
}
impl Platform for Desktop {
    /// Uses the real bounded decoder, ID association and external-change fingerprint guard.
    fn playback(&self, id: &str) -> Result<MacroDocument, String> {
        self.folder.repository.read(id)
    }
    /// Models a physical-held startup snapshot without changing the desktop.
    fn playback_ready(&self) -> Result<(), String> {
        if self.physical_hold.load(Ordering::Acquire) {
            Err(crate::playback::INPUT_HELD.into())
        } else {
            Ok(())
        }
    }
    /// Changes only when the fixture explicitly changes semantic layout; screen power is irrelevant.
    fn check_playback_environment(&self, _: &RecordingEnvironment) -> Result<(), String> {
        if self.environment_changed.load(Ordering::Acquire) {
            Err(crate::playback::ENVIRONMENT_CHANGED.into())
        } else {
            Ok(())
        }
    }
    /// Supplies external operation ownership; session phase/generation still enforce all session exclusion.
    fn reserve(&self) -> Result<Box<dyn Send>, String> {
        Ok(Box::new(()))
    }
    /// Library discovery is complete and no editor is open in this fixture.
    fn available(&self) -> bool {
        true
    }
    /// Makes both global shortcuts available without registering anything on Windows.
    fn shortcuts(&self, _: bool, _: bool) -> Result<Availability, String> {
        Ok(Availability {
            record: true,
            stop: true,
        })
    }
    /// Tracks placement and optionally exercises a real Stop/suspend while compact entry is still pending.
    fn view(&self, compact: bool) -> Result<(), String> {
        self.compact.store(compact, Ordering::Release);
        if compact {
            let callback = self.on_entry.lock().expect("Entry callback").take();
            if let Some(callback) = callback {
                callback();
            }
        }
        Ok(())
    }
    /// Uses fixture coordinates without reading the shared cursor.
    fn cursor(&self) -> (i32, i32) {
        (test_support::TEST_X, test_support::TEST_Y)
    }
    /// Records successful transitions and can reject a selected event without granting ownership of that transition.
    fn inject(&self, kind: &EventKind) -> Result<(), String> {
        let mut events = self.events.lock().expect("Injected events");
        if events.len() == self.fail_injection.load(Ordering::Acquire) {
            return Err(FAILURE.into());
        }
        events.push(kind.clone());
        let count = events.len();
        drop(events);
        if count == self.callback_at.load(Ordering::Acquire) {
            let callback = self.on_inject.lock().expect("Injection callback").take();
            if let Some(callback) = callback {
                callback();
            }
        }
        Ok(())
    }
    /// Retains failed releases for Retry and records only successful session-owned releases.
    fn release(&self, held: Held) -> Result<(), String> {
        if self.fail_release.load(Ordering::Acquire) {
            return Err(FAILURE.into());
        }
        self.releases.lock().expect("Owned releases").push(held);
        Ok(())
    }
    /// Observes only the public backend snapshot, including elapsed time and selected macro name.
    fn publish(&self, snapshot: Snapshot) {
        self.snapshots.lock().expect("Snapshots").push(snapshot);
    }
}
/// Waits through the public status boundary for worker cleanup, without inspecting session internals.
fn completed(session: &Session) -> Snapshot {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let snapshot = session.snapshot().expect("Session snapshot");
        if snapshot.phase == Phase::Idle || snapshot.cleanup_failed {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "Playback cleanup timed out");
        std::thread::sleep(POLL);
    }
}
/// Returns a representative ordered held-drag/wheel stream, including repeated keys and equal timestamps.
fn events() -> Vec<Event> {
    [
        test_support::key(true),
        test_support::key(true),
        EventKind::Mouse {
            button: MouseButton::Left,
            down: true,
            x: test_support::TEST_X,
            y: test_support::TEST_Y,
        },
        EventKind::Move {
            x: test_support::TEST_X + MOVE_OFFSET,
            y: test_support::TEST_Y,
        },
        EventKind::Wheel {
            delta: WHEEL,
            x: test_support::TEST_X,
            y: test_support::TEST_Y,
        },
        test_support::key(false),
    ]
    .into_iter()
    .map(|kind| Event {
        at: Duration::ZERO,
        kind,
    })
    .collect()
}

#[test]
/// A selected saved stream replays verbatim once, leaves properties untouched and releases an end-held drag before restoring the library.
fn saved_playback_runs_once_and_releases_end_held_input() {
    let source = events();
    let desktop = Desktop::new(source.clone(), END);
    let session = Session::new(desktop.clone());
    desktop.play(&session).expect("Selected playback starts");
    let final_state = completed(&session);
    assert_eq!(final_state.message, crate::playback::COMPLETE);
    assert!(!final_state.unsaved);
    assert!(desktop
        .snapshots
        .lock()
        .expect("Snapshots")
        .iter()
        .any(|snapshot| snapshot.phase == Phase::Playing
            && snapshot.macro_name.is_some()
            && !snapshot.unsaved));
    assert_eq!(
        *desktop.events.lock().expect("Events"),
        source
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        *desktop.releases.lock().expect("Releases"),
        [Held::Button(MouseButton::Left)]
    );
    assert!(!desktop.compact.load(Ordering::Acquire));
    let persisted = desktop
        .folder
        .repository
        .read(&desktop.id)
        .expect("Original file");
    assert_eq!(persisted.playback.speed, NONDEFAULT_SPEED);
    assert_eq!(persisted.playback.total_runs, NONDEFAULT_RUNS);
}

#[test]
/// Held physical input and changed environments refuse startup before ordinary dispatch; rejection never retries automatically.
fn saved_playback_rejects_interfering_startup_state() {
    let desktop = Desktop::new(events(), END);
    let session = Session::new(desktop.clone());
    desktop.physical_hold.store(true, Ordering::Release);
    assert_eq!(
        desktop.play(&session).err().as_deref(),
        Some(crate::playback::INPUT_HELD)
    );
    desktop.physical_hold.store(false, Ordering::Release);
    desktop.environment_changed.store(true, Ordering::Release);
    assert_eq!(
        desktop.play(&session).err().as_deref(),
        Some(crate::playback::ENVIRONMENT_CHANGED)
    );
    assert!(desktop.events.lock().expect("Events").is_empty());
    assert_eq!(session.snapshot().expect("Snapshot").phase, Phase::Idle);
}

#[test]
/// A partial positioned-action rejection stops later actions and retains prior holds until cleanup succeeds.
fn injection_failure_retains_owned_cleanup_for_retry() {
    let desktop = Desktop::new(events(), END);
    let session = Session::new(desktop.clone());
    desktop
        .fail_injection
        .store(REJECTED_BUTTON, Ordering::Release);
    desktop.fail_release.store(true, Ordering::Release);
    desktop.play(&session).expect("Start");
    let failed = completed(&session);
    assert_eq!(failed.phase, Phase::Stopping);
    assert!(failed.cleanup_failed);
    assert!(desktop.play(&session).is_err());
    desktop.fail_release.store(false, Ordering::Release);
    let stopped = session.stop().expect("Retry cleanup");
    assert!(stopped.message.contains(FAILURE));
    assert_eq!(
        desktop.events.lock().expect("Events").len(),
        REJECTED_BUTTON
    );
    assert_eq!(
        *desktop.releases.lock().expect("Releases"),
        [Held::Key(
            test_support::TEST_KEY,
            test_support::TEST_SCAN,
            false
        )]
    );
}

#[test]
/// Stop or system sleep during compact entry dispatches nothing, and elapsed time stays zero throughout preparation.
fn cancelled_compact_entry_never_starts_recorded_time_or_input() {
    for interrupted in [false, true] {
        let desktop = Desktop::new(events(), END);
        let session = Session::new(desktop.clone());
        session.record().expect("Previous temporary recording");
        session.capture(test_support::key(true));
        session.capture(test_support::key(false));
        let retained = session.stop().expect("Retain previous capture");
        let weak = Arc::downgrade(&session);
        *desktop.on_entry.lock().expect("Entry") = Some(Box::new(move || {
            let session = weak.upgrade().expect("Session owner");
            assert_eq!(
                session.snapshot().expect("Preparing snapshot").elapsed_ms,
                PREPARATION_ELAPSED_MS
            );
            session.signal_stop(interrupted).expect("Cancel startup");
        }));
        desktop.play(&session).expect("Cancelled start completes");
        let stopped = completed(&session);
        assert_eq!(stopped.phase, Phase::Idle);
        assert_eq!(stopped.event_count, retained.event_count);
        assert!(stopped.unsaved);
        assert!(desktop.events.lock().expect("Events").is_empty());
    }
}

#[test]
/// Stop and an actual layout change cancel a long trailing hold, preserving the last replayed position for cleanup.
fn active_drag_stops_on_stop_or_detected_environment_change() {
    for changed in [false, true] {
        let desktop = Desktop::new(events(), LONG_WAIT);
        let session = Session::new(desktop.clone());
        let weak = Arc::downgrade(&session);
        let effects = desktop.clone();
        desktop
            .callback_at
            .store(THROUGH_MOVEMENT, Ordering::Release);
        *desktop.on_inject.lock().expect("Injection") = Some(Box::new(move || {
            let session = weak.upgrade().expect("Session owner");
            if changed {
                effects.environment_changed.store(true, Ordering::Release);
                session.refresh().expect("Detect changed layout");
            } else {
                session.signal_stop(false).expect("Global Stop");
            }
        }));
        desktop.play(&session).expect("Start");
        let stopped = completed(&session);
        assert_eq!(
            desktop.events.lock().expect("Events").len(),
            THROUGH_MOVEMENT
        );
        assert!(!stopped.message.contains(crate::playback::COMPLETE));
        assert_eq!(
            *desktop.releases.lock().expect("Releases"),
            [
                Held::Button(MouseButton::Left),
                Held::Key(test_support::TEST_KEY, test_support::TEST_SCAN, false)
            ]
        );
    }
}

#[test]
/// A file changed after startup cannot alter the owned run; a later start rejects the changed backing file.
fn session_snapshot_is_stable_and_next_start_checks_file_fingerprint() {
    let desktop = Desktop::new(events(), END);
    let session = Session::new(desktop.clone());
    let path = std::fs::read_dir(&desktop.folder.path)
        .expect("Fixture folder")
        .next()
        .expect("Saved file")
        .expect("File entry")
        .path();
    *desktop.on_inject.lock().expect("Injection") = Some(Box::new(move || {
        std::fs::write(path, FILE_CHANGE).expect("External edit after snapshot");
    }));
    desktop.play(&session).expect("Start");
    assert_eq!(completed(&session).message, crate::playback::COMPLETE);
    assert_eq!(desktop.events.lock().expect("Events").len(), events().len());
    assert!(desktop.play(&session).is_err());
}

#[test]
/// Both Stop and actual system sleep cancel an initial wait with no dispatch or overdue backlog.
fn stop_and_sleep_cancel_saved_initial_wait() {
    for interrupted in [false, true] {
        let mut source = events();
        for event in &mut source {
            event.at = LONG_WAIT;
        }
        let desktop = Desktop::new(source, LONG_WAIT);
        let session = Session::new(desktop.clone());
        desktop.play(&session).expect("Begin initial wait");
        if interrupted {
            session.suspend().expect("System sleep");
        } else {
            session.stop().expect("Stop");
        }
        assert_eq!(completed(&session).phase, Phase::Idle);
        assert!(desktop.events.lock().expect("Events").is_empty());
        assert!(desktop.releases.lock().expect("Releases").is_empty());
    }
}

#[test]
/// A hold begun during compact entry rejects startup without destroying a previously retained temporary recording.
fn failed_final_preflight_preserves_previous_capture() {
    let desktop = Desktop::new(events(), END);
    let session = Session::new(desktop.clone());
    session.record().expect("Previous temporary recording");
    session.capture(test_support::key(true));
    let retained = session.stop().expect("Retain previous capture");
    let effects = desktop.clone();
    *desktop.on_entry.lock().expect("Entry") = Some(Box::new(move || {
        effects.physical_hold.store(true, Ordering::Release);
    }));
    assert_eq!(
        desktop.play(&session).err().as_deref(),
        Some(crate::playback::INPUT_HELD)
    );
    let stopped = completed(&session);
    assert_eq!(stopped.event_count, retained.event_count);
    assert!(stopped.unsaved);
    assert!(desktop.events.lock().expect("Events").is_empty());
}
