//! Opt-in development diagnostics for lost controls; release builds never enable tracing.

use crate::session::Snapshot;
use crate::shortcuts::{RECORD_KEY, STOP_KEY};
use std::fmt::Debug;
use std::sync::{mpsc, OnceLock};

const ENVIRONMENT: &str = "MACROLOOM_TRACE_SHORTCUTS";
const ENABLED: &str = "1";
const PREFIX: &str = "[ML-14 shortcut trace]";
static OUTPUT: OnceLock<Option<mpsc::Sender<String>>> = OnceLock::new();

#[derive(Debug)]
/// Metadata for an F8/F9 callback only; excludes ordinary keys, recorded contents, and injected text.
pub(crate) struct KeyTrace {
    pub key: u32,
    pub flags: u32,
    pub injected: bool,
    pub message: usize,
    pub app_focused: bool,
    pub record_reserved: bool,
    pub stop_reserved: bool,
    pub consumed: bool,
}

/// Initializes opt-in output before hooks start; terminal writes run on a separate worker and never delay native callbacks.
pub(crate) fn start() {
    if !cfg!(debug_assertions) {
        return;
    }
    OUTPUT.get_or_init(|| {
        if std::env::var(ENVIRONMENT).as_deref() != Ok(ENABLED) {
            return None;
        }
        let (sender, receiver) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            eprintln!(
                "{PREFIX} enabled; key={RECORD_KEY} is F9, key={STOP_KEY} is F8; only control metadata is logged"
            );
            for line in receiver {
                eprintln!("{PREFIX} {line}");
            }
        });
        Some(sender)
    });
}
/// Returns whether diagnostics are enabled, allowing callbacks to avoid even collecting metadata in ordinary runs.
pub(crate) fn enabled() -> bool {
    matches!(OUTPUT.get(), Some(Some(_)))
}
/// Queues a control-key metadata record without waiting for terminal output; a disconnected observer does not affect session behavior.
pub(crate) fn key(value: KeyTrace) {
    if let Some(Some(sender)) = OUTPUT.get() {
        let KeyTrace {
            key,
            flags,
            injected,
            message,
            app_focused,
            record_reserved,
            stop_reserved,
            consumed,
        } = value;
        let _ = sender.send(format!("key={key} flags={flags} injected={injected} message={message} app_focused={app_focused} record_reserved={record_reserved} stop_reserved={stop_reserved} consumed={consumed}"));
    }
}
/// Queues the backend outcome for a native control, including rejection; never reports a frontend-only success.
pub(crate) fn completed(control: impl Debug, result: &Result<Snapshot, String>) {
    if let Some(Some(sender)) = OUTPUT.get() {
        let outcome = match result {
            Ok(snapshot) => format!("phase={:?} message={}", snapshot.phase, snapshot.message),
            Err(reason) => format!("error={reason}"),
        };
        let _ = sender.send(format!("control={control:?} {outcome}"));
    }
}
