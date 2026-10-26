//! Recording-specific lifecycle; storage work never runs in an input callback or while holding session state.
use super::*;
use crate::recording::{self, RecordingOutcome};

impl Session {
    /// Rejects a keyboard event observed under a changed foreground layout before it can enter the saved stream.
    pub fn capture_with_layout(&self, kind: EventKind, layout: &str) {
        let Ok(mut state) = self.lock() else {
            return;
        };
        let changed = state
            .recording
            .as_ref()
            .is_some_and(|recording| recording.document.recording.keyboard_layout != layout);
        if changed && state.phase == Phase::Recording {
            let at = state.start.map_or(Duration::ZERO, |start| start.elapsed());
            freeze_automatic(&mut state, recording::ENVIRONMENT_CHANGED, at);
            return;
        }
        drop(state);
        self.capture_recording(kind);
    }
    /// Accounts for retained events before append; automatic stops freeze the valid prefix immediately.
    pub(super) fn capture_recording(&self, kind: EventKind) {
        let Ok(mut state) = self.lock() else {
            return;
        };
        if state.phase != Phase::Recording {
            return;
        }
        let Some(start) = state.start else {
            return;
        };
        let at = start.elapsed();
        if !state.capture.will_record(&kind) {
            return;
        }
        let State {
            capture, recording, ..
        } = &mut *state;
        if let Some(recording) = recording {
            if !recording.accept_capture(capture, at, &kind) {
                freeze_automatic(&mut state, recording::LIMIT_REACHED, at);
                return;
            }
        }
        state.capture.append(at, kind);
    }
    /// Checks environmental compatibility on the status worker and completes auto-stop cleanup once capture is frozen.
    pub(super) fn check_recording(&self) -> Result<(), String> {
        let observation = {
            let state = self.lock()?;
            if state.phase == Phase::Recording {
                state
                    .recording
                    .as_ref()
                    .map(|recording| (state.generation, recording.document.recording.clone()))
            } else {
                None
            }
        };
        if let Some((generation, environment)) = observation {
            if let Err(reason) = self.platform.check_environment(&environment) {
                self.interrupt_recording_for(generation, &reason)?;
            }
        }
        let request = {
            let state = self.lock()?;
            StopRequest {
                generation: state.generation,
            }
        };
        self.cleanup_for(request)
    }
    /// Applies an asynchronous environment failure only to the recording generation that produced its observation.
    fn interrupt_recording_for(&self, generation: u64, reason: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.generation == generation && state.phase == Phase::Recording {
            let at = state.start.map_or(Duration::ZERO, |start| start.elapsed());
            freeze_automatic(&mut state, reason, at);
        }
        Ok(())
    }
    /// Freezes an interrupted prefix; detected native failures use the same persistence path as explicit Stop.
    pub fn interrupt_recording(&self, reason: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.phase == Phase::Recording {
            let at = state.start.map_or(Duration::ZERO, |start| start.elapsed());
            freeze_automatic(&mut state, reason, at);
        }
        Ok(())
    }
    /// Publishes Saving in the restored full view, then commits owned data without holding the state mutex.
    pub(super) fn persist_recording(&self) -> Result<(), String> {
        let (mut recording, capture) = {
            let mut state = self.lock()?;
            state.save_failed = false;
            state.message = recording::SAVING.into();
            (
                state.recording.take().ok_or(STATE_ERROR)?,
                state.capture.clone(),
            )
        };
        self.publish()?;
        if recording.document.events.is_empty() {
            recording.freeze(&capture);
        }
        let result = self.platform.save_recording(&mut recording);
        self.recording_result(recording, result)?;
        self.idle_shortcuts()?;
        self.publish()
    }
    /// Retains success/failure receipts across event delivery failures; only commit success relinquishes operation ownership.
    fn recording_result(
        &self,
        recording: Recording,
        result: Result<crate::repository::recording_save::RecordingCommit, String>,
    ) -> Result<(), String> {
        let mut state = self.lock()?;
        state.outcome_sequence += 1;
        let outcome = recording_outcome(&recording, result, state.outcome_sequence);
        state.message = outcome.message.clone();
        state.terminal = outcome.message.clone();
        if let Some(context) = &mut state.interruption_context {
            *context = outcome.message.clone();
        }
        state.save_failed = !outcome.success;
        if outcome.success {
            state.selected_macro_id = Some(outcome.id.clone());
            state.capture = Capture::default();
            state.phase = Phase::Idle;
            state.reservation = None;
        } else {
            state.recording = Some(recording);
        }
        state.recording_outcome = Some(outcome);
        Ok(())
    }
    /// Retries the same frozen document once; concurrent Retry/Discard and new sessions are rejected.
    pub fn retry_recording(&self) -> Result<Snapshot, String> {
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        if !self.lock()?.save_failed {
            return Err(ACTIVE_SESSION.into());
        }
        self.persist_recording()?;
        self.snapshot()
    }
    /// Discards recoverable recording data only in response to an explicit action after failed persistence.
    pub fn discard_recording(&self) -> Result<Snapshot, String> {
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        let mut state = self.lock()?;
        if !state.save_failed || state.close_requested {
            return Err(ACTIVE_SESSION.into());
        }
        state.recording = None;
        state.capture = Capture::default();
        state.save_failed = false;
        state.phase = Phase::Idle;
        state.reservation = None;
        state.interruption_context = None;
        state.message = READY.into();
        state.terminal = READY.into();
        drop(state);
        self.idle_shortcuts()?;
        self.publish()?;
        self.snapshot()
    }
}

/// Marks a recording stopped without native calls; the status/control worker subsequently restores the window and saves.
fn freeze_automatic(state: &mut State, reason: &str, at: Duration) {
    state.capture.finish(at, (0, 0));
    if let Some(recording) = &mut state.recording {
        recording.reason = reason.into();
    }
    state.phase = Phase::Stopping;
    state.message = STOPPING.into();
    state.terminal = reason.into();
    state.can_record = false;
}
/// Converts a commit receipt to a user-visible outcome; errors retain persistent recovery, successes may carry refresh guidance.
fn recording_outcome(
    recording: &Recording,
    result: Result<crate::repository::recording_save::RecordingCommit, String>,
    sequence: u64,
) -> RecordingOutcome {
    let name = recording.document.name.clone();
    let (success, message, library) = match result {
        Ok(commit) => (
            true,
            format!("Saved “{name}”. {} {}", recording.reason, commit.warning)
                .trim()
                .into(),
            commit.library,
        ),
        Err(error) => (
            false,
            format!(
                "Could not save “{name}”. {} {error} {}",
                recording.reason,
                recording::SAVE_FAILED
            ),
            None,
        ),
    };
    RecordingOutcome {
        sequence,
        id: recording.document.id.clone(),
        name,
        success,
        message,
        library,
    }
}
