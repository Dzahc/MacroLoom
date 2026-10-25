//! Exit coordination with library mutations and acknowledged native resource shutdown.
use super::*;

impl Session {
    /// Gates editor creation/submission atomically against Close/start, retaining the editor's typed validation result.
    /// The callback may only update the short-lived editor interlock, never perform disk/UI work or reenter Session.
    /// Successful actions acknowledge interruption; outer errors reject ownership before executing the callback.
    pub fn editor_operation<R, E>(
        &self,
        operation: impl FnOnce() -> Result<R, E>,
    ) -> Result<Result<R, E>, String> {
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        let mut state = self.lock()?;
        if !start_available(&state) {
            return Err(ACTIVE_SESSION.into());
        }
        let result = operation();
        if result.is_ok() {
            acknowledge_interruption(&mut state);
            state.can_record = false;
        }
        drop(state);
        self.publish()?;
        Ok(result)
    }
    /// Attempts final native teardown only after all session/mutation work and saving completed; failure waits for explicit Retry/F8.
    /// Returns true only when exit is safe. Never blocks a UI callback: callers use a worker.
    pub fn finish_exit(&self) -> Result<bool, String> {
        let Ok(_transition) = self.transition.try_lock() else {
            return Ok(false);
        };
        if !exit_available(&*self.lock()?) {
            return Ok(false);
        }
        if let Err(error) = self.platform.shutdown() {
            self.cleanup_error(format!("{EXIT_CLEANUP_FAILED} {error}"))?;
            self.publish()?;
            return Ok(false);
        }
        Ok(true)
    }
    /// Captures close cleanup ownership without turning repeated native Close into explicit cleanup Retry.
    pub(crate) fn close_cleanup(&self) -> Result<StopRequest, String> {
        Ok(StopRequest {
            generation: self.lock()?.generation,
        })
    }
    /// Executes confirmed deletion under session/operation exclusion; Close waits for its result and failure cancels exit.
    /// The supplied disk operation runs on its caller's worker without holding session state.
    pub fn delete<R>(
        &self,
        owner: Option<&str>,
        operation: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        let reservation = self.take_confirmation(owner)?;
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        let reservation = match reservation {
            Some(reservation) => reservation,
            None => self.platform.reserve()?,
        };
        self.begin_deletion()?;
        let result = operation();
        {
            let mut state = self.lock()?;
            state.deleting = false;
            if result.is_err() {
                state.close_requested = false;
            }
        }
        drop(reservation);
        if let Err(error) = self.idle_shortcuts() {
            self.report(error);
        }
        self.publish()?;
        result
    }
    /// Marks mutation ownership before disk work, rejecting a Close that won the reservation race.
    fn begin_deletion(&self) -> Result<(), String> {
        let mut state = self.lock()?;
        if !start_available(&state) {
            return Err(ACTIVE_SESSION.into());
        }
        state.deleting = true;
        acknowledge_interruption(&mut state);
        state.can_record = false;
        Ok(())
    }
}

/// Clears only a prior interruption explanation when a deliberate new application operation has been accepted.
pub(super) fn acknowledge_interruption(state: &mut State) {
    if state.interruption_context.take().is_some() {
        state.message = READY.into();
        state.terminal = READY.into();
    }
}

/// Selects a terminal close with no outstanding session, deletion, recording, or failed cleanup ownership.
fn exit_available(state: &State) -> bool {
    state.close_requested
        && state.phase == Phase::Idle
        && !state.starting
        && !state.deleting
        && !state.cleanup_failed
        && state.capture.events.is_empty()
        && state.recording.is_none()
}
