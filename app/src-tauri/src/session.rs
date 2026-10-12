//! One backend-owned temporary session, cancellation, and recoverable input cleanup.
use crate::input::{track_held, wait_until, Capture, EventKind, Held};
use crate::shortcuts::{Availability, RECORD_UNAVAILABLE, STOP_UNAVAILABLE};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const SESSION_EVENT: &str = "session-state";
pub const ACTIVE_SESSION: &str = "A session is already active or stopping";
pub const READY: &str = "Ready. F9 records; F8 stops recording or playback.";
pub const UNSAVED: &str = "Recording retained in memory; not saved.";
pub const EMPTY_CAPTURE: &str = "No input captured.";
pub const RECORDING: &str = "Recording. F8 stops.";
pub const PLAYING: &str = "Playing temporary recording. F8 stops.";
pub const STOPPING: &str = "Stopping; releasing input and restoring the window.";
pub const INTERRUPTED: &str = "Recording interrupted; not saved.";
pub const NO_CAPTURE: &str = "Record input before replaying the temporary capture.";
pub const STATE_ERROR: &str = "Session state is unavailable; restart MacroLoom.";
pub const CLEANUP_FAILED: &str =
    "Cleanup failed. Retry cleanup or press F8; new sessions are blocked.";
pub const REPLAY_COMPLETE: &str = "Temporary playback complete.";
pub const REPLAY_CANCELLED: &str = "Playback stopped; injected input released.";
pub const STATUS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
/// Idle grants new ownership; Stopping retains it until every release/restoration succeeds.
pub enum Phase {
    Idle,
    Recording,
    Playing,
    Stopping,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Revisioned command/event contract; availability reflects actual native reservations and capture stays backend-owned.
pub struct Snapshot {
    pub revision: u64,
    pub phase: Phase,
    pub elapsed_ms: u64,
    pub event_count: usize,
    pub duration_ms: u64,
    pub drag_move_count: usize,
    pub last_release: Option<(i32, i32)>,
    pub message: String,
    pub shortcuts: Availability,
    pub can_record: bool,
    pub cleanup_failed: bool,
    pub close_requested: bool,
    pub unsaved: bool,
}

/// Native system boundaries; deterministic tests substitute desktop effects, never session internals.
pub trait Platform: Send + Sync {
    /// Claims application operation ownership; the returned guard releases exclusion when dropped.
    fn reserve(&self) -> Result<Box<dyn Send>, String>;
    /// Returns whether library initialization, editor, deletion, or external interlocks prevent a start.
    fn available(&self) -> bool;
    /// Reconciles desired reservations on their owning OS message thread; returns actual availability.
    fn shortcuts(&self, record: bool, stop: bool) -> Result<Availability, String>;
    /// Enters/restores the actual owned compact view without activation; failures retain restoration state.
    fn view(&self, compact: bool) -> Result<(), String>;
    /// Reads current signed physical cursor coordinates for a recording boundary.
    fn cursor(&self) -> (i32, i32);
    /// Dispatches one event, rejecting actual compact-window mouse collisions before injection.
    fn inject(&self, kind: &EventKind) -> Result<(), String>;
    /// Releases one input injected by this session; rejection must retain its ownership for Retry.
    fn release(&self, held: Held) -> Result<(), String>;
    /// Publishes a snapshot without influencing capture timing or cancellation.
    fn publish(&self, snapshot: Snapshot);
}

/// All capture and session ownership; no native/window/disk work occurs while changing phase.
struct State {
    phase: Phase,
    revision: u64,
    start: Option<Instant>,
    capture: Capture,
    held: Vec<Held>,
    cancel: Arc<AtomicBool>,
    worker: bool,
    reservation: Option<Box<dyn Send>>,
    shortcuts: Availability,
    can_record: bool,
    message: String,
    terminal: String,
    cleanup_failed: bool,
    close_requested: bool,
    confirmation: Option<Box<dyn Send>>,
}
impl Default for State {
    /// Initializes an idle owner with no input, cancellation, or reservations.
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            revision: 0,
            start: None,
            capture: Capture::default(),
            held: Vec::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            worker: false,
            reservation: None,
            shortcuts: Availability::default(),
            can_record: false,
            message: READY.into(),
            terminal: READY.into(),
            cleanup_failed: false,
            close_requested: false,
            confirmation: None,
        }
    }
}

