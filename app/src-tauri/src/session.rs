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
const CONFIRMATION_OWNER_MAX: usize = 64;
const INVALID_CONFIRMATION_OWNER: &str = "Invalid confirmation owner";
const NO_CLOSE_REQUEST: &str = "No pending exit decision";
const GENERATION_STEP: u64 = 1;
const START_CANCELLED: &str = "Session start cancelled.";
const START_INTERRUPTED: &str = "Session start interrupted by computer sleep.";

#[derive(Clone, Copy)]
/// Opaque eligibility observed at command arrival; only the receiving session may execute this request.
pub(crate) struct StartRequest {
    generation: u64,
}
#[derive(Clone, Copy)]
/// Cleanup ownership captured when Stop/suspend signals cancellation; cannot affect a later session generation.
pub(crate) struct StopRequest {
    generation: u64,
}

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
    generation: u64,
    starting: bool,
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
    confirmation_owner: Option<String>,
}
impl Default for State {
    /// Initializes an idle owner with no input, cancellation, or reservations.
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            generation: 0,
            starting: false,
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
            confirmation_owner: None,
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
            can_record: state.can_record && state.phase == Phase::Idle && !state.starting,
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
        if self.lock()?.phase != Phase::Idle {
            return Ok(());
        }
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
        Ok(state.confirmation.is_none() && !state.close_requested && !state.starting)
    }
    /// Explicitly reprobes global shortcuts while idle; no recording or playback is started by retry.
    pub fn retry_shortcuts(&self) -> Result<Snapshot, String> {
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        if !start_available(&*self.lock()?) {
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
    fn prepare(&self, recording: bool) -> Result<(), String> {
        let reservation = self.platform.reserve()?;
        self.lock()?.reservation = Some(reservation);
        if self.lock()?.cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        let shortcuts = self.platform.shortcuts(recording, true)?;
        self.lock()?.shortcuts = shortcuts;
        if !shortcuts.stop {
            return Err(STOP_UNAVAILABLE.into());
        }
        Ok(())
    }
    /// Captures Record/Play eligibility without native work; a queued request cannot reinterpret a later Idle state as its original intent.
    pub(crate) fn start_request(&self) -> Result<Option<StartRequest>, String> {
        let state = self.lock()?;
        Ok(start_available(&state).then_some(StartRequest {
            generation: state.generation,
        }))
    }
    /// Claims cancellation ownership before any wait or native reservation, ignoring requests superseded by another start or idle Stop.
    fn claim_start(&self, request: StartRequest, playback: bool) -> Result<bool, String> {
        let mut state = self.lock()?;
        if request.generation != state.generation {
            return Ok(false);
        }
        if !start_available(&state) {
            return Err(ACTIVE_SESSION.into());
        }
        if playback && state.capture.events.is_empty() {
            return Err(NO_CAPTURE.into());
        }
        state.generation = state
            .generation
            .checked_add(GENERATION_STEP)
            .ok_or(STATE_ERROR)?;
        state.starting = true;
        state.can_record = false;
        state.cancel = Arc::new(AtomicBool::new(false));
        Ok(true)
    }
    /// Serializes native startup after claiming its identity; cancellation remains effective while waiting for other transitions.
    fn start_transition(
        &self,
        request: StartRequest,
        playback: bool,
    ) -> Result<Option<MutexGuard<'_, ()>>, String> {
        if !self.claim_start(request, playback)? {
            return Ok(None);
        }
        self.transition
            .lock()
            .map(Some)
            .map_err(|_| STATE_ERROR.into())
    }
    #[cfg(any(test, feature = "native-checks"))]
    /// Receives and runs a Record command synchronously for deterministic and native integration checks.
    pub fn record(&self) -> Result<Snapshot, String> {
        let request = self.start_request()?.ok_or(ACTIVE_SESSION)?;
        self.record_requested(request)
    }
    /// Executes an arrival-bound Record request; obsolete queue entries return current state without replacing capture.
    pub(crate) fn record_requested(&self, request: StartRequest) -> Result<Snapshot, String> {
        let Some(transition) = self.start_transition(request, false)? else {
            return self.snapshot();
        };
        let result = self.start_recording();
        drop(transition);
        self.finish_start(result.as_ref().err().map(String::as_str))?;
        result?;
        self.snapshot()
    }
    /// Validates native ownership before capture activation, leaving previous data untouched when startup was cancelled.
    fn start_recording(&self) -> Result<(), String> {
        self.prepare(true)?;
        let Some(previous) = self.begin_record()? else {
            return Ok(());
        };
        self.publish()?;
        self.enter_view(previous)
    }
    /// Claims recording data and returns the previous capture for rollback if native entry fails.
    fn begin_record(&self) -> Result<Option<Capture>, String> {
        let mut state = self.lock()?;
        if state.cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let previous = std::mem::take(&mut state.capture);
        state.phase = Phase::Recording;
        state.start = Some(Instant::now());
        state.message = RECORDING.into();
        state.can_record = false;
        state.cleanup_failed = false;
        Ok(Some(previous))
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
    fn finish_start(&self, failure: Option<&str>) -> Result<(), String> {
        let request = {
            let mut state = self.lock()?;
            state.starting = false;
            if let Some(error) = failure {
                state.worker = false;
                if state.phase != Phase::Stopping {
                    state.phase = Phase::Stopping;
                    state.terminal = error.into();
                }
            }
            StopRequest {
                generation: state.generation,
            }
        };
        self.cleanup_for(request)?;
        self.publish()?;
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
    /// Runs a controlled presentation update only while Idle, holding the same transition guard as real session starts.
    pub fn preview<R>(&self, update: impl FnOnce() -> Result<R, String>) -> Result<R, String> {
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        let state = self.lock()?;
        if state.phase != Phase::Idle || state.starting || state.close_requested {
            return Err(ACTIVE_SESSION.into());
        }
        drop(state);
        update()
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
        self.stop_request(interrupted).map(|_| ())
    }
    /// Signals cancellation at arrival, including pre-activation startup; idle signals invalidate pending start requests.
    pub(crate) fn stop_request(&self, interrupted: bool) -> Result<StopRequest, String> {
        let mut state = self.lock()?;
        let request = StopRequest {
            generation: state.generation,
        };
        match state.phase {
            Phase::Idle => {
                if !state.starting {
                    state.generation = state
                        .generation
                        .checked_add(GENERATION_STEP)
                        .ok_or(STATE_ERROR)?;
                    return Ok(StopRequest {
                        generation: state.generation,
                    });
                }
                state.cancel.store(true, Ordering::Release);
                state.terminal = if interrupted {
                    START_INTERRUPTED
                } else {
                    START_CANCELLED
                }
                .into();
            }
            Phase::Stopping => return Ok(request),
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
        Ok(request)
    }
    #[cfg(any(test, feature = "native-checks"))]
    /// Receives and completes Stop synchronously for deterministic and native integration checks.
    pub fn stop(&self) -> Result<Snapshot, String> {
        self.complete_stop(self.stop_request(false)?)
    }
    /// Completes previously signalled cancellation for its original owner; stale cleanup jobs cannot stop later sessions.
    pub(crate) fn complete_stop(&self, request: StopRequest) -> Result<Snapshot, String> {
        self.cleanup_for(request)?;
        self.publish()?;
        self.snapshot()
    }
    #[cfg(test)]
    /// Receives and completes sleep interruption synchronously for deterministic session checks.
    pub fn suspend(&self) -> Result<Snapshot, String> {
        self.complete_stop(self.stop_request(true)?)
    }
    /// Checks cleanup identity and worker/startup ownership without performing native work.
    fn cleanup_pending(&self, request: StopRequest) -> Result<bool, String> {
        let state = self.lock()?;
        Ok(state.generation == request.generation
            && state.phase == Phase::Stopping
            && !state.starting
            && !state.worker)
    }
    /// Serializes native cleanup and rechecks its original owner after waiting, preventing stale jobs from touching new sessions.
    fn cleanup_for(&self, request: StopRequest) -> Result<(), String> {
        if !self.cleanup_pending(request)? {
            return Ok(());
        }
        let _transition = self.transition.lock().map_err(|_| STATE_ERROR)?;
        if !self.cleanup_pending(request)? {
            return Ok(());
        }
        self.release_outstanding()?;
        if !self.lock()?.held.is_empty() {
            return self.cleanup_error(CLEANUP_FAILED.into());
        }
        if let Err(error) = self.platform.view(false) {
            return self.cleanup_error(format!("{CLEANUP_FAILED} {error}"));
        }
        self.complete_cleanup()
    }
    /// Makes resource-release failure actionable while allowing idle Record conflicts to remain ordinary availability errors.
    fn complete_cleanup(&self) -> Result<(), String> {
        if let Err(error) = self.finish_cleanup() {
            if self.lock()?.phase == Phase::Stopping {
                self.cleanup_error(format!("{CLEANUP_FAILED} {error}"))?;
            } else {
                self.report(error);
            }
        }
        Ok(())
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
    #[cfg(test)]
    /// Receives and starts temporary replay synchronously for deterministic session checks.
    pub fn play(self: &Arc<Self>) -> Result<Snapshot, String> {
        let request = self.start_request()?.ok_or(ACTIVE_SESSION)?;
        self.play_requested(request)
    }
    /// Starts arrival-bound temporary playback only if its request still owns the original idle generation.
    pub(crate) fn play_requested(
        self: &Arc<Self>,
        request: StartRequest,
    ) -> Result<Snapshot, String> {
        let Some(transition) = self.start_transition(request, true)? else {
            return self.snapshot();
        };
        let result = self.start_replay();
        drop(transition);
        self.finish_start(result.as_ref().err().map(String::as_str))?;
        if let Some((capture, cancel)) = result? {
            let session = self.clone();
            std::thread::spawn(move || session.replay(capture, cancel));
        }
        self.snapshot()
    }
    /// Prepares playback and compact entry without resetting a cancellation received during native reservations.
    fn start_replay(&self) -> Result<Option<(Capture, Arc<AtomicBool>)>, String> {
        self.prepare(false)?;
        let Some((capture, cancel)) = self.begin_replay()? else {
            return Ok(None);
        };
        self.publish()?;
        self.enter_view(capture.clone())?;
        Ok(Some((capture, cancel)))
    }
    /// Claims playback ownership and a stable temporary capture before native layout work starts.
    fn begin_replay(&self) -> Result<Option<(Capture, Arc<AtomicBool>)>, String> {
        let mut state = self.lock()?;
        if state.cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        state.phase = Phase::Playing;
        state.start = Some(Instant::now());
        state.worker = true;
        state.message = PLAYING.into();
        state.terminal = REPLAY_COMPLETE.into();
        state.can_record = false;
        Ok(Some((state.capture.clone(), state.cancel.clone())))
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
        if let Err(error) = self.finish_replay() {
            self.report(error);
        }
        let _ = self.publish();
    }
    /// Relinquishes dispatch and captures cleanup identity under one state guard; delayed completion cannot clean up a newer generation.
    fn finish_replay(&self) -> Result<(), String> {
        let request = {
            let mut state = self.lock()?;
            state.worker = false;
            state.phase = Phase::Stopping;
            StopRequest {
                generation: state.generation,
            }
        };
        self.cleanup_for(request)
    }
    /// Checks cancellation before native injection and tracks success afterwards; never holds session state across a callback-producing OS call.
    fn dispatch(&self, kind: &EventKind, cancel: &AtomicBool) -> Result<(), String> {
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        self.platform.inject(kind)?;
        // The playback worker retains ownership until this in-flight call and tracking complete, so Stop cannot start cleanup early.
        track_held(&mut self.lock()?.held, kind);
        Ok(())
    }
    /// Stops the worker on injection failure and gives failure precedence over successful completion.
    fn replay_error(&self, error: String) {
        if let Ok(mut state) = self.lock() {
            state.cancel.store(true, Ordering::Release);
            state.terminal = format!("Playback failed: {error}");
            state.phase = Phase::Stopping;
        }
    }
    /// Atomically reserves/releases a native confirmation interlock so global Record cannot race a Delete dialog.
    pub fn confirmation(&self, owner: &str, blocked: bool) -> Result<Snapshot, String> {
        validate_confirmation_owner(owner)?;
        let _transition = self.transition.lock().map_err(|_| STATE_ERROR)?;
        if blocked {
            self.reserve_confirmation(owner)?;
        } else {
            self.release_confirmation(owner)?;
        }
        self.idle_shortcuts()?;
        self.publish()?;
        self.snapshot()
    }
    /// Claims a confirmation for exactly one frontend owner; stale/reconnected owners cannot reuse another dialog's reservation.
    fn reserve_confirmation(&self, owner: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.phase != Phase::Idle || state.starting {
            return Err(ACTIVE_SESSION.into());
        }
        if let Some(existing) = &state.confirmation_owner {
            if existing != owner {
                return Err(ACTIVE_SESSION.into());
            }
        }
        if state.confirmation.is_none() {
            state.confirmation = Some(self.platform.reserve()?);
            state.confirmation_owner = Some(owner.into());
        }
        Ok(())
    }
    /// Releases only the calling owner's confirmation, leaving a newer owner's reservation untouched.
    fn release_confirmation(&self, owner: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.confirmation_owner.as_deref() == Some(owner) {
            state.confirmation = None;
            state.confirmation_owner = None;
        }
        Ok(())
    }
    /// Transfers confirmed-dialog operation ownership directly to deletion without permitting an intervening session start.
    pub fn take_confirmation(&self, owner: Option<&str>) -> Result<Option<Box<dyn Send>>, String> {
        let _transition = self.transition.lock().map_err(|_| STATE_ERROR)?;
        let mut state = self.lock()?;
        if state.phase != Phase::Idle || state.starting {
            return Err(ACTIVE_SESSION.into());
        }
        if state.confirmation_owner.as_deref() != owner {
            return Err(ACTIVE_SESSION.into());
        }
        state.confirmation_owner = None;
        Ok(state.confirmation.take())
    }
    /// Prevents exit until active work stops and any unsaved recording receives an explicit discard decision.
    pub fn request_close(&self) -> Result<bool, String> {
        let mut state = self.lock()?;
        let prevent =
            state.phase != Phase::Idle || state.starting || !state.capture.events.is_empty();
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
        if state.phase != Phase::Idle || state.starting {
            return Err(ACTIVE_SESSION.into());
        }
        if !state.close_requested {
            return Err(NO_CLOSE_REQUEST.into());
        }
        state.capture = Capture::default();
        Ok(())
    }
}
/// Returns whether this idle generation can accept a new session intent; native availability is revalidated during preparation.
fn start_available(state: &State) -> bool {
    state.phase == Phase::Idle
        && !state.starting
        && state.confirmation.is_none()
        && !state.close_requested
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
/// Validates externally supplied owner IDs before they can acquire or release native operation ownership.
fn validate_confirmation_owner(owner: &str) -> Result<(), String> {
    if owner.is_empty() || owner.len() > CONFIRMATION_OWNER_MAX || !owner.is_ascii() {
        return Err(INVALID_CONFIRMATION_OWNER.into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
