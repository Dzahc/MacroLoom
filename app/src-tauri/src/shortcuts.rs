//! Availability-driven global reservations and complete consumed key sequences.
use serde::Serialize;

pub const RECORD_KEY: u16 = 0x78;
pub const STOP_KEY: u16 = 0x77;
pub const RECORD_ID: i32 = 1;
pub const STOP_ID: i32 = 2;
pub const STOP_UNAVAILABLE: &str =
    "F8 Stop is unavailable. Close the application using it, then Retry shortcuts.";
pub const RECORD_UNAVAILABLE: &str =
    "F9 Record is unavailable; use the Record button or Retry shortcuts.";

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
/// Actual OS reservation availability, never inferred from enabled UI controls.
pub struct Availability {
    pub record: bool,
    pub stop: bool,
}

#[derive(Clone, Copy)]
/// Backend control emitted exactly once for a consumed physical press.
pub enum Control {
    Record,
    Stop,
}

#[derive(Default)]
/// Per-key consumed sequence state survives reservation changes through the matching release.
pub struct KeySequences {
    record: KeyState,
    stop: KeyState,
}
#[derive(Default)]
/// Distinguishes an already-held unreserved press from a consumed control sequence.
enum KeyState {
    #[default]
    Up,
    Passed,
    Consumed,
}
impl KeySequences {
    /// Returns suppression and an optional first-press control; unreserved sequences remain untouched.
    pub fn key(&mut self, key: u16, down: bool, reserved: Availability) -> (bool, Option<Control>) {
        match key {
            RECORD_KEY => Self::sequence(&mut self.record, down, reserved.record, Control::Record),
            STOP_KEY => Self::sequence(&mut self.stop, down, reserved.stop, Control::Stop),
            _ => (false, None),
        }
    }
    /// Consumes repeats/releases only for a sequence whose first down was reserved.
    fn sequence(
        held: &mut KeyState,
        down: bool,
        reserved: bool,
        control: Control,
    ) -> (bool, Option<Control>) {
        let consumed = matches!(held, KeyState::Consumed);
        if !down {
            *held = KeyState::Up;
            return (consumed, None);
        }
        if !matches!(held, KeyState::Up) {
            return (consumed, None);
        }
        if reserved {
            *held = KeyState::Consumed;
            (true, Some(control))
        } else {
            *held = KeyState::Passed;
            (false, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Holding an unreserved F8 before a session starts must not turn its repeats into a new Stop gesture.
    fn reservation_mid_press_does_not_consume_existing_sequence() {
        let mut keys = KeySequences::default();
        let idle = Availability::default();
        let active = Availability {
            record: false,
            stop: true,
        };
        assert!(!keys.key(STOP_KEY, true, idle).0);
        assert!(!keys.key(STOP_KEY, true, active).0);
        assert!(!keys.key(STOP_KEY, false, active).0);
        assert!(keys.key(STOP_KEY, true, active).0);
    }
    #[test]
    /// Reserved first down emits one action; every repeat and the trailing release remain consumed after unregistration.
    fn consumed_sequence_survives_unregistration_without_repeating_control() {
        let mut keys = KeySequences::default();
        let recording = Availability {
            record: true,
            stop: true,
        };
        let first = keys.key(RECORD_KEY, true, recording);
        assert!(first.0);
        assert!(first.1.is_some());
        let repeated = keys.key(RECORD_KEY, true, recording);
        assert!(repeated.0);
        assert!(repeated.1.is_none());
        let released = keys.key(RECORD_KEY, false, Availability::default());
        assert!(released.0);
        assert!(released.1.is_none());
        assert!(!keys.key(RECORD_KEY, true, Availability::default()).0);
    }
}