/// Public session command boundary; a transition mutex serializes native start/cleanup without blocking capture callbacks.
pub struct Session {
    platform: Arc<dyn Platform>,
    state: Mutex<State>,
    transition: Mutex<()>,
}
impl Session {
    /// Binds external desktop effects to a new idle owner; starts no hooks or session.
    pub fn new(platform: Arc<dyn Platform>) -> Arc<Self> {
        Arc::new(Self {
            platform,
            state: Mutex::new(State::default()),
            transition: Mutex::new(()),
        })
    }
    /// Obtains a short-lived state guard, refusing to recover conflicting ownership after panic.
    fn lock(&self) -> Result<MutexGuard<'_, State>, String> {
        self.state.lock().map_err(|_| STATE_ERROR.into())
    }
    /// Returns the current immutable revisioned state; recording elapsed time uses its monotonic start.
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let state = self.lock()?;
        Ok(Snapshot {
            revision: state.revision,
            phase: state.phase,
            elapsed_ms: state
                .start
                .map_or(0, |start| start.elapsed().as_millis() as u64),
            event_count: state.capture.events.len(),
            duration_ms: state.capture.duration.as_millis() as u64,
            drag_move_count: state.capture.drag_moves,
            last_release: state.capture.last_release,
            message: state.message.clone(),
            shortcuts: state.shortcuts,
            can_record: state.can_record && state.phase == Phase::Idle,
            cleanup_failed: state.cleanup_failed,
            close_requested: state.close_requested,
            unsaved: !state.capture.events.is_empty(),
        })
    }
    /// Increments the revision before publishing, allowing frontend events and IPC results to reconcile safely.
    fn publish(&self) -> Result<(), String> {
        self.lock()?.revision += 1;
        self.platform.publish(self.snapshot()?);
        Ok(())
    }
    /// Publishes actionable command errors, including errors from global controls with no frontend caller.
    pub fn report(&self, error: String) {
        if let Ok(mut state) = self.lock() {
            state.message = error;
        }
        let _ = self.publish();
    }
    /// Refreshes idle availability and progress; a busy transition is left to its owner.
    pub fn refresh(&self) -> Result<Snapshot, String> {
        if let Ok(_transition) = self.transition.try_lock() {
            if self.lock()?.phase == Phase::Idle {
                self.idle_shortcuts()?;
            }
        }
        self.publish()?;
        self.snapshot()
    }
    /// Acquires idle Record and probes Stop without retaining its reservation in inactive states.
    fn idle_shortcuts(&self) -> Result<(), String> {
        let available = self.platform.available() && self.idle_available()?;
        let result = self.platform.shortcuts(available, false)?;
        let mut state = self.lock()?;
        state.shortcuts = result;
        state.can_record = available && result.stop;
        if available {
            update_shortcut_message(&mut state, result);
        }
        Ok(())
    }
    /// Rejects idle Record reservations during native confirmation or a pending close decision.
    fn idle_available(&self) -> Result<bool, String> {
        let state = self.lock()?;
        Ok(state.confirmation.is_none() && !state.close_requested)
    }
    /// Explicitly reprobes global shortcuts while idle; no recording or playback is started by retry.
    pub fn retry_shortcuts(&self) -> Result<Snapshot, String> {
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        if self.lock()?.phase != Phase::Idle {
            return Err(ACTIVE_SESSION.into());
        }
        if self.platform.available() {
            self.platform.shortcuts(true, true)?;
        }
        self.idle_shortcuts()?;
        self.publish()?;
        self.snapshot()
    }
    /// Claims one operation and required Stop before changing capture; failed validation preserves the previous capture.
    fn prepare(&self, recording: bool) -> Result<Box<dyn Send>, String> {
        let state = self.lock()?;
        if state.phase != Phase::Idle || state.confirmation.is_some() || state.close_requested {
            return Err(ACTIVE_SESSION.into());
        }
        drop(state);
        let reservation = self.platform.reserve()?;
        let shortcuts = self.platform.shortcuts(recording, true)?;
        self.lock()?.shortcuts = shortcuts;
        if !shortcuts.stop {
            return Err(STOP_UNAVAILABLE.into());
        }
        Ok(reservation)
    }
    /// Restores idle shortcut reservations after rejected start validation, preserving the previous capture.
    fn prepare_start(&self, recording: bool) -> Result<Box<dyn Send>, String> {
        match self.prepare(recording) {
            Ok(reservation) => Ok(reservation),
            Err(error) => {
                self.idle_shortcuts()?;
                Err(error)
            }
        }
    }
    /// Starts capture before native layout work so compact transitions never postpone the recording clock.
    pub fn record(&self) -> Result<Snapshot, String> {
        let transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        let reservation = self.prepare_start(true)?;
        let previous = self.begin_record(reservation)?;
        self.publish()?;
        let result = self.enter_view(previous);
        drop(transition);
        self.finish_start()?;
        result?;
        self.snapshot()
    }
    /// Claims recording data and returns the previous capture for rollback if native entry fails.
    fn begin_record(&self, reservation: Box<dyn Send>) -> Result<Capture, String> {
        let mut state = self.lock()?;
        let previous = std::mem::take(&mut state.capture);
        state.phase = Phase::Recording;
        state.start = Some(Instant::now());
        state.reservation = Some(reservation);
        state.message = RECORDING.into();
        state.can_record = false;
        state.cleanup_failed = false;
        Ok(previous)
    }
    /// Performs native entry after ownership is visible; restores prior temporary data if entry fails.
    fn enter_view(&self, previous: Capture) -> Result<(), String> {
        if let Err(error) = self.platform.view(true) {
            self.rollback_start(previous, &error)?;
            return Err(error);
        }
        Ok(())
    }
    /// Completes cancellation or rollback that arrived while native startup owned the transition guard.
    fn finish_start(&self) -> Result<(), String> {
        if self.lock()?.phase == Phase::Stopping {
            self.cleanup()?;
            self.publish()?;
        }
        Ok(())
    }
    /// Restores the previous temporary capture after failed native entry while retaining cleanup ownership until full restoration.
    fn rollback_start(&self, previous: Capture, error: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        state.capture = previous;
        state.phase = Phase::Stopping;
        state.start = None;
        state.worker = false;
        state.terminal = error.into();
        state.message = error.into();
        Ok(())
    }
    /// Appends one observed event only while recording; short callbacks never do persistence or window transitions.
    pub fn capture(&self, kind: EventKind) {
        if let Ok(mut state) = self.lock() {
            if state.phase != Phase::Recording {
                return;
            }
            if let Some(start) = state.start {
                state.capture.append(start.elapsed(), kind);
            }
        }
    }
    /// Signals cancellation and freezes recording at the initiating Stop boundary, before any frontend callback or cleanup wait.
    pub fn signal_stop(&self, interrupted: bool) -> Result<(), String> {
        let mut state = self.lock()?;
        match state.phase {
            Phase::Idle | Phase::Stopping => return Ok(()),
            Phase::Playing => {
                state.cancel.store(true, Ordering::Release);
                state.terminal = REPLAY_CANCELLED.into();
            }
            Phase::Recording => {
                let duration = state.start.map_or(Duration::ZERO, |start| start.elapsed());
                state.capture.finish(duration, self.platform.cursor());
                state.terminal =
                    recording_message(interrupted, state.capture.events.is_empty()).into();
            }
        }
        state.phase = Phase::Stopping;
        state.message = STOPPING.into();
        state.can_record = false;
        Ok(())
    }
    /// Stops/requests cancellation; repeated Stop retries only failed cleanup after the playback worker has relinquished dispatch.
    pub fn stop(&self) -> Result<Snapshot, String> {
        self.signal_stop(false)?;
        if !self.lock()?.worker {
            self.cleanup()?;
        }
        self.publish()?;
        self.snapshot()
    }
    /// Ends capture or playback on system sleep, retaining temporary capture and never resuming overdue dispatch.
    pub fn suspend(&self) -> Result<Snapshot, String> {
        self.signal_stop(true)?;
        self.stop()
    }
    /// Releases outstanding holds then restores the window; failures retain Stopping and operation ownership.
    fn cleanup(&self) -> Result<(), String> {
        let Ok(_transition) = self.transition.try_lock() else {
            return Ok(());
        };
        if self.lock()?.phase != Phase::Stopping {
            return Ok(());
        }
        self.release_outstanding()?;
        if !self.lock()?.held.is_empty() {
            return self.cleanup_error(CLEANUP_FAILED.into());
        }
        if let Err(error) = self.platform.view(false) {
            return self.cleanup_error(format!("{CLEANUP_FAILED} {error}"));
        }
        self.finish_cleanup()
    }
    /// Releases only outstanding session-owned input, retaining every failed release for an explicit retry.
    fn release_outstanding(&self) -> Result<(), String> {
        let held = std::mem::take(&mut self.lock()?.held);
        let mut failed = Vec::new();
        for item in held.into_iter().rev() {
            if self.platform.release(item).is_err() {
                failed.push(item);
            }
        }
        self.lock()?.held = failed;
        Ok(())
    }
    /// Retains an actionable cleanup error without claiming that held input or native restoration completed.
    fn cleanup_error(&self, error: String) -> Result<(), String> {
        let mut state = self.lock()?;
        state.cleanup_failed = true;
        state.message = error;
        Ok(())
    }
    /// Grants Idle only after releases, native restoration, and Stop unregistration complete.
    fn finish_cleanup(&self) -> Result<(), String> {
        let availability = self.platform.shortcuts(false, false)?;
        let mut state = self.lock()?;
        state.shortcuts = availability;
        state.phase = Phase::Idle;
        state.start = None;
        state.reservation = None;
        state.cleanup_failed = false;
        state.message = state.terminal.clone();
        drop(state);
        self.idle_shortcuts()
    }
    /// Runs temporary replay on one worker; Stop becomes effective before native entry and any input injection.
    pub fn play(self: &Arc<Self>) -> Result<Snapshot, String> {
        let transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        if self.lock()?.capture.events.is_empty() {
            return Err(NO_CAPTURE.into());
        }
        let reservation = self.prepare_start(false)?;
        let (capture, cancel) = self.begin_replay(reservation)?;
        self.publish()?;
        let entry = self.enter_view(capture.clone());
        drop(transition);
        if let Err(error) = entry {
            self.finish_start()?;
            return Err(error);
        }
        let session = self.clone();
        std::thread::spawn(move || session.replay(capture, cancel));
        self.snapshot()
    }
    /// Claims playback ownership and a stable temporary capture before native layout work starts.
    fn begin_replay(
        &self,
        reservation: Box<dyn Send>,
    ) -> Result<(Capture, Arc<AtomicBool>), String> {
        let mut state = self.lock()?;
        state.phase = Phase::Playing;
        state.start = Some(Instant::now());
        state.cancel = Arc::new(AtomicBool::new(false));
        state.worker = true;
        state.reservation = Some(reservation);
        state.message = PLAYING.into();
        state.terminal = REPLAY_COMPLETE.into();
        state.can_record = false;
        Ok((state.capture.clone(), state.cancel.clone()))
    }
    /// Executes ordered deadlines and guarded injection; terminal cleanup stays owned after cancellation or failure.
    fn replay(&self, capture: Capture, cancel: Arc<AtomicBool>) {
        let start = Instant::now();
        for event in capture.events {
            if !wait_until(start + event.at, &cancel) {
                break;
            }
            if let Err(error) = self.dispatch(&event.kind, &cancel) {
                self.replay_error(error);
                break;
            }
        }
        let _ = wait_until(start + capture.duration, &cancel);
        if let Ok(mut state) = self.lock() {
            state.worker = false;
            state.phase = Phase::Stopping;
        }
        if let Err(error) = self.cleanup() {
            self.report(error);
        }
        let _ = self.publish();
    }
    /// Serializes cancellation against the final injection check; tracks only successful input.
    fn dispatch(&self, kind: &EventKind, cancel: &AtomicBool) -> Result<(), String> {
        let mut state = self.lock()?;
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        self.platform.inject(kind)?;
        track_held(&mut state.held, kind);
        Ok(())
    }
    /// Stops the worker on injection failure and gives failure precedence over successful completion.
    fn replay_error(&self, error: String) {
        if let Ok(mut state) = self.lock() {
            state.cancel.store(true, Ordering::Release);
            state.terminal = format!("Playback failed: {error}");
        }
    }
    /// Atomically reserves/releases a native confirmation interlock so global Record cannot race a Delete dialog.
    pub fn confirmation(&self, blocked: bool) -> Result<Snapshot, String> {
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        if blocked {
            let mut state = self.lock()?;
            if state.phase != Phase::Idle {
                return Err(ACTIVE_SESSION.into());
            }
            if state.confirmation.is_none() {
                state.confirmation = Some(self.platform.reserve()?);
            }
        } else {
            self.lock()?.confirmation = None;
        }
        self.idle_shortcuts()?;
        self.publish()?;
        self.snapshot()
    }
    /// Prevents exit until active work stops and any unsaved recording receives an explicit discard decision.
    pub fn request_close(&self) -> Result<bool, String> {
        let mut state = self.lock()?;
        let prevent = state.phase != Phase::Idle || !state.capture.events.is_empty();
        if prevent {
            state.close_requested = true;
        }
        drop(state);
        if prevent {
            self.signal_stop(false)?;
            self.publish()?;
        }
        Ok(prevent)
    }
    /// Keeps retained input available and cancels the pending exit decision without activating another window.
    pub fn keep_open(&self) -> Result<Snapshot, String> {
        self.lock()?.close_requested = false;
        self.publish()?;
        self.snapshot()
    }
    /// Allows explicit discard/exit only after native cleanup is complete.
    pub fn discard_exit(&self) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.phase != Phase::Idle {
            return Err(ACTIVE_SESSION.into());
        }
        state.capture = Capture::default();
        state.close_requested = false;
        Ok(())
    }
}
/// Selects truthful terminal capture feedback without claiming persistence.
fn recording_message(interrupted: bool, empty: bool) -> &'static str {
    if empty {
        EMPTY_CAPTURE
    } else if interrupted {
        INTERRUPTED
    } else {
        UNSAVED
    }
}
/// Updates conflict feedback without removing successful capture/terminal messages after availability recovery.
fn update_shortcut_message(state: &mut State, availability: Availability) {
    if !availability.stop {
        state.message = STOP_UNAVAILABLE.into();
    } else if !availability.record {
        state.message = RECORD_UNAVAILABLE.into();
    } else if state.message == STOP_UNAVAILABLE || state.message == RECORD_UNAVAILABLE {
        state.message = state.terminal.clone();
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
