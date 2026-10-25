//! Behavior tests through session commands and snapshots, substituting only desktop effects.
use crate::input::{EventKind, Held};
use crate::session::{Phase, Platform, Session, Snapshot, ACTIVE_SESSION};
use crate::session_control::ControlRequest;
use crate::shortcuts::Availability;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CONFIRMATION_OWNER: &str = "session-test-owner";
const TEST_KEY: u16 = 0x41;
const TEST_SCAN: u16 = 0x1e;
const REJECTED: &str = "Injected native failure";
const EXPECTED_IDLE_MESSAGE: &str = "Ready";
const OTHER_FAILURE: &str = "Different command failure";
const TEST_DEADLINE: Duration = Duration::from_secs(2);
const TEST_POLL: Duration = Duration::from_millis(1);
const STARTUP_CANCEL_CONTROLS: [ControlRequest; 2] =
    [ControlRequest::Stop, ControlRequest::Suspend];
const STARTUP_RECORD_STOP_CONTROLS: [ControlRequest; 2] =
    [ControlRequest::Record, ControlRequest::Stop];

#[test]
/// Idle snapshots leave shortcut instructions to the presentation's dedicated hint line.
fn idle_message_does_not_repeat_shortcut_instructions() {
    let session = Session::new(Arc::new(Desktop::default()));
    assert_eq!(
        session.snapshot().expect("Initial idle snapshot").message,
        EXPECTED_IDLE_MESSAGE
    );
}

#[test]
/// Close waits for deletion's disk result; failed deletion cancels exit and success retains the exit request.
fn close_during_deletion_waits_and_failure_cancels_exit() {
    for succeeds in [false, true] {
        let session = Session::new(Arc::new(Desktop::default()));
        session
            .confirmation(CONFIRMATION_OWNER, true)
            .expect("Confirm deletion");
        let result = session.delete(Some(CONFIRMATION_OWNER), || {
            assert!(session.request_close().expect("Defer Close"));
            assert!(session
                .start_request()
                .expect("Start eligibility")
                .is_none());
            if succeeds {
                Ok(())
            } else {
                Err(REJECTED.to_owned())
            }
        });
        assert_eq!(result.is_ok(), succeeds);
        let snapshot = session.snapshot().expect("Deletion result");
        assert_eq!(snapshot.close_requested, succeeds);
        assert_eq!(snapshot.can_record, !succeeds);
    }
}

#[derive(Default)]
/// A deterministic desktop-effect substitute; never replaces session state or lifecycle logic.
struct Desktop {
    shutdown_failure: AtomicBool,
    shortcut_failure: AtomicBool,
    stop_conflict: AtomicBool,
    key_up_failure: AtomicBool,
    release_failure: AtomicBool,
    restore_failure: AtomicBool,
    snapshots: Mutex<Vec<Snapshot>>,
    pressed: AtomicBool,
    entry_failure: AtomicBool,
    on_entry: Mutex<Option<Box<dyn Fn() + Send>>>,
    operation: Arc<AtomicBool>,
    registered: Mutex<Availability>,
    on_inject: Mutex<Option<Box<dyn Fn() + Send>>>,
    unregister_failure: AtomicBool,
    on_shortcuts: Mutex<Option<Box<dyn Fn() + Send>>>,
    injections: std::sync::atomic::AtomicUsize,
}
impl Platform for Desktop {
    /// Models an acknowledged native teardown, retaining failure until the external blocker is removed.
    fn shutdown(&self) -> Result<(), String> {
        if self.shutdown_failure.load(Ordering::Acquire) {
            Err(REJECTED.into())
        } else {
            Ok(())
        }
    }
    /// Returns an owned external-operation token, with no native input effects.
    fn reserve(&self) -> Result<Box<dyn Send>, String> {
        self.operation
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| ACTIVE_SESSION.to_owned())?;
        Ok(Box::new(DesktopReservation(self.operation.clone())))
    }
    /// The fixture library is ready and no editor is active.
    fn available(&self) -> bool {
        !self.operation.load(Ordering::Acquire)
    }
    /// Reports a controllable native Stop conflict while retaining Record availability.
    fn shortcuts(&self, record: bool, stop: bool) -> Result<Availability, String> {
        if self.shortcut_failure.load(Ordering::Acquire) {
            return Err(REJECTED.into());
        }
        if !stop
            && self.registered.lock().expect("Fixture reservations").stop
            && self.unregister_failure.load(Ordering::Acquire)
        {
            return Err(REJECTED.into());
        }
        *self.registered.lock().expect("Fixture reservation mutex") = Availability {
            record,
            stop: stop && !self.stop_conflict.load(Ordering::Acquire),
        };
        if stop {
            let callback = self
                .on_shortcuts
                .lock()
                .expect("Fixture shortcut callback")
                .take();
            if let Some(callback) = callback {
                callback();
            }
        }
        Ok(Availability {
            record,
            stop: !self.stop_conflict.load(Ordering::Acquire),
        })
    }
    /// Simulates successful entry and a controllable restoration failure.
    fn view(&self, compact: bool) -> Result<(), String> {
        if compact {
            if let Some(callback) = self
                .on_entry
                .lock()
                .expect("Fixture callback mutex")
                .as_ref()
            {
                callback();
            }
            if self.entry_failure.load(Ordering::Acquire) {
                return Err(REJECTED.into());
            }
        }
        if !compact && self.restore_failure.load(Ordering::Acquire) {
            Err(REJECTED.into())
        } else {
            Ok(())
        }
    }
    /// Returns one valid physical desktop position.
    fn cursor(&self) -> (i32, i32) {
        (0, 0)
    }
    /// Can reject key-up after a successful key-down to leave session-owned held input.
    fn inject(&self, kind: &EventKind) -> Result<(), String> {
        self.injections.fetch_add(1, Ordering::AcqRel);
        let callback = self
            .on_inject
            .lock()
            .expect("Fixture callback mutex")
            .take();
        if let Some(callback) = callback {
            callback();
        }
        if let EventKind::Key { down: false, .. } = kind {
            if self.key_up_failure.load(Ordering::Acquire) {
                return Err(REJECTED.into());
            }
        }
        if let EventKind::Key { down, .. } = kind {
            self.pressed.store(*down, Ordering::Release);
        }
        Ok(())
    }
    /// Can reject native release until the external blocker is removed.
    fn release(&self, _: Held) -> Result<(), String> {
        if self.release_failure.load(Ordering::Acquire) {
            return Err(REJECTED.into());
        }
        self.pressed.store(false, Ordering::Release);
        Ok(())
    }
    /// Retains public event snapshots for assertions about observable states.
    fn publish(&self, snapshot: Snapshot) {
        self.snapshots
            .lock()
            .expect("Fixture mutex must be healthy")
            .push(snapshot);
    }
}

