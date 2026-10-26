//! Name-save ownership and arrival-bound session starts; disk work never holds session locks.
use super::*;

/// Worker-owned save reservation; dropping an unfinished worker releases waiters as a failed save.
pub(crate) struct RenameGuard {
    session: Arc<Session>,
    reservation: Option<Box<dyn Send>>,
}
impl Session {
    /// Claims a name-only save while idle; drafting itself never reserves the native session boundary.
    pub(crate) fn begin_rename(self: &Arc<Self>) -> Result<RenameGuard, String> {
        let _transition = self.transition.try_lock().map_err(|_| ACTIVE_SESSION)?;
        if !rename_available(&*self.lock()?) {
            return Err(ACTIVE_SESSION.into());
        }
        let reservation = self.platform.reserve()?;
        // Arm Stop before any deferred request can be accepted. Never hold the
        // session mutex while waiting for the native hook owner's acknowledgement.
        let shortcuts = self.platform.shortcuts(true, true)?;
        let mut state = self.lock()?;
        if !rename_available(&state) {
            return Err(ACTIVE_SESSION.into());
        }
        state.rename_saving = true;
        state.play_gesture = false;
        state.shortcuts = shortcuts;
        state.can_record = shortcuts.stop;
        drop(state);
        Ok(RenameGuard {
            session: self.clone(),
            reservation: Some(reservation),
        })
    }

    /// Waits only on a command worker until Save resolves or Stop/Close invalidates this start generation.
    pub(super) fn wait_for_rename(&self, request: StartRequest) -> Result<(), String> {
        if self.lock()?.rename_saving {
            self.publish()?;
        }
        let mut state = self.lock()?;
        while state.rename_saving
            && request.generation == state.generation
            && !state.close_requested
        {
            state = self.rename_ready.wait(state).map_err(|_| STATE_ERROR)?;
        }
        Ok(())
    }
}

/// Rejects writes when a previously received start owns priority, including while native Stop is being armed.
fn rename_available(state: &State) -> bool {
    start_available(state)
        && !state.rename_saving
        && !state.deferred_start
        && !state.start_requested
}
impl RenameGuard {
    /// Resolves Save, releases mutation ownership, and wakes starts; returns whether failure should discard the draft.
    pub(crate) fn finish(&mut self, succeeded: bool) -> Result<bool, String> {
        let _transition = self.session.transition.lock().map_err(|_| STATE_ERROR)?;
        let mut state = self.session.lock()?;
        let discard = state.deferred_start;
        if !succeeded {
            state.close_requested = false;
        }
        self.reservation.take();
        state.rename_saving = false;
        drop(state);
        self.session.rename_ready.notify_all();
        Ok(discard)
    }
}
impl Drop for RenameGuard {
    /// Fails closed on unwinding and wakes blocked workers without ever undoing an accepted disk commit.
    fn drop(&mut self) {
        if self.reservation.is_some() {
            let _ = self.finish(false);
        }
    }
}
