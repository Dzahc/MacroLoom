//! Behavior tests through session commands and snapshots, substituting only desktop effects.
use crate::input::{EventKind, Held};
use crate::session::{Phase, Platform, Session, Snapshot, ACTIVE_SESSION};
use crate::shortcuts::Availability;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
}
impl Platform for Desktop {
    /// Returns an owned external-operation token, with no native input effects.
    fn reserve(&self) -> Result<Box<dyn Send>, String> {
        Ok(Box::new(()))
    }
    /// The fixture library is ready and no editor is active.
    fn available(&self) -> bool {
        true
    }
    /// Reports a controllable native Stop conflict while retaining Record availability.
    fn shortcuts(&self, record: bool, _: bool) -> Result<Availability, String> {
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
    let session = Session::new(Arc::new(Desktop::default()));
    session.record().expect("Start recording");
    session.capture(key(true));
    assert_eq!(session.record().err().as_deref(), Some(ACTIVE_SESSION));
    let snapshot = session.snapshot().expect("Snapshot");
    assert_eq!(snapshot.phase, Phase::Recording);
    assert_eq!(snapshot.event_count, 1);
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
            .confirmation(true)
            .expect("Reserve confirmation")
            .can_record
    );
    assert!(session.record().is_err());
    session.confirmation(false).expect("Cancel confirmation");
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
