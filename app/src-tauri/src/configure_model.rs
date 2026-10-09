//! Typed Configure boundary and submission state, independent of native window dispatch.

use crate::macro_format::{self, MacroDocument, PlaybackProperties, MAX_SAFE_INTEGER};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::mpsc::Sender;

pub const CONFIGURE_BUSY: &str = "Close Configure before starting another operation";
pub const CONFIGURE_MISSING: &str = "Configure is no longer available";
pub const CONFIGURE_PENDING: &str = "Wait for Save to finish";
pub const CALLBACK_FAILED: &str =
    "Could not save changes. Your edits are retained; try again or Cancel.";
const WRONG_TARGET: &str = "Configure draft does not match its original macro";
const INVALID_RESULT: &str = "Invalid Configure callback result";
const MAX_ERROR_CHARS: usize = 2048;
const ERROR_FIELDS: [&str; 5] = ["name", "speed", "repeatMode", "totalRuns", "interval"];

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Complete property draft; no events, file paths, IDs to rename, or timestamps may be edited.
pub struct ConfigureDraft {
    pub macro_id: String,
    pub name: String,
    pub playback: PlaybackProperties,
}

impl ConfigureDraft {
    /// Reduces the supplied validated macro to local property inputs, preserving stable identity.
    pub fn from_document(document: MacroDocument) -> Self {
        Self {
            macro_id: document.id,
            name: document.name,
            playback: document.playback,
        }
    }
    /// Trims and validates external drafts against the frozen identity before any callback is emitted.
    /// Rejects unsupported numeric presets/ranges and metadata injections into playback properties.
    pub fn validate(&mut self, original: &Self) -> Result<(), String> {
        if self.macro_id != original.macro_id {
            return Err(WRONG_TARGET.into());
        }
        self.name = self.name.trim().into();
        macro_format::validate_name(&self.name)
            .map_err(|error| format!("{}: {}", error.field, error.message))?;
        macro_format::validate_playback(&self.playback)
            .map_err(|error| format!("{}: {}", error.field, error.message))?;
        if !self.playback.extra.is_empty() {
            return Err(INVALID_RESULT.into());
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Async consumer acknowledgement; failure carries persistent actionable text and optional field errors.
pub struct ConfigureResult {
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

impl ConfigureResult {
    /// Constructs a retained form error when the consumer bridge fails; never claims persistence success.
    pub fn failure() -> Self {
        Self {
            ok: false,
            message: CALLBACK_FAILED.into(),
            fields: HashMap::new(),
        }
    }
    /// Bounds external error text and accepts only fields rendered by the editor.
    pub fn validate(&self) -> Result<(), String> {
        if self.message.chars().count() > MAX_ERROR_CHARS {
            return Err(INVALID_RESULT.into());
        }
        if !self.ok && self.message.trim().is_empty() {
            return Err(INVALID_RESULT.into());
        }
        for (field, text) in &self.fields {
            if !ERROR_FIELDS.contains(&field.as_str()) || text.chars().count() > MAX_ERROR_CHARS {
                return Err(INVALID_RESULT.into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Once-per-attempt event delivered only to the main window's supplied asynchronous consumer.
pub struct ConfigureSubmission {
    pub attempt_id: u64,
    pub draft: ConfigureDraft,
}

/// Active callback channel retained until the main consumer acknowledges the matching attempt.
pub struct PendingSave {
    pub attempt_id: u64,
    pub sender: Sender<ConfigureResult>,
}

#[derive(Default)]
/// Singleton editor reservation, local snapshot, callback guard and deferred application-exit intent.
pub struct ConfigureState {
    pub occupied: bool,
    pub operation: bool,
    pub original: Option<ConfigureDraft>,
    pub pending: Option<PendingSave>,
    pub next_attempt: u64,
    pub exit_requested: bool,
    pub save_succeeded: bool,
}

impl ConfigureState {
    /// Reserves an editor before asynchronous disk reads; concurrent operation/open requests fail without changing state.
    pub fn reserve(&mut self) -> Result<(), String> {
        if self.occupied || self.operation {
            return Err(CONFIGURE_BUSY.into());
        }
        self.occupied = true;
        Ok(())
    }
    /// Creates one submission guard after native validation; reentrant or repeated Save cannot replace the active callback.
    pub fn begin(
        &mut self,
        mut draft: ConfigureDraft,
        sender: Sender<ConfigureResult>,
    ) -> Result<ConfigureSubmission, String> {
        if self.pending.is_some() {
            return Err(CONFIGURE_PENDING.into());
        }
        if self.save_succeeded {
            return Err(CONFIGURE_MISSING.into());
        }
        let original = self.original.as_ref().ok_or(CONFIGURE_MISSING)?;
        draft.validate(original)?;
        if self.next_attempt >= MAX_SAFE_INTEGER {
            return Err(CONFIGURE_MISSING.into());
        }
        self.next_attempt += 1;
        self.pending = Some(PendingSave {
            attempt_id: self.next_attempt,
            sender,
        });
        self.save_succeeded = false;
        Ok(ConfigureSubmission {
            attempt_id: self.next_attempt,
            draft,
        })
    }
    /// Completes an acknowledged attempt; failed Save cancels deferred exit so recoverable edits remain open.
    pub fn settle(&mut self, result: &ConfigureResult) {
        self.pending = None;
        self.save_succeeded = result.ok;
        if !result.ok {
            self.exit_requested = false;
        }
    }
    /// Checks an idle native operation independently of presentation availability.
    pub fn require_operation(&self) -> Result<(), String> {
        if self.occupied || self.operation {
            Err(CONFIGURE_BUSY.into())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macro_format::{RepeatMode, MAX_NAME_CHARS, SPEEDS};
    const ID: &str = "68833c94-3d18-4fb2-8a59-0cc72382b616";
    const WRONG_ID: &str = "78833c94-3d18-4fb2-8a59-0cc72382b616";
    const NAME: &str = "Fill form";
    const EMOJI: &str = "🧶";
    const INVALID_SPEED: f64 = 3.0;
    const FIRST_RUN: u64 = 1;

    /// Returns a complete minimal property snapshot matching the existing storage contract.
    fn draft() -> ConfigureDraft {
        ConfigureDraft {
            macro_id: ID.into(),
            name: NAME.into(),
            playback: PlaybackProperties {
                speed: SPEEDS[2],
                repeat_mode: RepeatMode::Once,
                total_runs: FIRST_RUN,
                interval_ms: 0,
                extra: Default::default(),
            },
        }
    }
    #[test]
    /// Native validation trims names, counts emoji as scalar values, and refuses identity/range changes before emitting a callback.
    fn validates_external_drafts_using_storage_rules() {
        let original = draft();
        let mut value = draft();
        value.name = format!("  {}  ", EMOJI.repeat(MAX_NAME_CHARS));
        assert!(value.validate(&original).is_ok());
        value.name.push_str(EMOJI);
        assert!(value.validate(&original).is_err());
        value = draft();
        value.macro_id = WRONG_ID.into();
        assert!(value.validate(&original).is_err());
        value = draft();
        value.playback.speed = INVALID_SPEED;
        assert!(value.validate(&original).is_err());
        value = draft();
        value.playback.total_runs = 0;
        assert!(value.validate(&original).is_err());
        value = draft();
        value.playback.interval_ms = MAX_SAFE_INTEGER + FIRST_RUN;
        assert!(value.validate(&original).is_err());
    }
    #[test]
    /// Submission and operation guards hold throughout a callback; failed acknowledgement unlocks Retry while clearing queued exit.
    fn failure_retains_snapshot_and_allows_a_new_attempt() {
        let mut state = ConfigureState::default();
        assert!(state.reserve().is_ok());
        assert!(state.require_operation().is_err());
        state.original = Some(draft());
        let (sender, _) = std::sync::mpsc::channel();
        let first = state
            .begin(draft(), sender.clone())
            .expect("Valid reserved editor accepts its first draft");
        assert!(state.begin(draft(), sender.clone()).is_err());
        state.exit_requested = true;
        state.settle(&ConfigureResult::failure());
        assert!(!state.exit_requested);
        assert!(state.original.is_some());
        let retry = state
            .begin(draft(), sender)
            .expect("Failure releases only the submission guard");
        assert!(retry.attempt_id > first.attempt_id);
    }

    #[test]
    /// Successful acknowledgement preserves queued exit and prevents another submission from the completed editor.
    fn success_preserves_exit_and_finishes_the_attempt() {
        let mut state = ConfigureState {
            occupied: true,
            original: Some(draft()),
            exit_requested: true,
            ..ConfigureState::default()
        };
        let (sender, _) = std::sync::mpsc::channel();
        state
            .begin(draft(), sender.clone())
            .expect("Reserved editor accepts its valid properties");
        let result = ConfigureResult {
            ok: true,
            message: String::new(),
            fields: HashMap::new(),
        };
        state.settle(&result);
        assert!(state.exit_requested);
        assert!(state.pending.is_none());
        assert!(state.begin(draft(), sender).is_err());
    }

    #[test]
    /// The draft/acknowledgement JSON contract uses camelCase and accepts actionable field failures, rejecting malformed errors.
    fn boundary_shapes_and_acknowledgement_validation_are_explicit() {
        const MACRO_ID_FIELD: &str = "macroId";
        const PLAYBACK_FIELD: &str = "playback";
        const TOTAL_RUNS_FIELD: &str = "totalRuns";
        const UNKNOWN_FIELD: &str = "events";
        const NAME_FIELD: &str = "name";
        let json = serde_json::to_value(draft()).expect("Known property draft serializes");
        assert_eq!(json[MACRO_ID_FIELD], ID);
        assert_eq!(json[PLAYBACK_FIELD][TOTAL_RUNS_FIELD], FIRST_RUN);
        let mut failure = ConfigureResult::failure();
        failure
            .fields
            .insert(NAME_FIELD.into(), CALLBACK_FAILED.into());
        assert!(failure.validate().is_ok());
        failure
            .fields
            .insert(UNKNOWN_FIELD.into(), CALLBACK_FAILED.into());
        assert!(failure.validate().is_err());
        let empty = ConfigureResult {
            ok: false,
            message: String::new(),
            fields: HashMap::new(),
        };
        assert!(empty.validate().is_err());
    }
}