#[test]
/// Even idle close waits for native teardown; failures require explicit cleanup retry and cannot be bypassed by discard/Keep open.
fn close_waits_for_acknowledged_shutdown_and_retry() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    desktop.shutdown_failure.store(true, Ordering::Release);
    assert!(session.request_close().expect("Defer idle close"));
    assert!(!session.finish_exit().expect("Retain teardown failure"));
    let failed = session.snapshot().expect("Recovery snapshot");
    assert!(failed.close_requested && failed.cleanup_failed);
    assert!(session.keep_open().is_err());
    assert!(session.discard_exit().is_err());
    assert!(session.record().is_err());
    desktop.shutdown_failure.store(false, Ordering::Release);
    assert!(!session.finish_exit().expect("No implicit retry"));
    session.stop().expect("Explicit Retry/F8");
    assert!(session.finish_exit().expect("Teardown acknowledged"));
}

#[test]
/// Automatic shortcut availability updates after wake supplement the sleep explanation instead of erasing it.
fn wake_shortcut_conflict_preserves_the_interruption_banner() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture(key(true));
    let interrupted = session.suspend().expect("Interrupt recording");
    desktop.stop_conflict.store(true, Ordering::Release);
    let conflict = session.refresh().expect("Wake with conflict");
    assert!(conflict.message.contains(&interrupted.message));
    assert!(conflict
        .message
        .contains(crate::shortcuts::STOP_UNAVAILABLE));
    desktop.stop_conflict.store(false, Ordering::Release);
    assert_eq!(
        session.refresh().expect("Availability recovers").message,
        interrupted.message
    );
}

#[test]
/// Automatic native shortcut errors must supplement the interruption outcome and disappear after native recovery.
fn wake_shortcut_error_preserves_the_interruption_outcome() {
    for playback in [false, true] {
        let desktop = Arc::new(Desktop::default());
        let session = Session::new(desktop.clone());
        session.record().expect("Record before suspend");
        session.capture(key(true));
        let interrupted = if playback {
            session.stop().expect("Retain playback capture");
            let weak = Arc::downgrade(&session);
            *desktop.on_inject.lock().expect("Suspend fixture") = Some(Box::new(
                // Signals suspension while the actual playback worker still owns dispatch and held-input cleanup.
                move || {
                    weak.upgrade()
                        .expect("Playback owner")
                        .signal_stop(true)
                        .expect("Suspend playback")
                },
            ));
            session.play().expect("Start playback");
            completed(&session)
        } else {
            session.suspend().expect("Interrupt recording")
        };
        desktop.shortcut_failure.store(true, Ordering::Release);
        let error = session
            .refresh()
            .err()
            .expect("Automatic shortcut refresh fails");
        session.report(error.clone());
        let failed = session.snapshot().expect("Reported native error");
        session.report(error);
        assert_eq!(
            session.snapshot().expect("Repeated report").message,
            failed.message
        );
        desktop.shortcut_failure.store(false, Ordering::Release);
        let recovered = session.refresh().expect("Native shortcut error clears");
        assert_eq!(recovered.message, interrupted.message);
        assert!(failed.message.contains(&interrupted.message));
        assert!(failed.message.contains(REJECTED));
    }
}

