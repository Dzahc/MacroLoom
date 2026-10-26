//! Availability-driven global reservations and complete consumed key sequences.
use serde::Serialize;

pub const RECORD_KEY: u16 = 0x78;
pub const STOP_KEY: u16 = 0x77;
pub const PLAY_KEY: u16 = 0x79;
pub const RECORD_ID: i32 = 1;
pub const STOP_ID: i32 = 2;
pub const PLAY_ID: i32 = 3;
pub const STOP_UNAVAILABLE: &str =
    "F8 Stop is unavailable. Close the application using it, then Retry shortcuts.";
pub const RECORD_UNAVAILABLE: &str =
    "F9 Record is unavailable; use the Record button or Retry shortcuts.";
pub const PLAY_UNAVAILABLE: &str =
    "F10 Play is unavailable; use the Play button or Retry shortcuts.";
const STARTS_UNAVAILABLE: &str =
    "F9 Record and F10 Play are unavailable; use toolbar buttons or Retry shortcuts.";
const RECORD_LABEL: &str = "F9 Record";
const PLAY_LABEL: &str = "F10 Play";
const BOTH_START_LABELS: &str = "F9 Record and F10 Play";

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
/// Actual OS reservation availability, never inferred from enabled UI controls.
pub struct Availability {
    pub record: bool,
    pub play: bool,
    pub stop: bool,
}

impl Availability {
    /// Explains every failed reservation, prioritizing required Stop and preserving button fallback for start-key conflicts.
    pub fn conflict(self) -> Option<String> {
        if !self.stop {
            let start = match (self.record, self.play) {
                (true, true) => return Some(STOP_UNAVAILABLE.into()),
                (false, true) => RECORD_LABEL,
                (true, false) => PLAY_LABEL,
                (false, false) => BOTH_START_LABELS,
            };
            return Some(format!("{STOP_UNAVAILABLE} {start} also unavailable."));
        }
        match (self.record, self.play) {
            (true, true) => None,
            (false, true) => Some(RECORD_UNAVAILABLE.into()),
            (true, false) => Some(PLAY_UNAVAILABLE.into()),
            (false, false) => Some(STARTS_UNAVAILABLE.into()),
        }
    }
}

#[derive(Clone, Copy)]
/// Backend transition emitted once at the corresponding boundary of a consumed physical gesture.
pub enum Control {
    Record,
    PlayPress,
    PlayRelease,
    Stop,
}

#[derive(Default)]
/// Per-key consumed sequence state survives reservation changes through the matching release.
pub struct KeySequences {
    record: KeyState,
    play: KeyState,
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
    /// Processes native input with first-down modifier ownership; only bare reserved F10 may activate Play.
    pub fn physical_key(
        &mut self,
        key: u16,
        down: bool,
        modified: bool,
        reserved: Availability,
    ) -> (bool, Option<Control>) {
        if key != PLAY_KEY {
            return self.key(key, down, reserved);
        }
        let releasing = !down && matches!(self.play, KeyState::Consumed);
        let (consumed, control) = Self::sequence(
            &mut self.play,
            down,
            reserved.play && !modified,
            Control::PlayPress,
        );
        (
            consumed,
            if releasing {
                Some(Control::PlayRelease)
            } else {
                control
            },
        )
    }

    /// Reports whether a held F10 belongs to an already consumed gesture, including after reservations change.
    pub fn play_consumed(&self) -> bool {
        matches!(self.play, KeyState::Consumed)
    }
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
    /// Becoming reserved during an already held unreserved F10 cannot consume it or start playback.
    fn unreserved_play_gesture_stays_passed_after_registration() {
        let mut keys = KeySequences::default();
        let idle = Availability {
            play: true,
            ..Availability::default()
        };
        assert!(
            !keys
                .physical_key(PLAY_KEY, true, false, Availability::default())
                .0
        );
        assert!(!keys.physical_key(PLAY_KEY, true, false, idle).0);
        assert!(!keys.physical_key(PLAY_KEY, false, false, idle).0);
        assert!(!keys.play_consumed());
    }
    #[test]
    /// Bare F10 emits one release-bound Play, while modified gestures stay passed after modifiers change.
    fn play_gestures_bind_ownership_on_first_down() {
        let mut keys = KeySequences::default();
        let idle = Availability {
            play: true,
            ..Availability::default()
        };
        let press = keys.physical_key(PLAY_KEY, true, false, idle);
        assert!(press.0);
        assert!(matches!(press.1, Some(Control::PlayPress)));
        assert!(keys.physical_key(PLAY_KEY, true, false, idle).1.is_none());
        let release = keys.physical_key(PLAY_KEY, false, false, Availability::default());
        assert!(release.0);
        assert!(matches!(release.1, Some(Control::PlayRelease)));
        assert!(keys.physical_key(PLAY_KEY, false, false, idle).1.is_none());
        assert!(!keys.physical_key(PLAY_KEY, true, true, idle).0);
        assert!(!keys.physical_key(PLAY_KEY, true, false, idle).0);
        assert!(!keys.physical_key(PLAY_KEY, false, false, idle).0);
    }
    #[test]
    /// Holding an unreserved F8 before a session starts must not turn its repeats into a new Stop gesture.
    fn reservation_mid_press_does_not_consume_existing_sequence() {
        let mut keys = KeySequences::default();
        let idle = Availability::default();
        let active = Availability {
            record: false,
            play: false,
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
            play: true,
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
