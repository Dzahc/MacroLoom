//! Typed Configure boundary and submission state, independent of native window dispatch.

use crate::macro_format::ValidationError;
use crate::macro_format::{self, MacroDocument, PlaybackProperties, RepeatMode, MAX_SAFE_INTEGER};
use crate::repository::LibraryState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CONFIGURE_BUSY: &str = "Close Configure before starting another operation";
pub const CONFIGURE_MISSING: &str = "Configure is no longer available";
pub const CONFIGURE_PENDING: &str = "Wait for Save to finish";
pub const CALLBACK_FAILED: &str =
    "Could not save changes. Your edits are retained; try again or Cancel.";
const WRONG_TARGET: &str = "Configure draft does not match its original macro";
const INVALID_RESULT: &str = "Configure properties cannot include extension metadata";
const ATTEMPT_INCREMENT: u64 = 1;
const MACRO_ID_FIELD: &str = "macroId";
const CONFIGURE_FIELD: &str = "configure";
const PROPERTY_ERROR_FIELDS: [(&str, &str); 5] = [
    (macro_format::fields::NAME, "name"),
    ("playback.speed", "speed"),
    ("playback.repeatMode", "repeatMode"),
    ("playback.totalRuns", "totalRuns"),
    ("playback.intervalMs", "interval"),
];
const REPEAT_MODE_FIELD: &str = "playback.repeatMode";
const TOTAL_RUNS_FIELD: &str = "playback.totalRuns";
const INTERVAL_FIELD: &str = "playback.intervalMs";
const ONCE_MODE: &str = "once";
const FIXED_MODE: &str = "fixed";
const INDEFINITE_MODE: &str = "indefinite";
const INVALID_MODE: &str = "Choose Once, Fixed count, or Indefinitely";
const INVALID_INTEGER: &str = "Must be a whole number in the supported safe integer range";
const RETAINED_GUIDANCE: &str =
    "Your edits are retained. Correct the problem and Retry, or Cancel.";