#[test]
/// Interruption feedback must not claim injected input was released while cleanup still retains failed holds.
fn interrupted_playback_feedback_waits_for_held_input_cleanup() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record held key");
    session.capture(key(true));
    session.stop().expect("Retain capture");
    desktop.release_failure.store(true, Ordering::Release);
    let weak = Arc::downgrade(&session);
    *desktop.on_inject.lock().expect("Suspend fixture") = Some(Box::new(
        // Interrupts after successful key-down while the worker still owns release responsibility.
        move || {
            weak.upgrade()
                .expect("Playback owner")
                .signal_stop(true)
                .expect("Suspend playback")
        },
    ));
    session.play().expect("Play held key");
    let failed = completed(&session);
    assert!(failed.cleanup_failed);
    assert!(failed.message.contains(crate::session::SUSPEND_INTERRUPTED));
    assert!(failed.message.contains(crate::session::CLEANUP_FAILED));
    assert!(!failed.message.contains(crate::session::REPLAY_INTERRUPTED));
    desktop.release_failure.store(false, Ordering::Release);
    assert_eq!(
        session.stop().expect("Retry releases hold").message,
        crate::session::REPLAY_INTERRUPTED
    );
}

#[test]
/// Recovering native reservations removes their transient error without erasing other actionable feedback or sleep context.
fn wake_shortcut_recovery_preserves_unrelated_feedback() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture(key(true));
    let interrupted = session.suspend().expect("Interrupt recording");
    desktop.shortcut_failure.store(true, Ordering::Release);
    session.report(session.refresh().err().expect("Native failure"));
    session.report(OTHER_FAILURE.into());
    let failed = session.snapshot().expect("Both errors");
    assert!(failed.message.contains(REJECTED));
    assert!(failed.message.contains(OTHER_FAILURE));
    desktop.shortcut_failure.store(false, Ordering::Release);
    let recovered = session.refresh().expect("Native recovery");
    assert!(recovered.message.contains(&interrupted.message));
    assert!(recovered.message.contains(OTHER_FAILURE));
    assert!(!recovered.message.contains(REJECTED));
}

#[test]
/// A deliberate editor operation acknowledges interruption; a pending close rejects it before its reservation callback executes.
fn editor_reservation_acknowledges_interruption_and_cannot_race_exit() {
    let session = Session::new(Arc::new(Desktop::default()));
    session.record().expect("Record");
    session.capture(key(true));
    session.suspend().expect("Interrupt");
    session
        .editor_operation(|| Ok::<(), String>(()))
        .expect("Editor ownership")
        .expect("Deliberate editor operation");
    assert_eq!(
        session
            .snapshot()
            .expect("Acknowledged interruption")
            .message,
        crate::session::READY
    );
    session.request_close().expect("Close requested");
    assert!(session
        .editor_operation::<(), String>(|| panic!("Closing must reject before editor effects"))
        .is_err());
}

#[test]
/// An already open editor cannot accept a new property save after main Close has won the session's exit gate.
fn accepted_close_fences_editor_submissions() {
    let session = Session::new(Arc::new(Desktop::default()));
    session
        .editor_operation(|| Ok::<(), String>(()))
        .expect("Editor ownership")
        .expect("Open editor");
    session.request_close().expect("Accept main Close");
    assert!(session.editor_operation(|| Ok::<(), String>(())).is_err());
}

#[test]
/// Shortcut refresh failure after disk deletion must not turn the durable successful mutation into a reported deletion failure.
fn committed_deletion_survives_shortcut_refresh_failure() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    let result = session.delete(None, || {
        desktop.shortcut_failure.store(true, Ordering::Release);
        Ok(())
    });
    assert!(result.is_ok());
    assert_eq!(
        session
            .snapshot()
            .expect("Actionable shortcut failure")
            .message,
        REJECTED
    );
}
/// Simulated external-operation ownership used only at the native platform boundary.
struct DesktopReservation(Arc<AtomicBool>);
impl Drop for DesktopReservation {
    /// Makes the external application-operation gate available after the session actually relinquishes its token.
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
/// Creates one recording input without exposing capture implementation details.
fn key(down: bool) -> EventKind {
    EventKind::Key {
        vk: TEST_KEY,
        scan: TEST_SCAN,
        extended: false,
        down,
    }
}
/// Waits for bounded worker completion through the public snapshot contract.
fn completed(session: &Session) -> Snapshot {
    let deadline = Instant::now() + TEST_DEADLINE;
    loop {
        let snapshot = session
            .snapshot()
            .expect("Session snapshot must remain available");
        if snapshot.phase == Phase::Idle || snapshot.cleanup_failed {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "Worker failed to reach a terminal or recoverable state"
        );
        std::thread::sleep(TEST_POLL);
    }
}

#[test]
/// A second F9 observed during compact entry must not become a new recording after immediate F8 cleanup reaches Idle.
fn queued_record_during_startup_cannot_replace_stopped_capture() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    let queued = Arc::new(Mutex::new(Vec::new()));
    let received = queued.clone();
    let weak = Arc::downgrade(&session);
    *desktop.on_entry.lock().expect("Fixture callback") = Some(Box::new(move || {
        let session = weak.upgrade().expect("Startup session remains owned");
        session.capture(key(true));
        session.capture(key(false));
        for control in STARTUP_RECORD_STOP_CONTROLS {
            if let Some(job) = control.receive(&session).expect("Native control arrival") {
                received.lock().expect("Native job fixture").push(job);
            }
        }
    }));
    let initial = ControlRequest::Record
        .receive(&session)
        .expect("Initial F9")
        .expect("Idle accepts F9");
    let stopped = initial
        .execute(&session)
        .expect("Startup completes cancellation");
    *desktop.on_entry.lock().expect("Fixture callback") = None;
    for job in queued.lock().expect("Native job fixture").drain(..) {
        job.execute(&session).expect("Queued control completes");
    }
    let final_state = session.snapshot().expect("Snapshot");
    assert_eq!(final_state.phase, Phase::Idle);
    assert_eq!(
        final_state.event_count, stopped.event_count,
        "Stale F9 must preserve the stopped recording"
    );
    assert!(final_state.unsaved);
}

