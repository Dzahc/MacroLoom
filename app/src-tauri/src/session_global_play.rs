//! Release-bound global Play intent, independent of native sequence consumption and session ownership.
use super::*;

pub const SELECT_TO_PLAY: &str = "Select a macro to play";

impl Session {
    /// Updates the selected identity without reading files or changing session ownership; native callers validate loaded IDs.
    pub(crate) fn select_macro(&self, macro_id: Option<String>) -> Result<(), String> {
        self.lock()?.selected_macro_id = macro_id;
        self.publish()
    }

    /// Clears only a successfully deleted selected identity; deletion publishes the final session snapshot after disk success.
    pub(crate) fn forget_macro(&self, id: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.selected_macro_id.as_deref() == Some(id) {
            state.selected_macro_id = None;
        }
        Ok(())
    }

    /// Arms an eligible bare F10 down without claiming a session or binding selection; busy presses remain ignored.
    pub(crate) fn press_play(&self) -> Result<bool, String> {
        let mut state = self.lock()?;
        state.play_gesture = command_available(&state) && state.can_record;
        Ok(state.play_gesture)
    }

    /// Reports whether an unreserved early F8 must invalidate unfinished or worker-queued startup without consuming that key.
    pub(crate) fn pending_start(&self) -> Result<bool, String> {
        let state = self.lock()?;
        Ok(state.play_gesture || state.start_requested || state.starting)
    }

    /// Binds selection and start identity at F10 release; no-selection feedback is separate from persistent status.
    pub(crate) fn release_play(&self) -> Result<Option<(StartRequest, String)>, String> {
        let mut state = self.lock()?;
        let armed = std::mem::take(&mut state.play_gesture);
        if !armed || !command_available(&state) || !state.can_record {
            return Ok(None);
        }
        let Some(id) = state.selected_macro_id.clone() else {
            state.notice_sequence = state
                .notice_sequence
                .checked_add(GENERATION_STEP)
                .ok_or(STATE_ERROR)?;
            state.notice = Some(SessionNotice {
                sequence: state.notice_sequence,
                message: SELECT_TO_PLAY.into(),
            });
            return Ok(None);
        };
        Ok(accept_start(&mut state)?.map(|request| (request, id)))
    }
}