const NONNEGATIVE_MINIMUM: f64 = 0.0;
const POSITIVE_MINIMUM: f64 = 1.0;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// External editable playback inputs retain invalid enum/numeric values long enough to report the exact field.
pub struct ConfigurePlaybackInput {
    speed: f64,
    repeat_mode: String,
    total_runs: f64,
    interval_ms: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Typed command payload; fractional/negative counts and unknown repeat modes are validated before conversion to storage types.
pub struct ConfigureDraftInput {
    macro_id: String,
    name: String,
    playback: ConfigurePlaybackInput,
}
impl ConfigureDraftInput {
    /// Converts externally supplied values only after field-specific validation and verifies the editor's frozen identity.
    pub fn validate(self, original: &ConfigureDraft) -> Result<ConfigureDraft, ValidationError> {
        let repeat_mode = match self.playback.repeat_mode.as_str() {
            ONCE_MODE => RepeatMode::Once,
            FIXED_MODE => RepeatMode::Fixed,
            INDEFINITE_MODE => RepeatMode::Indefinite,
            _ => return Err(ValidationError::new(REPEAT_MODE_FIELD, INVALID_MODE)),
        };
        let total_runs = safe_integer(self.playback.total_runs, true, TOTAL_RUNS_FIELD)?;
        let interval_ms = safe_integer(self.playback.interval_ms, false, INTERVAL_FIELD)?;
        let mut draft = ConfigureDraft {
            macro_id: self.macro_id,
            name: self.name,
            playback: PlaybackProperties {
                speed: self.playback.speed,
                repeat_mode,
                total_runs,
                interval_ms,
                extra: Default::default(),
            },
        };
        draft.validate(original)?;
        Ok(draft)
    }
}

/// Converts finite nonnegative integral numbers within JavaScript's safe range, requiring positive run counts.
fn safe_integer(value: f64, positive: bool, field: &str) -> Result<u64, ValidationError> {
    let minimum = if positive {
        POSITIVE_MINIMUM
    } else {
        NONNEGATIVE_MINIMUM
    };
    if !value.is_finite()
        || value.fract() != NONNEGATIVE_MINIMUM
        || value < minimum
        || value > MAX_SAFE_INTEGER as f64
    {
        return Err(ValidationError::new(field, INVALID_INTEGER));
    }
    // Finite integral values in the checked safe range convert exactly without saturation or truncation.
    Ok(value as u64)
}

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
        let mut playback = document.playback;
        // Extension metadata belongs to the file, never to the editable command payload.
        playback.extra.clear();
        Self {
            macro_id: document.id,
            name: document.name,
            playback,
        }
    }
    /// Trims and validates external drafts against the frozen identity before persistence starts.
    /// Rejects unsupported numeric presets/ranges and metadata injections into playback properties.
    pub fn validate(&mut self, original: &Self) -> Result<(), ValidationError> {
        if self.macro_id != original.macro_id {
            return Err(ValidationError::new(MACRO_ID_FIELD, WRONG_TARGET));
        }
        self.name = self.name.trim().into();
        macro_format::validate_name(&self.name)?;
        macro_format::validate_playback(&self.playback)?;
        if !self.playback.extra.is_empty() {
            return Err(ValidationError::new(
                macro_format::fields::PLAYBACK,
                INVALID_RESULT,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Backend persistence result; failure carries persistent actionable text and optional field errors.
pub struct ConfigureResult {
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub fields: HashMap<String, String>,
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub warning: String,
}

impl ConfigureResult {
    /// Constructs a retained form error for a failed attempt; never claims persistence success.
    pub fn failure() -> Self {
        Self {
            ok: false,
            message: CALLBACK_FAILED.into(),
            fields: HashMap::new(),
            changed: false,
            name: String::new(),
            warning: String::new(),
        }
    }
    /// Converts a precommit field/storage diagnostic to retained editor feedback using rendered field keys.
    pub fn rejected(error: ValidationError) -> Self {
        let field = PROPERTY_ERROR_FIELDS
            .iter()
            .find(
                // Match backend JSON paths to the editor's rendered control keys.
                |(path, _)| *path == error.field,
            )
            .map(
                // Return only the rendered control key, retaining full text in the persistent message.
                |(_, field)| *field,
            );
        let mut result = Self::failure();
        result.message = format!("{}. {RETAINED_GUIDANCE}", error.message);
        if let Some(field) = field {
            result.fields.insert(field.into(), error.message);
        }
        result
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Backend-owned completed attempt survives IPC delivery failure and editor dismissal until the next opening.
pub struct ConfigureCompletion {
    pub attempt_id: u64,
    pub result: ConfigureResult,
    pub library: Option<LibraryState>,
}

#[derive(Clone, Serialize)]
/// Read-only recovery snapshot; querying it never writes, changes timestamps or claims another attempt.
pub struct ConfigureStatus {
    pub pending: bool,
    pub completion: Option<ConfigureCompletion>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Backend-owned attempt identity used to match one validated draft to its worker completion.
pub struct ConfigureSubmission {
    pub attempt_id: u64,
}

/// Worker reservation retains one claimable draft until its matching completion is stored.
pub struct PendingSave {
    pub attempt_id: u64,
    pub draft: Option<ConfigureDraft>,
}

#[derive(Default)]
/// Singleton editor reservation, local snapshot, worker guard and deferred application-exit intent.
pub struct ConfigureState {
    pub occupied: bool,
    pub operation: bool,
    pub original: Option<ConfigureDraft>,
    pub pending: Option<PendingSave>,
    pub next_attempt: u64,
    pub exit_requested: bool,
    pub save_succeeded: bool,
    pub completion: Option<ConfigureCompletion>,
}

impl ConfigureState {
    /// Reserves an editor before asynchronous disk reads; concurrent operation/open requests fail without changing state.
    pub fn reserve(&mut self) -> Result<(), String> {
        if self.occupied || self.operation {
            return Err(CONFIGURE_BUSY.into());
        }
        self.occupied = true;
        self.completion = None;
        self.save_succeeded = false;
        Ok(())
    }
    /// Creates one submission guard after native validation; reentrant or repeated Save cannot replace active worker data.
    pub fn begin(
        &mut self,
        mut draft: ConfigureDraft,
    ) -> Result<ConfigureSubmission, ValidationError> {
        if self.pending.is_some() {
            return Err(ValidationError::new(CONFIGURE_FIELD, CONFIGURE_PENDING));
        }
        if self.save_succeeded {
            return Err(ValidationError::new(CONFIGURE_FIELD, CONFIGURE_MISSING));
        }
        let original = self
            .original
            .as_ref()
            .ok_or_else(|| ValidationError::new(CONFIGURE_FIELD, CONFIGURE_MISSING))?;
        draft.validate(original)?;
        if self.next_attempt >= MAX_SAFE_INTEGER {
            return Err(ValidationError::new(CONFIGURE_FIELD, CONFIGURE_MISSING));
        }
        self.next_attempt += ATTEMPT_INCREMENT;
        self.pending = Some(PendingSave {
            attempt_id: self.next_attempt,
            draft: Some(draft),
        });
        self.save_succeeded = false;
        self.completion = None;
        Ok(ConfigureSubmission {
            attempt_id: self.next_attempt,
        })
    }
    /// Returns the matching native-validated draft once; missing, forged, stale and replayed attempt IDs cannot deliver data.
    pub fn claim(&mut self, attempt_id: u64) -> Result<ConfigureDraft, String> {
        let pending = self.pending.as_mut().ok_or(CONFIGURE_MISSING)?;
        if pending.attempt_id != attempt_id {
            return Err(CONFIGURE_MISSING.into());
        }
        pending.draft.take().ok_or_else(|| CONFIGURE_MISSING.into())
    }
    /// Stores only a matching claimed worker result; stale/replayed completions cannot release another pending Save.
    /// Returns the authoritative result for IPC after retaining its receipt, independently of frontend acknowledgement.
    pub fn complete(&mut self, completion: ConfigureCompletion) -> Result<ConfigureResult, String> {
        let pending = self.pending.as_ref().ok_or(CONFIGURE_MISSING)?;
        if pending.attempt_id != completion.attempt_id || pending.draft.is_some() {
            return Err(CONFIGURE_MISSING.into());
        }
        let result = completion.result.clone();
        self.settle(&result);
        self.completion = Some(completion);
        Ok(result)
    }
    /// Ends worker ownership; failed Save cancels deferred exit so recoverable edits remain open.
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
    const NORMAL_SPEED_INDEX: usize = 2;
    const NO_INTERVAL_MS: u64 = 0;
    const INVALID_TOTAL_RUNS: u64 = 0;
    const INTERVAL_ERROR_KEY: &str = "interval";
    const INVALID_FRACTION: f64 = 1.5;
    const INVALID_NEGATIVE: f64 = -1.0;
    const UNKNOWN_MODE: &str = "unknown";
    const JSON_REPEAT_MODE: &str = "repeatMode";
    const JSON_TOTAL_RUNS: &str = "totalRuns";
    const JSON_INTERVAL: &str = "intervalMs";

    #[test]
    /// Direct command callers receive rendered field diagnostics for unknown modes and fractional/negative numeric inputs.
    fn malformed_domain_inputs_have_field_specific_feedback() {
        let original = draft();
        let mut json = serde_json::to_value(draft()).unwrap();
        json[macro_format::fields::PLAYBACK][JSON_REPEAT_MODE] = UNKNOWN_MODE.into();
        let input: ConfigureDraftInput = serde_json::from_value(json).unwrap();
        let error = input.validate(&original).err().unwrap();
        assert!(ConfigureResult::rejected(error)
            .fields
            .contains_key(JSON_REPEAT_MODE));
        let mut json = serde_json::to_value(draft()).unwrap();
        json[macro_format::fields::PLAYBACK][JSON_TOTAL_RUNS] = INVALID_FRACTION.into();
        let input: ConfigureDraftInput = serde_json::from_value(json).unwrap();
        let error = input.validate(&original).err().unwrap();
        assert!(ConfigureResult::rejected(error)
            .fields
            .contains_key(JSON_TOTAL_RUNS));
        let mut json = serde_json::to_value(draft()).unwrap();
        json[macro_format::fields::PLAYBACK][JSON_INTERVAL] = INVALID_NEGATIVE.into();
        let input: ConfigureDraftInput = serde_json::from_value(json).unwrap();
        let error = input.validate(&original).err().unwrap();
        assert!(ConfigureResult::rejected(error)
            .fields
            .contains_key(INTERVAL_ERROR_KEY));
    }

    /// Returns a complete minimal property snapshot matching the existing storage contract.
    fn draft() -> ConfigureDraft {
        ConfigureDraft {
            macro_id: ID.into(),
            name: NAME.into(),
            playback: PlaybackProperties {
                speed: SPEEDS[NORMAL_SPEED_INDEX],
                repeat_mode: RepeatMode::Once,
                total_runs: FIRST_RUN,
                interval_ms: NO_INTERVAL_MS,
                extra: Default::default(),
            },
        }
    }
    #[test]
    /// Native validation trims names, counts emoji as scalar values, and refuses identity/range changes before persistence.
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
        value.playback.total_runs = INVALID_TOTAL_RUNS;
        assert!(value.validate(&original).is_err());
        value = draft();
        value.playback.interval_ms = MAX_SAFE_INTEGER + FIRST_RUN;
        assert!(value.validate(&original).is_err());
    }
    #[test]
    /// Submission and operation guards hold throughout a worker; failed completion unlocks Retry while clearing queued exit.
    fn failure_retains_snapshot_and_allows_a_new_attempt() {
        let mut state = ConfigureState::default();
        assert!(state.reserve().is_ok());
        assert!(state.require_operation().is_err());
        state.original = Some(draft());
        let first = state
            .begin(draft())
            .expect("Valid reserved editor accepts its first draft");
        assert!(state.begin(draft()).is_err());
        state.exit_requested = true;
        state.settle(&ConfigureResult::failure());
        assert!(!state.exit_requested);
        assert!(state.original.is_some());
        let retry = state
            .begin(draft())
            .expect("Failure releases only the submission guard");
        assert!(retry.attempt_id > first.attempt_id);
    }

    #[test]
    /// Successful completion preserves queued exit and prevents another submission from the completed editor.
    fn success_preserves_exit_and_finishes_the_attempt() {
        let mut state = ConfigureState {
            occupied: true,
            original: Some(draft()),
            exit_requested: true,
            ..ConfigureState::default()
        };
        state
            .begin(draft())
            .expect("Reserved editor accepts its valid properties");
        let result = ConfigureResult {
            ok: true,
            message: String::new(),
            fields: HashMap::new(),
            changed: false,
            name: String::new(),
            warning: String::new(),
        };
        state.settle(&result);
        assert!(state.exit_requested);
        assert!(state.pending.is_none());
        assert!(state.begin(draft()).is_err());
    }

    #[test]
    /// A forged claim cannot take data; the active claim returns the validated draft once and replay leaves its worker pending.
    fn claims_only_the_matching_validated_draft_once() {
        let mut state = ConfigureState {
            occupied: true,
            original: Some(draft()),
            ..ConfigureState::default()
        };
        assert!(state.claim(FIRST_RUN).is_err());
        let mut submitted = draft();
        submitted.name = format!("  {NAME}  ");
        let notification = state
            .begin(submitted)
            .expect("Reserved editor accepts validated draft");
        assert!(state.claim(notification.attempt_id + FIRST_RUN).is_err());
        let claimed = state
            .claim(notification.attempt_id)
            .expect("Only the matching notification claims the pending draft");
        assert_eq!(claimed.macro_id, ID);
        assert_eq!(claimed.name, NAME);
        assert!(state.claim(notification.attempt_id).is_err());
        assert!(state.pending.is_some());
    }

    #[test]
    /// Validation failure creates no claimable draft and Retry cannot be consumed by an earlier attempt notification.
    fn invalid_drafts_and_stale_attempts_cannot_be_claimed() {
        let mut state = ConfigureState {
            occupied: true,
            original: Some(draft()),
            ..ConfigureState::default()
        };
        let mut invalid = draft();
        invalid.macro_id = WRONG_ID.into();
        assert!(state.begin(invalid).is_err());
        assert!(state.claim(FIRST_RUN).is_err());
        let first = state
            .begin(draft())
            .expect("Valid draft creates the first attempt");
        state
            .claim(first.attempt_id)
            .expect("First worker claims the validated draft");
        state.settle(&ConfigureResult::failure());
        let retry = state.begin(draft()).expect("Failure permits a new attempt");
        assert!(state.claim(first.attempt_id).is_err());
        assert!(state.claim(retry.attempt_id).is_ok());
    }

    #[test]
    /// Only the claimed matching worker can store a receipt; stale and repeated completions leave the active guard untouched.
    fn completion_requires_a_claim_and_preserves_authoritative_receipt() {
        let mut state = ConfigureState {
            occupied: true,
            original: Some(draft()),
            ..ConfigureState::default()
        };
        let attempt = state.begin(draft()).unwrap().attempt_id;
        let completion = ConfigureCompletion {
            attempt_id: attempt,
            result: ConfigureResult::failure(),
            library: None,
        };
        assert!(state.complete(completion.clone()).is_err());
        state.claim(attempt).unwrap();
        let mut stale = completion.clone();
        stale.attempt_id += ATTEMPT_INCREMENT;
        assert!(state.complete(stale).is_err());
        assert!(state.pending.is_some());
        assert!(!state.complete(completion.clone()).unwrap().ok);
        assert!(state.pending.is_none());
        assert_eq!(state.completion.as_ref().unwrap().attempt_id, attempt);
        assert!(state.complete(completion).is_err());
        assert!(state.claim(attempt).is_err());
    }

    #[test]
    /// The draft JSON uses camelCase and retained diagnostics use the editor's rendered field keys.
    fn boundary_shapes_and_field_diagnostics_are_explicit() {
        const PLAYBACK_FIELD: &str = "playback";
        const TOTAL_RUNS_JSON_FIELD: &str = "totalRuns";
        const RESULT_FIELDS: &str = "fields";
        let json = serde_json::to_value(draft()).unwrap();
        assert_eq!(json[MACRO_ID_FIELD], ID);
        assert_eq!(json[PLAYBACK_FIELD][TOTAL_RUNS_JSON_FIELD], FIRST_RUN);
        let failure = ConfigureResult::rejected(ValidationError::new(
            macro_format::fields::NAME,
            CALLBACK_FAILED,
        ));
        let json = serde_json::to_value(failure).unwrap();
        assert_eq!(
            json[RESULT_FIELDS][macro_format::fields::NAME],
            CALLBACK_FAILED
        );
    }
}