#[test]
/// Two Record requests received before worker dispatch share eligibility; the delayed request cannot replace a stopped capture.
fn queued_idle_record_cannot_replace_a_completed_generation() {
    let session = Session::new(Arc::new(Desktop::default()));
    let first = session
        .start_request()
        .expect("Initial arrival")
        .expect("Idle");
    let delayed = session
        .start_request()
        .expect("Second arrival")
        .expect("Idle");
    session
        .record_requested(first)
        .expect("First worker starts");
    session.capture(key(true));
    let retained = session.stop().expect("Stop first recording");
    let obsolete = session
        .record_requested(delayed)
        .expect("Obsolete worker resolves");
    assert_eq!(obsolete.phase, Phase::Idle);
    assert_eq!(obsolete.event_count, retained.event_count);
    assert_eq!(obsolete.duration_ms, retained.duration_ms);
}
#[test]
/// Stop or sleep received before worker dispatch invalidates both Record and Play intents without clearing data or injecting input.
fn idle_cancellation_invalidates_queued_start_commands() {
    for control in STARTUP_CANCEL_CONTROLS {
        let desktop = Arc::new(Desktop::default());
        let session = Session::new(desktop.clone());
        session.record().expect("Original recording");
        session.capture(key(true));
        let retained = session.stop().expect("Retain recording");
        let record = session
            .start_request()
            .expect("Record arrival")
            .expect("Idle");
        let play = session
            .start_request()
            .expect("Play arrival")
            .expect("Idle");
        control
            .receive(&session)
            .expect("Cancellation arrival")
            .expect("Cleanup job")
            .execute(&session)
            .expect("Cancellation resolves");
        session
            .record_requested(record)
            .expect("Obsolete Record resolves");
        session
            .play_requested(play)
            .expect("Obsolete Play resolves");
        let state = session.snapshot().expect("Snapshot");
        assert_eq!(state.phase, Phase::Idle);
        assert_eq!(state.event_count, retained.event_count);
        assert_eq!(state.duration_ms, retained.duration_ms);
        assert_eq!(desktop.injections.load(Ordering::Acquire), 0);
    }
}
#[test]
/// A delayed cleanup job belongs to the stopped generation and cannot cancel a newer recording after startup completes cleanup.
fn delayed_cleanup_cannot_stop_a_new_generation() {
    for control in STARTUP_CANCEL_CONTROLS {
        let session = Session::new(Arc::new(Desktop::default()));
        session.record().expect("First recording");
        let delayed = control
            .receive(&session)
            .expect("Cancellation arrival")
            .expect("Cleanup job");
        session.stop().expect("Complete first cleanup");
        session.record().expect("Fresh recording");
        session.capture(key(true));
        let current = delayed
            .execute(&session)
            .expect("Obsolete cleanup resolves");
        assert_eq!(current.phase, Phase::Recording);
        assert!(current.unsaved);
        session.stop().expect("Finish fixture");
    }
}

/// Waits for a queued start to claim eligibility without waiting for its blocked native transition.
fn starting_claimed(session: &Session) {
    let deadline = Instant::now() + TEST_DEADLINE;
    while session
        .start_request()
        .expect("Start eligibility")
        .is_some()
    {
        assert!(Instant::now() < deadline, "Start did not claim ownership");
        std::thread::sleep(TEST_POLL);
    }
}
#[test]
/// Cancellation remains effective while startup waits behind a preview transition, before any native reservation exists.
fn cancellation_while_waiting_for_presentation_prevents_start() {
    for control in STARTUP_CANCEL_CONTROLS {
        let desktop = Arc::new(Desktop::default());
        let session = Session::new(desktop.clone());
        session.record().expect("Original recording");
        session.capture(key(true));
        let retained = session.stop().expect("Retain recording");
        let waiting = session.clone();
        let (sent, finished) = std::sync::mpsc::sync_channel(1);
        session
            .preview(|| {
                std::thread::spawn(move || {
                    let _ = sent.send(waiting.record());
                });
                starting_claimed(&session);
                assert!(!desktop.operation.load(Ordering::Acquire));
                control
                    .receive(&session)
                    .expect("Cancellation arrival")
                    .expect("Cleanup job")
                    .execute(&session)
                    .expect("Cancellation signals without deadlock");
                assert_eq!(session.snapshot().expect("Snapshot").phase, Phase::Stopping);
                Ok(())
            })
            .expect("Preview releases transition");
        let stopped = finished
            .recv_timeout(TEST_DEADLINE)
            .expect("Startup resolves")
            .expect("Cancelled startup completes");
        assert_eq!(stopped.phase, Phase::Idle);
        assert_eq!(stopped.event_count, retained.event_count);
        assert!(!desktop.operation.load(Ordering::Acquire));
        assert!(!desktop.registered.lock().expect("Reservations").stop);
    }
}

