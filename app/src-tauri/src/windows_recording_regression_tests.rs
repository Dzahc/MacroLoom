//! Native recording-boundary/filter regressions using controlled focus and held-state observations, without desktop injection.
use super::*;
use crate::input::Capture;
use crate::macro_format::EventData;
use crate::recording::{test_support, Recording};

const FIRST_MODIFIER: usize = 0;
const CONTROL_KEY: u16 = SNAPSHOT_MODIFIERS[FIRST_MODIFIER];
const CONTROL_SCAN: u16 = 0x1d;
const RELEASE_OFFSET: Duration = Duration::from_millis(1);
const TYPING_OFFSET: Duration = Duration::from_millis(2);
const EXPECTED_EVENTS: usize = 4;
const RELEASE_INDEX: usize = 1;
const EXPECTED_RELEASE: &str = "Ctrl release must precede subsequent typing";
const ENTER_KEY: u16 = SNAPSHOT_CONTROL_KEYS[2];
const SPACE_KEY: u16 = SNAPSHOT_CONTROL_KEYS[3];
const ACTIVATION_KEYS: [u16; 2] = [ENTER_KEY, SPACE_KEY];
const LATER_CONTROL_KEY: u16 = test_support::TEST_KEY;

#[test]
/// Activation gestures retain their MacroLoom origin when asynchronous startup snapshots after focus leaves the app.
fn regression_record_activation_stays_excluded_after_startup_focus_change() {
    for activation in ACTIVATION_KEYS {
        let mut keys = OwnKeySequences::default();
        assert!(keys.key(activation, true, true));
        assert!(keys.key(CONTROL_KEY, true, true));
        let initial = keys.initial_snapshot(
            false,
            POINT {
                x: test_support::TEST_X,
                y: test_support::TEST_Y,
            },
            false,
            // Enter/Space remains held from the Record control while Ctrl is valid initial recording data.
            |key| [CONTROL_KEY, activation].contains(&key),
        );
        let captured: Vec<u16> = initial
            .iter()
            .filter_map(
                // Captures the real snapshot identities before subsequent repeats/releases can affect the filter.
                |input| match input {
                    EventKind::Key { vk, .. } => Some(*vk),
                    _ => None,
                },
            )
            .collect();
        assert_eq!(
            captured,
            vec![CONTROL_KEY],
            "Record activation entered the initial snapshot after focus changed"
        );
        assert!(
            keys.key(activation, true, false),
            "Record activation repeat escaped exclusion"
        );
        assert!(
            keys.key(activation, false, false),
            "Record activation release escaped exclusion"
        );
        assert!(
            !keys.key(CONTROL_KEY, false, false),
            "Valid initial modifier lost its release"
        );
        assert!(
            !keys.key(activation, true, false),
            "A new external activation-key press must be recordable"
        );
        assert!(!keys.key(activation, false, false));
    }
}

#[test]
/// Enter/Space held in the target application remains ordinary initial data when it did not originate in a control.
fn external_held_activation_keys_remain_recordable() {
    for activation in ACTIVATION_KEYS {
        let mut keys = OwnKeySequences::default();
        assert!(!keys.key(activation, true, false));
        let initial = keys.initial_snapshot(
            false,
            POINT {
                x: test_support::TEST_X,
                y: test_support::TEST_Y,
            },
            false,
            // A real target-app Enter/Space hold has no MacroLoom control history.
            |key| key == activation,
        );
        let captured: Vec<u16> = initial
            .iter()
            .filter_map(
                // Reads native key identities from the same boundary used by startup.
                |input| match input {
                    EventKind::Key { vk, .. } => Some(*vk),
                    _ => None,
                },
            )
            .collect();
        assert_eq!(captured, vec![activation]);
        assert!(!keys.key(activation, true, false));
        assert!(!keys.key(activation, false, false));
    }
}

