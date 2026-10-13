//! Arrival and execution boundaries for native controls queued behind session work.
use crate::session::{Session, Snapshot, StartRequest, StopRequest};
use std::sync::Arc;

#[derive(Clone, Copy)]
/// Native control observed now; Stop and suspend signal cancellation before worker delivery.
pub(crate) enum ControlRequest {
    Record,
    Stop,
    Suspend,
}
#[derive(Clone, Copy)]
/// Control awaiting serialized backend execution rather than frontend event delivery.
pub(crate) enum QueuedControl {
    Record(StartRequest),
    Cleanup(StopRequest),
}
impl ControlRequest {
    /// Signals cancellation immediately when applicable and returns work for the native control queue.
    pub(crate) fn receive(self, session: &Session) -> Result<Option<QueuedControl>, String> {
        match self {
            Self::Record => session
                .start_request()
                .map(|request| request.map(QueuedControl::Record)),
            control => session
                .stop_request(matches!(control, Self::Suspend))
                .map(QueuedControl::Cleanup)
                .map(Some),
        }
    }
}
impl QueuedControl {
    /// Executes one previously received control through the shared session command boundary.
    pub(crate) fn execute(self, session: &Arc<Session>) -> Result<Snapshot, String> {
        match self {
            Self::Record(request) => session.record_requested(request),
            Self::Cleanup(request) => session.complete_stop(request),
        }
    }
}