/// Installs one stop/suspend action during the real startup reservation boundary, preserving the session's own cancellation handling.
fn cancel_during_reservation(desktop: &Desktop, session: &Arc<Session>, control: ControlRequest) {
    let weak = Arc::downgrade(session);
    *desktop.on_shortcuts.lock().expect("Fixture callback") = Some(Box::new(move || {
        let session = weak.upgrade().expect("Startup session remains owned");
        match control {
            ControlRequest::Stop => {
                session.stop().expect("Stop during reservation");
            }
            ControlRequest::Suspend => {
                session.suspend().expect("Sleep during reservation");
            }
            ControlRequest::Record => {
                unreachable!("Cancellation fixtures use Stop or suspend only")
            }
        }
    }));
}
#[test]
/// Stop or sleep after F8 acquisition but before capture activation must end startup and retain the previous capture.
fn cancellation_during_reservation_cannot_start_recording() {
    for control in STARTUP_CANCEL_CONTROLS {
        let desktop = Arc::new(Desktop::default());
        let session = Session::new(desktop.clone());
        session.record().expect("Previous recording");
        session.capture(key(true));
        let retained = session.stop().expect("Retain previous capture");
        cancel_during_reservation(&desktop, &session, control);
        let stopped = session
            .record()
            .expect("Cancelled startup resolves normally");
        assert_eq!(stopped.phase, Phase::Idle);
        assert_eq!(stopped.event_count, retained.event_count);
        assert_eq!(stopped.duration_ms, retained.duration_ms);
    }
}
#[test]
/// Stop or sleep during playback reservation must prevent all injection rather than starting a worker after interruption.
fn cancellation_during_reservation_cannot_start_playback() {
    for control in STARTUP_CANCEL_CONTROLS {
        let desktop = Arc::new(Desktop::default());
        let session = Session::new(desktop.clone());
        session.record().expect("Record replay data");
        session.capture(key(true));
        session.stop().expect("Retain replay data");
        let before = desktop.injections.load(Ordering::Acquire);
        cancel_during_reservation(&desktop, &session, control);
        session.play().expect("Cancelled startup resolves normally");
        assert_eq!(completed(&session).phase, Phase::Idle);
        assert_eq!(
            desktop.injections.load(Ordering::Acquire),
            before,
            "Interrupted startup must dispatch no playback input"
        );
    }
}
#[test]
/// A Stop conflict blocks new recording without destroying the previous captured input; explicit retry recovers.
fn stop_conflict_preserves_capture_and_retry_recovers() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session
        .record()
        .expect("Available shortcuts allow recording");
    session.capture(key(true));
    let old = session.stop().expect("Capture stops");
    desktop.stop_conflict.store(true, Ordering::Release);
    assert!(session.record().is_err());
    assert_eq!(
        session.snapshot().expect("Snapshot").event_count,
        old.event_count
    );
    desktop.stop_conflict.store(false, Ordering::Release);
    assert!(
        session
            .retry_shortcuts()
            .expect("Retry must recover")
            .can_record
    );
    assert_eq!(session.record().expect("New start").event_count, 0);
}

