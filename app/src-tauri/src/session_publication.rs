//! Ordered session-to-presentation delivery across status and command workers.
use crate::session::Snapshot;
use std::sync::Mutex;

const PUBLICATION_UNAVAILABLE: &str = "Session publication is unavailable; restart MacroLoom.";

#[derive(Default)]
/// Serializes snapshot delivery and drops delayed/duplicate revisions before they can change native presentation.
pub struct Publications {
    revision: Mutex<Option<u64>>,
}
impl Publications {
    /// Delivers a newer owned snapshot synchronously; `deliver` must not reenter this gate and never runs under the session-state lock.
    pub fn send(&self, snapshot: Snapshot, deliver: impl FnOnce(Snapshot)) -> Result<(), String> {
        let mut revision = self.revision.lock().map_err(|_| PUBLICATION_UNAVAILABLE)?;
        if revision.is_some_and(|current| snapshot.revision <= current) {
            return Ok(());
        }
        *revision = Some(snapshot.revision);
        deliver(snapshot);
        Ok(())
    }
}
