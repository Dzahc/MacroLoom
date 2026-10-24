//! Selected saved-macro startup and replay, sharing the session's generation, cancellation and cleanup owner.
use super::*;
use crate::playback::{self, MonotonicClock};

type Replay = (Capture, Arc<AtomicBool>, Instant);
const PLAYING_PREFIX: &str = "Playing";
const ONCE_STATUS: &str = "Run 1/1. F8 stops.";
const FAILURE_PREFIX: &str = "Playback failed";

impl Session {
    #[cfg(test)]
    /// Starts temporary playback through the same arrival-bound command boundary used by development controls.
    pub fn play(self: &Arc<Self>) -> Result<Snapshot, String> {
        self.play_requested(self.start_request()?.ok_or(ACTIVE_SESSION)?)
    }
    /// Starts an arrival-bound temporary session; delayed requests cannot operate on a newer generation.
    pub(crate) fn play_requested(
        self: &Arc<Self>,
        request: StartRequest,
    ) -> Result<Snapshot, String> {
        self.launch_replay(request, None)
    }
    /// Starts a selected saved macro on the worker; caller supplies only a trusted-library ID, never input data.
    pub(crate) fn play_macro_requested(
        self: &Arc<Self>,
        request: StartRequest,
        id: &str,
    ) -> Result<Snapshot, String> {
        self.launch_replay(request, Some(id))
    }
    /// Claims cancellation before repository/native work and launches only a fully prepared replay.
    fn launch_replay(
        self: &Arc<Self>,
        request: StartRequest,
        id: Option<&str>,
    ) -> Result<Snapshot, String> {
        let Some(transition) = self.start_transition(request, id.is_none())? else {
            return self.snapshot();
        };
        let result = self.start_replay(id);
        if result.is_err() {
            // A preparation error has not installed saved events; preserve any previous temporary capture.
            self.lock()?.playback = None;
        }
        drop(transition);
        self.finish_start(result.as_ref().err().map(String::as_str))?;
        if let Some((capture, cancel, start)) = result? {
            let session = self.clone();
            std::thread::spawn(move || session.replay(capture, cancel, start));
        }
        self.snapshot()
    }
    /// Reserves Stop, snapshots the file/properties and checks holds/environment without adding a countdown.
    fn start_replay(&self, id: Option<&str>) -> Result<Option<Replay>, String> {
        self.prepare(false)?;
        let document = id.map(|id| self.platform.playback(id)).transpose()?;
        self.validate_playback(document.as_ref())?;
        if !self.begin_replay(document)? {
            return Ok(None);
        }
        self.publish()?;
        let previous = self.lock()?.capture.clone();
        self.enter_view(previous)?;
        self.arm_replay()
    }
    /// Validates physical holds and recorded semantic environment; a rejected start never injects input.
    fn validate_playback(&self, document: Option<&MacroDocument>) -> Result<(), String> {
        self.platform.playback_ready()?;
        if let Some(document) = document {
            playback::validate_targets(document)?;
            self.platform
                .check_playback_environment(&document.recording)?;
        }
        Ok(())
    }
    /// Publishes preparation ownership without starting recorded time before compact entry completes.
    fn begin_replay(&self, document: Option<MacroDocument>) -> Result<bool, String> {
        let mut state = self.lock()?;
        if state.cancel.load(Ordering::Acquire) {
            return Ok(false);
        }
        state.message = match &document {
            Some(document) => format!("{PLAYING_PREFIX} {}. {ONCE_STATUS}", document.name),
            None => PLAYING.into(),
        };
        state.playback = document.map(Arc::new);
        state.phase = Phase::Playing;
        state.start = None;
        state.worker = true;
        state.terminal = if state.playback.is_some() {
            playback::COMPLETE
        } else {
            REPLAY_COMPLETE
        }
        .into();
        state.can_record = false;
        Ok(true)
    }
    /// Rechecks preparation, then shares a single epoch between deadlines and visible elapsed status.
    fn arm_replay(&self) -> Result<Option<Replay>, String> {
        let document = self.lock()?.playback.clone();
        let validation = self.validate_playback(document.as_deref());
        let capture = match &document {
            Some(document) => playback::capture(document),
            None => self.lock()?.capture.clone(),
        };
        let mut state = self.lock()?;
        if state.cancel.load(Ordering::Acquire) {
            state.playback = None;
            state.worker = false;
            return Ok(None);
        }
        validation?;
        if document.is_some() {
            state.capture = capture.clone();
            state.persistent = true;
        }
        let start = Instant::now();
        state.start = Some(start);
        Ok(Some((capture, state.cancel.clone(), start)))
    }
    /// Runs one absolute-deadline schedule; completion, cancellation and failure all converge on owned cleanup.
    fn replay(&self, capture: Capture, cancel: Arc<AtomicBool>, start: Instant) {
        let result = playback::run(&capture, &MonotonicClock(start), &cancel, |kind| {
            self.dispatch(kind, &cancel)
        });
        if let Err(error) = result {
            self.replay_error(error);
        }
        if let Err(error) = self.finish_replay() {
            self.report(error);
        }
        let _ = self.publish();
    }
    /// Relinquishes the in-flight dispatch owner before cleanup; stale workers cannot clean a later generation.
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
    /// Cancels before further dispatch and gives failure precedence over successful completion.
    fn replay_error(&self, error: String) {
        if let Ok(mut state) = self.lock() {
            state.cancel.store(true, Ordering::Release);
            state.terminal = format!("{FAILURE_PREFIX}: {error}");
            state.phase = Phase::Stopping;
        }
    }
    /// Checks cancellation before each native event and records only successful transitions for cleanup.
    fn dispatch(&self, kind: &EventKind, cancel: &AtomicBool) -> Result<(), String> {
        self.check_playback()?;
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        self.platform.inject(kind)?;
        // Worker ownership lasts through native completion and tracking, so Stop cannot release too early.
        track_held(&mut self.lock()?.held, kind);
        Ok(())
    }
    /// Observes saved playback compatibility on the status worker; late checks cannot interrupt a newer generation.
    pub(super) fn check_playback(&self) -> Result<(), String> {
        let observation = {
            let state = self.lock()?;
            state
                .playback
                .as_ref()
                .filter(|_| state.phase == Phase::Playing)
                .map(|document| (state.generation, document.recording.clone()))
        };
        if let Some((generation, environment)) = observation {
            if let Err(error) = self.platform.check_playback_environment(&environment) {
                self.interrupt_playback_for(generation, &error)?;
            }
        }
        Ok(())
    }
    /// Applies environment failure only to its observed owner and lets the worker perform held-input cleanup.
    fn interrupt_playback_for(&self, generation: u64, reason: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        if generation != state.generation || state.phase != Phase::Playing {
            return Ok(());
        }
        state.cancel.store(true, Ordering::Release);
        state.phase = Phase::Stopping;
        state.message = STOPPING.into();
        state.terminal = reason.into();
        Ok(())
    }
}