#[test]
/// Repeated Record rejects overlap without clearing captured input or resetting the active recording.
fn repeated_record_keeps_the_active_capture() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Start recording");
    session.capture(key(true));
    assert_eq!(session.record().err().as_deref(), Some(ACTIVE_SESSION));
    let snapshot = session.snapshot().expect("Snapshot");
    assert_eq!(snapshot.phase, Phase::Recording);
    assert_eq!(snapshot.event_count, 1);
    assert!(
        desktop.registered.lock().expect("Native reservations").stop,
        "Repeated Record must retain global Stop"
    );
}
#[test]
/// Failed releases retain ownership and block every new start until explicit Retry succeeds; retry injects no ordinary playback.
fn failed_cleanup_blocks_sessions_and_retry_releases_only_outstanding_input() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture(key(true));
    session.capture(key(false));
    session.stop().expect("Stop capture");
    desktop.key_up_failure.store(true, Ordering::Release);
    desktop.release_failure.store(true, Ordering::Release);
    session.play().expect("Replay");
    let failed = completed(&session);
    assert_eq!(failed.phase, Phase::Stopping);
    assert!(failed.cleanup_failed);
    assert!(desktop.pressed.load(Ordering::Acquire));
    assert!(session.record().is_err());
    assert!(session.play().is_err());
    desktop.release_failure.store(false, Ordering::Release);
    let retried = session.stop().expect("Retry cleanup");
    assert_eq!(retried.phase, Phase::Idle);
    assert!(!retried.cleanup_failed);
    assert!(!desktop.pressed.load(Ordering::Acquire));
    assert!(retried.message.contains(REJECTED));
}
#[test]
/// Restoration failure retains Stopping even after input release; retry grants Idle only after the native frame is restored.
fn native_restoration_failure_keeps_session_owned_until_retry() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture(key(true));
    desktop.restore_failure.store(true, Ordering::Release);
    let failed = session.stop().expect("Stop records recoverable failure");
    assert_eq!(failed.phase, Phase::Stopping);
    assert!(failed.cleanup_failed);
    assert!(session.record().is_err());
    desktop.restore_failure.store(false, Ordering::Release);
    assert_eq!(
        session.stop().expect("Retry restoration").phase,
        Phase::Idle
    );
}
#[test]
/// Explicit close cannot silently discard capture; Keep open preserves it and Discard is unavailable while cleanup remains active.
fn close_retains_unsaved_capture_until_explicit_discard() {
    let session = Session::new(Arc::new(Desktop::default()));
    session.record().expect("Record");
    session.capture(key(true));
    assert!(session.request_close().expect("Close decision"));
    assert!(session.discard_exit().is_err());
    let stopped = session.stop().expect("Stop on close");
    assert!(stopped.close_requested);
    assert!(stopped.unsaved);
    assert!(session.keep_open().expect("Keep open").unsaved);
    assert!(session.request_close().expect("Ask again"));
    session.discard_exit().expect("Explicit discard");
    assert!(!session.snapshot().expect("Snapshot").unsaved);
}
#[test]
/// Computer sleep ends recording, retains data, and does not restart capture when status refreshes after wake.
fn sleep_ends_recording_and_retains_temporary_data() {
    let session = Session::new(Arc::new(Desktop::default()));
    session.record().expect("Record");
    session.capture(key(true));
    let interrupted = session.suspend().expect("Sleep interruption");
    assert_eq!(interrupted.phase, Phase::Idle);
    assert!(interrupted.unsaved);
    assert_eq!(interrupted.message, crate::session::INTERRUPTED);
    assert_eq!(session.refresh().expect("Wake refresh").phase, Phase::Idle);
}
#[test]
/// Native confirmation exclusion is checked independently of an enabled or stale Record button/global shortcut.
fn confirmation_blocks_start_until_cancelled() {
    let session = Session::new(Arc::new(Desktop::default()));
    assert!(
        !session
            .confirmation(CONFIRMATION_OWNER, true)
            .expect("Reserve confirmation")
            .can_record
    );
    assert!(session.record().is_err());
    session
        .confirmation(CONFIRMATION_OWNER, false)
        .expect("Cancel confirmation");
    assert_eq!(
        session.record().expect("Record after Cancel").phase,
        Phase::Recording
    );
}
#[test]
/// Fast in-memory replay releases its worker ownership before Idle; no extra Stop is required to finish.
fn successful_temporary_replay_returns_to_idle() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture(key(true));
    session.stop().expect("Stop capture");
    session.play().expect("Replay");
    assert_eq!(completed(&session).phase, Phase::Idle);
    assert!(!desktop.pressed.load(Ordering::Acquire));
}
#[test]
/// Failed compact entry rolls back to the previous recording instead of destroying it or granting unsafe active ownership.
fn failed_start_preserves_the_previous_capture() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture(key(true));
    let previous = session.stop().expect("Stop");
    desktop.entry_failure.store(true, Ordering::Release);
    assert!(session.record().is_err());
    let snapshot = session.snapshot().expect("Snapshot");
    assert_eq!(snapshot.event_count, previous.event_count);
    assert_eq!(snapshot.phase, Phase::Idle);
}
#[test]
/// Input arriving during native compact transition is captured immediately; the recording clock never waits for UI layout.
fn input_during_compact_entry_is_captured() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    let weak = Arc::downgrade(&session);
    *desktop.on_entry.lock().expect("Fixture callback") = Some(Box::new(move || {
        if let Some(session) = weak.upgrade() {
            session.capture(key(true));
        }
    }));
    assert_eq!(session.record().expect("Record").event_count, 1);
}
#[test]
/// Stop arriving during native entry remains effective; startup cannot overwrite the cancelled session with Recording.
fn stop_during_compact_entry_cannot_be_lost() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    let weak = Arc::downgrade(&session);
    *desktop.on_entry.lock().expect("Fixture callback") = Some(Box::new(move || {
        if let Some(session) = weak.upgrade() {
            session.signal_stop(false).expect("Native Stop signal");
        }
    }));
    assert_eq!(
        session
            .record()
            .expect("Record handles concurrent Stop")
            .phase,
        Phase::Idle
    );
}
#[test]
/// Controlled preview validation/publication holds the same start guard; active sessions reject every preview update.
fn preview_publication_and_session_start_are_serialized() {
    let session = Session::new(Arc::new(Desktop::default()));
    let waiting = session.clone();
    let (completed_start, started) = std::sync::mpsc::sync_channel(1);
    session
        .preview(|| {
            std::thread::spawn(move || {
                let _ = completed_start.send(waiting.record());
            });
            assert_eq!(
                session.snapshot().expect("Snapshot during preview").phase,
                Phase::Idle
            );
            Ok(())
        })
        .expect("Idle preview");
    assert_eq!(
        started
            .recv_timeout(TEST_DEADLINE)
            .expect("Queued start must complete")
            .expect("Record after preview")
            .phase,
        Phase::Recording
    );
    assert!(session.preview(|| Ok(())).is_err());
}
#[test]
/// Confirmation passes ownership to disk work without opening a gap for a global Record request.
fn confirmed_deletion_handoff_keeps_operation_exclusion() {
    let session = Session::new(Arc::new(Desktop::default()));
    session
        .confirmation(CONFIRMATION_OWNER, true)
        .expect("Open confirmation");
    let reservation = session
        .take_confirmation(Some(CONFIRMATION_OWNER))
        .expect("Transfer confirmation")
        .expect("Existing token");
    assert!(session.record().is_err());
    drop(reservation);
    assert_eq!(
        session.record().expect("Record after disk work").phase,
        Phase::Recording
    );
}
#[test]
/// A late old-owner cancellation cannot drop another mounted dialog's native exclusion.
fn stale_confirmation_release_does_not_release_a_new_owner() {
    const NEXT_OWNER: &str = "next-session-test-owner";
    let session = Session::new(Arc::new(Desktop::default()));
    session
        .confirmation(CONFIRMATION_OWNER, true)
        .expect("Old confirmation");
    session
        .confirmation(CONFIRMATION_OWNER, false)
        .expect("Old cleanup");
    session
        .confirmation(NEXT_OWNER, true)
        .expect("New confirmation");
    session
        .confirmation(CONFIRMATION_OWNER, false)
        .expect("Late old cleanup");
    assert!(session.record().is_err());
    session
        .confirmation(NEXT_OWNER, false)
        .expect("Current owner Cancel");
    assert!(session.record().is_ok());
}
#[test]
/// A late disposed-dialog cleanup during recording cannot release the active session's global Stop reservation.
fn late_confirmation_cleanup_keeps_active_stop_reserved() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session
        .confirmation(CONFIRMATION_OWNER, false)
        .expect("Harmless abandoned-owner release");
    assert!(desktop.registered.lock().expect("Native reservations").stop);
    assert_eq!(
        session.snapshot().expect("Snapshot").phase,
        Phase::Recording
    );
}
#[test]
/// A callback delivered during synchronous native injection can signal Stop without waiting on session state held by its caller.
fn stop_callback_during_native_injection_does_not_deadlock() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    session.capture(key(true));
    session.stop().expect("Stop capture");
    let weak = Arc::downgrade(&session);
    let callback_completed = Arc::new(AtomicBool::new(false));
    let observed = callback_completed.clone();
    *desktop.on_inject.lock().expect("Fixture callback") = Some(Box::new(move || {
        let (sent, received) = std::sync::mpsc::sync_channel(1);
        let weak = weak.clone();
        std::thread::spawn(move || {
            if let Some(session) = weak.upgrade() {
                let result = session.signal_stop(false);
                let _ = sent.send(result);
            }
        });
        let completed = received.recv_timeout(TEST_DEADLINE).is_ok();
        observed.store(completed, Ordering::Release);
    }));
    session.play().expect("Replay");
    assert_eq!(completed(&session).phase, Phase::Idle);
    assert!(callback_completed.load(Ordering::Acquire));
    assert!(!desktop.pressed.load(Ordering::Acquire));
}
#[test]
/// Failed global reservation release is a recoverable cleanup failure; it cannot silently grant Idle or omit Retry cleanup.
fn failed_shortcut_cleanup_is_visible_and_retryable() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.record().expect("Record");
    desktop.unregister_failure.store(true, Ordering::Release);
    let stopped = session.stop().expect("Stop retains failure");
    assert_eq!(stopped.phase, Phase::Stopping);
    assert!(stopped.cleanup_failed);
    assert!(session.record().is_err());
    desktop.unregister_failure.store(false, Ordering::Release);
    assert_eq!(
        session
            .stop()
            .expect("Retry native reservation release")
            .phase,
        Phase::Idle
    );
}
#[test]
/// Delayed Idle status cannot obtain a newer presentation revision after Recording has already been delivered.
fn delayed_idle_publication_cannot_replace_newer_recording() {
    let session = Session::new(Arc::new(Desktop::default()));
    let old_idle = session.snapshot().expect("Initial idle snapshot");
    let recording = session.record().expect("Recording snapshot");
    let publications = crate::session_publication::Publications::default();
    let phases = Mutex::new(Vec::new());
    publications
        .send(recording, |snapshot| {
            phases.lock().expect("Fixture phases").push(snapshot.phase)
        })
        .expect("New active delivery");
    publications
        .send(old_idle, |snapshot| {
            phases.lock().expect("Fixture phases").push(snapshot.phase)
        })
        .expect("Old delivery is ignored");
    assert_eq!(
        *phases.lock().expect("Delivered phases"),
        vec![Phase::Recording]
    );
}
#[test]
/// Confirmation cancellation queues behind a short presentation/availability transition instead of leaving a stranded lease.
fn confirmation_cancel_waits_for_transient_transition_and_recovers() {
    let session = Session::new(Arc::new(Desktop::default()));
    session
        .confirmation(CONFIRMATION_OWNER, true)
        .expect("Confirmation");
    let waiting = session.clone();
    let (started, starting) = std::sync::mpsc::sync_channel(1);
    let (sent, completed_cancel) = std::sync::mpsc::sync_channel(1);
    session
        .preview(|| {
            std::thread::spawn(move || {
                let _ = started.send(());
                let _ = sent.send(waiting.confirmation(CONFIRMATION_OWNER, false));
            });
            starting
                .recv_timeout(TEST_DEADLINE)
                .expect("Cancellation is scheduled");
            assert!(
                completed_cancel.recv_timeout(TEST_POLL).is_err(),
                "Cancellation must queue rather than fail while a transient transition is held"
            );
            Ok(())
        })
        .expect("Transient presentation guard");
    assert!(
        completed_cancel
            .recv_timeout(TEST_DEADLINE)
            .expect("Queued cancellation completes")
            .expect("Cancellation succeeds")
            .can_record
    );
    assert!(session.record().is_ok());
}

