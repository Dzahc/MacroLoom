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
const PREPARATION_WAIT: Duration = Duration::from_millis(10);
const FIRST_INJECTION: usize = 1;
const REJECTED_BUTTON: usize = 2;
const THROUGH_MOVEMENT: usize = 4;
const MOVE_OFFSET: i32 = -50;
const FILE_CHANGE: &[u8] = b"external file modification";
const OUTSIDE_DISPLAY_X: i32 = 0;
const GAP_X: i32 = 50;
const SECOND_DISPLAY_X: i32 = 100;
const SECOND_DISPLAY_Y: i32 = -1080;
const LAST_SECOND_X: i32 = 2019;
const LAST_SECOND_Y: i32 = -1;
const LAST_FIRST_X: i32 = -1;
const LAST_FIRST_Y: i32 = 1079;
const DISPLAY_SCALES: [f64; 4] = [1.0, 1.25, 1.5, 1.75];
const RAPID_REPEAT_RUNS: usize = 128;
const HELD_RELEASE_COUNT: usize = 2;
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
    on_rejection: Mutex<Option<Callback>>,
}
impl Desktop {
    /// Persists a known stream with nondefault speed/repeat properties for the real selected-ID command boundary.
    fn new(events: Vec<Event>, duration: Duration) -> Arc<Self> {
        Self::with_environment(events, duration, test_support::environment())
    }
    /// Saves fixture events against explicit original monitor surfaces, without observing the user's current desktop.
    fn with_environment(
        events: Vec<Event>,
        duration: Duration,
        environment: RecordingEnvironment,
    ) -> Arc<Self> {
        let folder = Folder::new();
        let mut recording = Recording::new(environment).expect("Fixture recording");
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
            on_rejection: Mutex::new(None),
        })
    }
    /// Starts the same selected-ID, arrival-bound workflow invoked by real Play and double click.
    fn play(&self, session: &Arc<Session>) -> Result<Snapshot, String> {
        session.play_macro_requested(session.start_request()?.ok_or(ACTIVE_SESSION)?, &self.id)
    }
    /// Commits fixture properties through the repository's real validation and atomic property-save interface.
    fn configure(&self, change: impl FnOnce(&mut crate::macro_format::PlaybackProperties)) {
        let mut document = self
            .folder
            .repository
            .read(&self.id)
            .expect("Saved fixture");
        change(&mut document.playback);
        self.folder
            .repository
            .save_properties(&self.id, &document.name, &document.playback)
            .expect("Configured fixture");
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
            drop(events);
            let callback = self
                .on_rejection
                .lock()
                .expect("Injection rejection callback")
                .take();
            if let Some(callback) = callback {
                callback();
            }
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
    wait_snapshot(session, |snapshot| {
        snapshot.phase == Phase::Idle || snapshot.cleanup_failed
    })
}
/// Polls the public command snapshot for an expected lifecycle observation without relying on progress-event frequency.
fn wait_snapshot(session: &Session, expected: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let snapshot = session.snapshot().expect("Session snapshot");
        if expected(&snapshot) {
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

/// Returns the expected verbatim stream for the fixture's fixed total, independently of scheduling implementation.
fn repeated_kinds(source: &[Event]) -> Vec<EventKind> {
    source
        .iter()
        .cycle()
        .take(source.len() * NONDEFAULT_RUNS as usize)
        .map(|event| event.kind.clone())
        .collect()
}

#[test]
/// Saved fixed-count playback repeats its stable stream and releases each end-held drag without changing properties.
fn saved_playback_repeats_and_releases_each_run_held_input() {
    let source = events();
    let desktop = Desktop::new(source.clone(), END);
    let session = Session::new(desktop.clone());
    desktop.play(&session).expect("Selected playback starts");
    let final_state = completed(&session);
    assert_eq!(final_state.message, crate::playback::COMPLETE);
    assert!(!final_state.unsaved);
    assert_eq!(final_state.run, crate::playback::FIRST_RUN);
    assert_eq!(final_state.total_runs, Some(crate::playback::FIRST_RUN));
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
        repeated_kinds(&source)
    );
    assert_eq!(
        *desktop.releases.lock().expect("Releases"),
        (0..NONDEFAULT_RUNS)
            .map(|_| Held::Button(MouseButton::Left))
            .collect::<Vec<_>>()
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
/// An unreachable later mouse target refuses the whole run before an earlier key can dispatch, preserving the valid file.
fn invalid_saved_target_prevents_even_the_keyboard_prefix() {
    let source = vec![
        Event {
            at: Duration::ZERO,
            kind: test_support::key(true),
        },
        Event {
            at: END,
            kind: EventKind::Mouse {
                button: MouseButton::Left,
                down: true,
                x: OUTSIDE_DISPLAY_X,
                y: test_support::TEST_Y,
            },
        },
    ];
    let desktop = Desktop::new(source, END);
    let original = desktop
        .folder
        .repository
        .read(&desktop.id)
        .expect("Valid saved macro");
    let session = Session::new(desktop.clone());
    assert!(desktop.play(&session).is_err());
    assert!(desktop.events.lock().expect("Events").is_empty());
    assert!(!desktop.compact.load(Ordering::Acquire));
    assert_eq!(session.snapshot().expect("Snapshot").phase, Phase::Idle);
    let retained = desktop
        .folder
        .repository
        .read(&desktop.id)
        .expect("Preserved macro");
    assert_eq!(
        serde_json::to_value(original).expect("Original document"),
        serde_json::to_value(retained).expect("Retained document")
    );
}

/// Creates two offset monitor surfaces with a real gap inside their shared virtual-desktop rectangle.
fn offset_displays(scale: f64) -> RecordingEnvironment {
    let mut environment = test_support::environment();
    environment.displays.push(crate::macro_format::Display {
        x: SECOND_DISPLAY_X,
        y: SECOND_DISPLAY_Y,
        width: test_support::TEST_WIDTH,
        height: test_support::TEST_HEIGHT,
        scale_factor: scale,
        extra: Default::default(),
    });
    environment
}

/// Builds a valid held-button prefix followed by one positioned action, retaining a keyboard event before the target.
fn positioned_source(kind: EventKind) -> Vec<Event> {
    [
        test_support::key(true),
        EventKind::Mouse {
            button: MouseButton::Left,
            down: true,
            x: test_support::TEST_X,
            y: test_support::TEST_Y,
        },
        kind,
    ]
    .into_iter()
    .map(|kind| Event {
        at: Duration::ZERO,
        kind,
    })
    .collect()
}

/// Provides all positioned v1 variants so no release, drag point or wheel can bypass whole-run targeting validation.
fn positioned_kinds(x: i32, y: i32) -> [EventKind; 4] {
    [
        EventKind::Mouse {
            button: MouseButton::Left,
            down: true,
            x,
            y,
        },
        EventKind::Mouse {
            button: MouseButton::Left,
            down: false,
            x,
            y,
        },
        EventKind::Move { x, y },
        EventKind::Wheel { delta: WHEEL, x, y },
    ]
}

#[test]
/// Every positioned event rejects gaps, exclusive monitor edges and extreme signed coordinates before any keyboard or button dispatch.
fn all_mouse_targets_reject_unreachable_recorded_pixels() {
    for (x, y) in [
        (GAP_X, test_support::TEST_Y),
        (OUTSIDE_DISPLAY_X, test_support::TEST_Y),
        (SECOND_DISPLAY_X, 0),
        (i32::MIN, test_support::TEST_Y),
        (test_support::TEST_X, i32::MAX),
    ] {
        for kind in positioned_kinds(x, y) {
            let desktop = Desktop::with_environment(
                positioned_source(kind),
                END,
                offset_displays(test_support::TEST_SCALE),
            );
            let session = Session::new(desktop.clone());
            let error = desktop
                .play(&session)
                .err()
                .expect("Unreachable target blocks startup");
            assert!(error.contains(crate::playback::INVALID_TARGET));
            assert!(desktop.events.lock().expect("Events").is_empty());
            assert!(desktop.releases.lock().expect("Releases").is_empty());
        }
    }
}

#[test]
/// Inclusive origins and last physical pixels replay verbatim on negative/offset monitors at supported scale factors.
fn valid_monitor_pixels_replay_without_dpi_remapping() {
    for scale in DISPLAY_SCALES {
        for (x, y) in [
            (test_support::TEST_DISPLAY_X, 0),
            (LAST_FIRST_X, LAST_FIRST_Y),
            (SECOND_DISPLAY_X, SECOND_DISPLAY_Y),
            (LAST_SECOND_X, LAST_SECOND_Y),
        ] {
            let source = positioned_source(EventKind::Wheel { delta: WHEEL, x, y });
            let desktop =
                Desktop::with_environment(source.clone(), Duration::ZERO, offset_displays(scale));
            let session = Session::new(desktop.clone());
            desktop
                .play(&session)
                .expect("Valid physical targets start");
            assert_eq!(completed(&session).message, crate::playback::COMPLETE);
            assert_eq!(
                *desktop.events.lock().expect("Events"),
                repeated_kinds(&source)
            );
        }
    }
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
/// Suspend received inside a native injection must remain visible when that injection later rejects and cleanup finishes.
fn suspend_during_rejected_injection_preserves_interruption_context() {
    for failed_cleanup in [false, true] {
        let desktop = Desktop::new(events(), END);
        let session = Session::new(desktop.clone());
        desktop
            .fail_injection
            .store(FIRST_INJECTION, Ordering::Release);
        desktop
            .fail_release
            .store(failed_cleanup, Ordering::Release);
        let weak = Arc::downgrade(&session);
        *desktop.on_rejection.lock().expect("Suspension fixture") = Some(Box::new(
            // Signals cancellation during the real platform injection call, before its failure reaches the playback worker.
            move || {
                let session = weak.upgrade().expect("Playback owner");
                session.signal_stop(true).expect("Suspend during injection");
                assert!(session
                    .snapshot()
                    .expect("Suspend received")
                    .message
                    .contains(SUSPEND_INTERRUPTED));
            },
        ));
        desktop.play(&session).expect("Start playback");
        let mut stopped = completed(&session);
        if failed_cleanup {
            assert!(stopped.cleanup_failed);
            assert!(stopped.message.contains(SUSPEND_INTERRUPTED));
            desktop.fail_release.store(false, Ordering::Release);
            stopped = session.stop().expect("Retry owned-input cleanup");
        }
        assert_eq!(stopped.phase, Phase::Idle);
        assert!(stopped.message.contains(FAILURE));
        assert!(
            stopped.message.contains(REPLAY_INTERRUPTED),
            "Suspension context lost: {}",
            stopped.message
        );
        assert_eq!(
            session.refresh().expect("Wake refresh").message,
            stopped.message
        );
        assert_eq!(
            desktop.events.lock().expect("Accepted events").len(),
            FIRST_INJECTION
        );
        assert_eq!(
            *desktop.releases.lock().expect("Owned releases"),
            [Held::Key(
                test_support::TEST_KEY,
                test_support::TEST_SCAN,
                false
            )]
        );
    }
}

#[test]
/// Preparation contributes to session elapsed time, but Stop/sleep still prevents every recorded event from starting.
fn cancelled_compact_entry_counts_preparation_without_dispatching_input() {
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
            std::thread::sleep(PREPARATION_WAIT);
            let preparing = session.snapshot().expect("Preparing snapshot");
            assert!(preparing.elapsed_ms >= PREPARATION_WAIT.as_millis() as u64);
            assert_eq!(preparing.playback_stage, PlaybackStage::Starting);
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
    assert_eq!(
        desktop.events.lock().expect("Events").len(),
        events().len() * NONDEFAULT_RUNS as usize
    );
    assert!(desktop.play(&session).is_err());
}

#[test]
/// Stop and simulated suspend cancel an initial wait without dispatch; interruption remains identifiable after refresh.
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
        let stopped = completed(&session);
        assert_eq!(stopped.phase, Phase::Idle);
        if interrupted {
            assert_eq!(stopped.message, REPLAY_INTERRUPTED);
            assert_eq!(
                session.refresh().expect("Wake refresh").message,
                stopped.message
            );
        } else {
            assert_eq!(stopped.message, REPLAY_CANCELLED);
        }
        assert!(desktop.events.lock().expect("Events").is_empty());
        assert!(desktop.releases.lock().expect("Releases").is_empty());
    }
}

#[test]
/// Close during held key/drag playback retains exit and failed holds until explicit Retry, even across refresh/repeated Close/suspend.
fn close_during_held_playback_waits_for_explicit_cleanup_retry() {
    let desktop = Desktop::new(events(), LONG_WAIT);
    desktop.fail_release.store(true, Ordering::Release);
    desktop
        .callback_at
        .store(THROUGH_MOVEMENT, Ordering::Release);
    let session = Session::new(desktop.clone());
    let weak = Arc::downgrade(&session);
    *desktop.on_inject.lock().expect("Close fixture") = Some(Box::new(move || {
        weak.upgrade()
            .expect("Playback owner")
            .request_close()
            .expect("Close during drag");
    }));
    desktop.play(&session).expect("Playback starts");
    let failed = completed(&session);
    assert!(failed.cleanup_failed && failed.close_requested);
    desktop.fail_release.store(false, Ordering::Release);
    assert!(
        session
            .refresh()
            .expect("Refresh is not Retry")
            .cleanup_failed
    );
    session
        .request_close()
        .expect("Repeated Close is not Retry");
    assert!(
        session
            .suspend()
            .expect("Sleep is not Retry")
            .cleanup_failed
    );
    assert!(!session.finish_exit().expect("No exit before release"));
    let stopped = session.stop().expect("Explicit cleanup Retry");
    assert!(stopped.close_requested && !stopped.unsaved && !stopped.cleanup_failed);
    assert_eq!(
        desktop.events.lock().expect("Ordinary events").len(),
        THROUGH_MOVEMENT
    );
    assert_eq!(
        desktop.releases.lock().expect("Released owned holds").len(),
        HELD_RELEASE_COUNT
    );
    assert!(session.finish_exit().expect("Exit after cleanup"));
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

#[test]
/// Once ignores inactive counts and intervals; a fixed total of one also completes without a final interval.
fn single_run_modes_ignore_the_final_interval() {
    use crate::macro_format::RepeatMode;
    for mode in [RepeatMode::Once, RepeatMode::Fixed] {
        let desktop = Desktop::new(events(), Duration::ZERO);
        desktop.configure(|properties| {
            properties.total_runs = crate::playback::FIRST_RUN;
            properties.repeat_mode = mode;
            properties.interval_ms = LONG_WAIT.as_millis() as u64;
        });
        let session = Session::new(desktop.clone());
        desktop.play(&session).expect("One run");
        assert_eq!(completed(&session).message, crate::playback::COMPLETE);
        assert_eq!(desktop.events.lock().expect("Events").len(), events().len());
        assert!(!desktop
            .snapshots
            .lock()
            .expect("Snapshots")
            .iter()
            .any(|state| state.phase == Phase::Interval));
    }
}

#[test]
/// Stop, suspend, Close and layout changes during an interval cancel the next run after the previous run has released its drag.
fn interval_cancellation_keeps_compact_ownership_until_cleanup() {
    const STOP: usize = 0;
    const SUSPEND: usize = 1;
    const ENVIRONMENT: usize = 2;
    const CLOSE: usize = 3;
    for reason in [STOP, SUSPEND, ENVIRONMENT, CLOSE] {
        let desktop = Desktop::new(events(), Duration::ZERO);
        desktop.configure(|properties| properties.interval_ms = LONG_WAIT.as_millis() as u64);
        let session = Session::new(desktop.clone());
        desktop.play(&session).expect("Repeated playback");
        let state = wait_snapshot(&session, |snapshot| snapshot.phase == Phase::Interval);
        assert_eq!(state.phase, Phase::Interval);
        assert_eq!(state.run, crate::playback::FIRST_RUN);
        assert_eq!(state.total_runs, Some(NONDEFAULT_RUNS));
        assert!(state.remaining_ms > 0);
        assert!(desktop.compact.load(Ordering::Acquire));
        assert_eq!(
            *desktop.releases.lock().expect("Released drag"),
            [Held::Button(MouseButton::Left)]
        );
        if reason == ENVIRONMENT {
            desktop.environment_changed.store(true, Ordering::Release);
            session.refresh().expect("Observe changed environment");
        } else if reason == CLOSE {
            session.request_close().expect("Close during interval");
        } else {
            session
                .signal_stop(reason == SUSPEND)
                .expect("Cancel interval");
        }
        let stopped = completed(&session);
        assert_eq!(stopped.phase, Phase::Idle);
        assert!(!stopped.message.contains(crate::playback::COMPLETE));
        assert_eq!(desktop.events.lock().expect("Events").len(), events().len());
        assert!(!desktop.compact.load(Ordering::Acquire));
    }
}

#[test]
/// Failed run-end release blocks every repeat and retains the same actionable terminal cleanup ownership.
fn failed_run_release_prevents_another_run_or_interval() {
    let desktop = Desktop::new(events(), Duration::ZERO);
    desktop.fail_release.store(true, Ordering::Release);
    let session = Session::new(desktop.clone());
    desktop.play(&session).expect("First run starts");
    let failed = completed(&session);
    assert_eq!(failed.phase, Phase::Stopping);
    assert!(failed.cleanup_failed);
    assert_eq!(desktop.events.lock().expect("Events").len(), events().len());
    desktop.fail_release.store(false, Ordering::Release);
    assert_eq!(session.stop().expect("Retry release").phase, Phase::Idle);
}

#[test]
/// Fixed safe-integer totals and infinity survive command snapshots without u32 truncation, and both remain stoppable.
fn large_and_indefinite_totals_are_exact_in_active_snapshots() {
    use crate::macro_format::{RepeatMode, MAX_SAFE_INTEGER};
    for (mode, expected) in [
        (RepeatMode::Fixed, Some(MAX_SAFE_INTEGER)),
        (RepeatMode::Indefinite, None),
    ] {
        let desktop = Desktop::new(events(), Duration::ZERO);
        desktop.configure(|properties| {
            properties.repeat_mode = mode;
            properties.total_runs = MAX_SAFE_INTEGER;
        });
        let session = Session::new(desktop.clone());
        let weak = Arc::downgrade(&session);
        *desktop.on_inject.lock().expect("Injection") = Some(Box::new(move || {
            let session = weak.upgrade().expect("Session owner");
            assert_eq!(
                session.snapshot().expect("Active snapshot").total_runs,
                expected
            );
            session.signal_stop(false).expect("Stop first run");
        }));
        desktop.play(&session).expect("Start saved plan");
        assert_eq!(completed(&session).phase, Phase::Idle);
        assert_eq!(
            desktop.events.lock().expect("Events").len(),
            FIRST_INJECTION
        );
    }
}

#[test]
/// Rapid repeats preserve backend cancellation without flooding the public progress stream between status ticks.
fn zero_duration_repeats_bound_progress_publication_without_delaying_input() {
    let source = vec![Event {
        at: Duration::ZERO,
        kind: test_support::key(true),
    }];
    let desktop = Desktop::new(source, Duration::ZERO);
    desktop.configure(|properties| {
        properties.repeat_mode = crate::macro_format::RepeatMode::Indefinite
    });
    desktop
        .callback_at
        .store(RAPID_REPEAT_RUNS, Ordering::Release);
    let session = Session::new(desktop.clone());
    let weak = Arc::downgrade(&session);
    *desktop.on_inject.lock().expect("Injection") = Some(Box::new(move || {
        weak.upgrade()
            .expect("Session owner")
            .signal_stop(false)
            .expect("Stop rapid repeats");
    }));
    desktop.play(&session).expect("Start rapid repeats");
    assert_eq!(completed(&session).phase, Phase::Idle);
    let snapshots = desktop.snapshots.lock().expect("Snapshots");
    let elapsed = snapshots
        .iter()
        .map(|snapshot| snapshot.elapsed_ms)
        .max()
        .unwrap_or_default();
    let progress = snapshots
        .iter()
        .filter(|snapshot| {
            snapshot.phase == Phase::Playing && snapshot.playback_stage != PlaybackStage::Starting
        })
        .count() as u64;
    let maximum = elapsed / STATUS_INTERVAL.as_millis() as u64 + crate::playback::FIRST_RUN;
    assert!(
        progress <= maximum,
        "Unbounded progress: {progress} updates in {elapsed} ms"
    );
    assert_eq!(
        desktop.events.lock().expect("Events").len(),
        RAPID_REPEAT_RUNS
    );
}
