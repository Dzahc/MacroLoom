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
        let mut state = self.lock()?;
        if !start_available(&state) || state.rename_saving || state.deferred_start {
            return Err(ACTIVE_SESSION.into());
        }
        let reservation = self.platform.reserve()?;
        state.rename_saving = true;
        drop(state);
        Ok(RenameGuard {
            session: self.clone(),
            reservation: Some(reservation),
        })
    }

    /// Waits only on a command worker until Save resolves or Stop/Close invalidates this start generation.
    pub(super) fn wait_for_rename(&self, request: StartRequest) -> Result<(), String> {
        self.prepare_rename_wait()?;
        let mut state = self.lock()?;
        while state.rename_saving
            && request.generation == state.generation
            && !state.close_requested
        {
            state = self.rename_ready.wait(state).map_err(|_| STATE_ERROR)?;
        }
        Ok(())
    }
    /// Registers pending-start Stop on its native owner before a command worker begins waiting.
    fn prepare_rename_wait(&self) -> Result<(), String> {
        if !self.lock()?.rename_saving {
            return Ok(());
        }
        if let Err(error) = self.idle_shortcuts() {
            self.signal_stop(false)?;
            return Err(error);
        }
        self.publish()
    }
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