#[test]
/// A waiting start exposes native Stop before Save finishes and Stop prevents recording afterwards.
fn rename_wait_exposes_stop_and_cancels_start() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    session.refresh().expect("Initial availability");
    let mut save = session.begin_rename().expect("Begin rename");
    assert!(
        desktop
            .registered
            .lock()
            .expect("Stop armed before deferred arrival")
            .stop
    );
    let initial_sequence = session
        .snapshot()
        .expect("Before queued start")
        .start_sequence;
    let request = session
        .start_request()
        .expect("Receive Record")
        .expect("Eligible start");
    assert_eq!(
        session
            .snapshot()
            .expect("Deferred draft lifetime")
            .start_sequence,
        initial_sequence
    );
    assert!(session.start_request().expect("Repeated start").is_none());
    let worker_session = session.clone();
    let worker = std::thread::spawn(move || worker_session.record_requested(request));
    let deadline = Instant::now() + TEST_DEADLINE;
    while !desktop.registered.lock().expect("Shortcuts").stop && Instant::now() < deadline {
        std::thread::sleep(TEST_POLL);
    }
    assert!(desktop.registered.lock().expect("Shortcuts").stop);
    session.signal_stop(false).expect("Immediate Stop");
    assert!(!session.snapshot().expect("Cancelled start").deferred_start);
    assert!(!save.finish(true).expect("Save success"));
    assert_eq!(
        worker
            .join()
            .expect("Start worker")
            .expect("Start result")
            .phase,
        Phase::Idle
    );
}