#[test]
/// A later recording boundary replaces initial key ownership instead of exempting gestures from an older snapshot.
fn newer_snapshot_replaces_initial_key_ownership() {
    let mut keys = OwnKeySequences::default();
    keys.initial_snapshot(
        true,
        POINT {
            x: test_support::TEST_X,
            y: test_support::TEST_Y,
        },
        false,
        // First recording begins with Ctrl held.
        |key| key == CONTROL_KEY,
    );
    keys.initial_snapshot(
        true,
        POINT {
            x: test_support::TEST_X,
            y: test_support::TEST_Y,
        },
        false,
        // The next recording observes no held keys, regardless of earlier filter ownership.
        |_| false,
    );
    assert!(keys.key(CONTROL_KEY, true, true));
    assert!(keys.key(CONTROL_KEY, false, false));
}

#[test]
/// A snapshotted modifier remains capture-owned through repeats before focus leaves MacroLoom.
fn regression_initially_held_modifier_repeats_cannot_restore_exclusion() {
    let mut keys = OwnKeySequences::default();
    assert!(keys.key(CONTROL_KEY, true, true));
    keys.initial_snapshot(
        true,
        POINT {
            x: test_support::TEST_X,
            y: test_support::TEST_Y,
        },
        false,
        // Only the held modifier belongs to the recording's initial snapshot.
        |key| key == CONTROL_KEY,
    );
    assert!(
        !keys.key(CONTROL_KEY, true, true),
        "Initially recorded repeat was reclassified as a control"
    );
    assert!(!keys.key(CONTROL_KEY, false, false));
}

#[test]
/// Snapshot handoff leaves activation gestures excluded and does not exempt later control-origin presses.
fn snapshot_preserves_activation_exclusion_and_later_control_gestures() {
    let mut keys = OwnKeySequences::default();
    assert!(keys.key(ENTER_KEY, true, true));
    assert!(keys.key(CONTROL_KEY, true, true));
    keys.initial_snapshot(
        true,
        POINT {
            x: test_support::TEST_X,
            y: test_support::TEST_Y,
        },
        false,
        // Enter activates Record and must remain excluded while Ctrl is admitted as initial data.
        |key| [CONTROL_KEY, ENTER_KEY].contains(&key),
    );
    assert!(keys.key(ENTER_KEY, false, false));
    assert!(keys.key(LATER_CONTROL_KEY, true, true));
    assert!(keys.key(LATER_CONTROL_KEY, false, false));
    keys.key(CONTROL_KEY, false, false);
    assert!(keys.key(CONTROL_KEY, true, true));
    assert!(keys.key(CONTROL_KEY, false, false));
}

#[test]
/// Holding Ctrl in MacroLoom, starting capture, then releasing externally must save Ctrl-up before the next key.
fn regression_initially_held_modifier_keeps_its_real_release() {
    let mut keys = OwnKeySequences::default();
    assert!(keys.key(CONTROL_KEY, true, true));
    let initial = keys.initial_snapshot(
        true,
        POINT {
            x: test_support::TEST_X,
            y: test_support::TEST_Y,
        },
        false,
        // Simulates Ctrl held at the real start boundary, without reading physical key state.
        |key| key == CONTROL_KEY,
    );
    let mut capture = Capture::default();
    for event in initial {
        capture.append(Duration::ZERO, event);
    }
    if !keys.key(CONTROL_KEY, false, false) {
        capture.append(
            RELEASE_OFFSET,
            EventKind::Key {
                vk: CONTROL_KEY,
                scan: CONTROL_SCAN,
                extended: false,
                down: false,
            },
        );
    }
    for down in [true, false] {
        if !keys.key(test_support::TEST_KEY, down, false) {
            capture.append(TYPING_OFFSET, test_support::key(down));
        }
    }
    capture.finish(TYPING_OFFSET, (test_support::TEST_X, test_support::TEST_Y));
    let mut recording = Recording::new(test_support::environment()).expect("Recording metadata");
    recording.freeze(&capture);
    assert_eq!(
        recording.document.events.len(),
        EXPECTED_EVENTS,
        "Saved recording lost the initially held Ctrl release"
    );
    let EventData::KeyUp { native, .. } = &recording.document.events[RELEASE_INDEX].data else {
        panic!("{EXPECTED_RELEASE}");
    };
    assert_eq!(native.virtual_key, CONTROL_KEY);
}
