//! Behavior tests through session commands and snapshots, substituting only desktop effects.
use crate::input::{EventKind, Held};
use crate::session::{Phase, Platform, Session, Snapshot, ACTIVE_SESSION};
use crate::shortcuts::Availability;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CONFIRMATION_OWNER: &str = "session-test-owner";
const TEST_KEY: u16 = 0x41;
const TEST_SCAN: u16 = 0x1e;
const REJECTED: &str = "Injected native failure";
const TEST_DEADLINE: Duration = Duration::from_secs(2);
const TEST_POLL: Duration = Duration::from_millis(1);

#[derive(Default)]
/// A deterministic desktop-effect substitute; never replaces session state or lifecycle logic.
struct Desktop {
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
}
impl Platform for Desktop {
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
    session
        .preview(|| {
            assert!(session.record().is_err());
            Ok(())
        })
        .expect("Idle preview");
    session.record().expect("Record after preview");
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