#[test]
/// A failed name save yields to the first requested start, without accumulating repeated starts.
fn rename_failure_yields_to_one_record_request() {
    let session = Session::new(Arc::new(Desktop::default()));
    let mut save = session.begin_rename().expect("Begin rename");
    let request = session
        .start_request()
        .expect("Record arrival")
        .expect("First start");
    assert!(session.start_request().expect("Repeat arrival").is_none());
    assert!(save.finish(false).expect("Failed Save yields draft"));
    assert_eq!(
        session
            .record_requested(request)
            .expect("Deferred Record")
            .phase,
        Phase::Recording
    );
    assert!(session.begin_rename().is_err());
    session.stop().expect("Stop recording");
}

#[test]
/// Close cancels deferred starts, waits for disk completion, and retains failure drafts by cancelling exit.
fn rename_close_waits_and_failure_cancels_exit() {
    for succeeds in [false, true] {
        let session = Session::new(Arc::new(Desktop::default()));
        let mut save = session.begin_rename().expect("Begin rename");
        let request = session
            .start_request()
            .expect("Start arrival")
            .expect("Deferred start");
        session.request_close().expect("Close");
        assert!(!session.finish_exit().expect("Wait for Save"));
        assert!(!save.finish(succeeds).expect("Resolve Save"));
        assert_eq!(
            session
                .record_requested(request)
                .expect("Cancelled start")
                .phase,
            Phase::Idle
        );
        assert_eq!(
            session.snapshot().expect("Exit state").close_requested,
            succeeds
        );
        assert_eq!(session.finish_exit().expect("Exit eligibility"), succeeds);
    }
}

#[test]
/// A Record received while native Stop is being armed wins before a name write can be accepted.
fn rename_arming_yields_to_an_arriving_global_record() {
    let desktop = Arc::new(Desktop::default());
    let session = Session::new(desktop.clone());
    let received = Arc::new(Mutex::new(None));
    let observed = received.clone();
    let owner = Arc::downgrade(&session);
    *desktop.on_shortcuts.lock().expect("Arming callback") = Some(Box::new(move || {
        let session = owner.upgrade().expect("Live session during native arming");
        *observed.lock().expect("Received start") =
            session.start_request().expect("Record arrival");
    }));
    assert!(session.begin_rename().is_err());
    assert!(!session.snapshot().expect("No accepted Save").rename_saving);
    let request = received
        .lock()
        .expect("Captured start")
        .take()
        .expect("Priority Record");
    assert_eq!(
        session
            .record_requested(request)
            .expect("Start after arming")
            .phase,
        Phase::Recording
    );
    session.stop().expect("Stop recording");
}

#[test]
/// A Stop reservation conflict permits metadata saving but cannot accept an uncancellable deferred start.
fn rename_save_rejects_deferred_starts_without_reserved_stop() {
    let desktop = Arc::new(Desktop::default());
    desktop.stop_conflict.store(true, Ordering::Release);
    let session = Session::new(desktop);
    let mut save = session
        .begin_rename()
        .expect("Name saving remains available");
    assert_eq!(
        session.start_request().err().as_deref(),
        Some(crate::shortcuts::STOP_UNAVAILABLE)
    );
    assert!(
        !session
            .snapshot()
            .expect("No accepted deferred start")
            .deferred_start
    );
    save.finish(true).expect("Metadata save completes normally");
}
