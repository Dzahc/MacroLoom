//! Saved mouse-stream behavior through capture and versioned document validation, without desktop injection.
use super::*;
use test_support::{environment, TEST_X, TEST_Y};

const FIRST_MOVE: Duration = Duration::from_millis(1);
const FINAL_MOVE: Duration = Duration::from_millis(2);
const STOP_TIME: Duration = Duration::from_millis(20);
const FINAL_X: i32 = TEST_X + 15;
const FINAL_Y: i32 = TEST_Y + 10;
const STOP_POSITION: (i32, i32) = (900, 800);

/// Returns a positioned button transition in the saved stream's signed physical coordinate space.
fn button(button: MouseButton, down: bool, x: i32, y: i32) -> EventKind {
    EventKind::Mouse { button, down, x, y }
}

/// Freezes and decodes a capture through the actual saved-format validator, including end-held input.
fn saved(capture: &mut Capture) -> MacroDocument {
    capture.finish(STOP_TIME, STOP_POSITION);
    let mut recording = Recording::new(environment()).expect("Recording metadata");
    recording.freeze(capture);
    let bytes = serde_json::to_vec(&recording.document).expect("Serialize capture");
    macro_format::decode(&bytes).expect("Valid saved mouse stream")
}

/// Requires an observed movement at the expected signed physical pixel; wrong event kinds fail the behavior assertion.
fn assert_move(event: &RecordedEvent, position: (i32, i32)) {
    let EventData::MouseMove { x, y } = event.data else {
        panic!("Expected retained movement");
    };
    assert_eq!((x, y), position);
}

/// Requires an exact vertical wheel amount and position after the production serialization/validation round trip.
fn assert_wheel(event: &RecordedEvent, amount: i32, position: (i32, i32)) {
    let EventData::MouseWheel {
        axis: macro_format::WheelAxis::Vertical,
        delta,
        x,
        y,
    } = event.data
    else {
        panic!("Expected positioned vertical wheel");
    };
    assert_eq!(delta, amount);
    assert_eq!((x, y), position);
}

#[test]
/// Stop retains both the first and final dense drag observations without warping to Stop or inventing a release.
fn stopping_a_dense_drag_preserves_its_first_and_final_points() {
    let mut capture = Capture::default();
    capture.append(
        Duration::ZERO,
        button(MouseButton::Left, true, TEST_X, TEST_Y),
    );
    capture.append(
        FIRST_MOVE,
        EventKind::Move {
            x: TEST_X,
            y: FINAL_Y,
        },
    );
    capture.append(
        FINAL_MOVE,
        EventKind::Move {
            x: FINAL_X,
            y: FINAL_Y,
        },
    );
    let document = saved(&mut capture);
    assert_eq!(
        document
            .events
            .iter()
            .map(|event| event.at_ms)
            .collect::<Vec<_>>(),
        vec![
            milliseconds(Duration::ZERO),
            milliseconds(FIRST_MOVE),
            milliseconds(FINAL_MOVE)
        ]
    );
    assert_move(
        document.events.last().expect("Final drag point"),
        (FINAL_X, FINAL_Y),
    );
    assert_eq!(document.duration_ms, milliseconds(STOP_TIME));
}

#[test]
/// A budgeted pending endpoint can be replaced at the event limit, but a following transition cannot exceed it.
fn replacing_a_pending_endpoint_at_the_event_limit_keeps_a_valid_prefix() {
    const DRAG_EVENTS: usize = 3;
    const LATER_X: i32 = FINAL_X + 1;
    let mut capture = Capture::default();
    let mut recording = Recording::new(environment()).expect("Recording metadata");
    for _ in DRAG_EVENTS..macro_format::MAX_EVENTS {
        let kind = test_support::key(true);
        assert!(recording.accept_capture(&capture, Duration::ZERO, &kind));
        capture.append(Duration::ZERO, kind);
    }
    for (at, kind) in [
        (
            Duration::ZERO,
            button(MouseButton::Left, true, TEST_X, TEST_Y),
        ),
        (
            FIRST_MOVE,
            EventKind::Move {
                x: TEST_X,
                y: TEST_Y,
            },
        ),
        (
            FINAL_MOVE,
            EventKind::Move {
                x: FINAL_X,
                y: FINAL_Y,
            },
        ),
        (
            FINAL_MOVE,
            EventKind::Move {
                x: LATER_X,
                y: FINAL_Y,
            },
        ),
    ] {
        assert!(recording.accept_capture(&capture, at, &kind));
        capture.append(at, kind);
    }
    assert!(!recording.accept_capture(&capture, STOP_TIME, &test_support::key(false)));
    let document = saved(&mut capture);
    assert_eq!(document.events.len(), macro_format::MAX_EVENTS);
    assert_move(
        document.events.last().expect("Budgeted endpoint"),
        (LATER_X, FINAL_Y),
    );
}

#[test]
/// Wheel amounts remain signed and unnormalized, with pending drag points ordered before equal-time wheel/key transitions.
fn positioned_wheel_amounts_and_transition_order_survive_serialization() {
    const SMALL_DELTA: i32 = 15;
    const LARGE_NEGATIVE_DELTA: i32 = -240;
    let mut capture = Capture::default();
    capture.append(
        Duration::ZERO,
        button(MouseButton::Right, true, TEST_X, TEST_Y),
    );
    capture.append(
        FIRST_MOVE,
        EventKind::Move {
            x: TEST_X,
            y: TEST_Y,
        },
    );
    capture.append(
        FINAL_MOVE,
        EventKind::Move {
            x: FINAL_X,
            y: FINAL_Y,
        },
    );
    capture.append(
        FINAL_MOVE,
        EventKind::Wheel {
            delta: SMALL_DELTA,
            x: FINAL_X,
            y: FINAL_Y,
        },
    );
    capture.append(FINAL_MOVE, test_support::key(true));
    capture.append(
        FINAL_MOVE,
        EventKind::Wheel {
            delta: LARGE_NEGATIVE_DELTA,
            x: TEST_X,
            y: TEST_Y,
        },
    );
    let document = saved(&mut capture);
    let [press, first, final_point, small, key, large] = document.events.as_slice() else {
        panic!("Expected two ordered wheel transitions after retained drag points");
    };
    let EventData::MouseDown {
        button: Button::Right,
        ..
    } = press.data
    else {
        panic!("Expected original right press");
    };
    assert_move(first, (TEST_X, TEST_Y));
    assert_move(final_point, (FINAL_X, FINAL_Y));
    assert_wheel(small, SMALL_DELTA, (FINAL_X, FINAL_Y));
    assert_wheel(large, LARGE_NEGATIVE_DELTA, (TEST_X, TEST_Y));
    let EventData::KeyDown { .. } = key.data else {
        panic!("Key transition must remain between wheel events");
    };
    assert_eq!(
        document
            .events
            .iter()
            .map(|event| event.at_ms)
            .collect::<Vec<_>>(),
        vec![
            milliseconds(Duration::ZERO),
            milliseconds(FIRST_MOVE),
            milliseconds(FINAL_MOVE),
            milliseconds(FINAL_MOVE),
            milliseconds(FINAL_MOVE),
            milliseconds(FINAL_MOVE)
        ]
    );
}

#[test]
/// A delayed next movement cannot erase the position occupied during a pause, even when that position was pending.
fn a_paused_drag_preserves_the_endpoint_before_the_pause() {
    const AFTER_PAUSE: Duration = Duration::from_millis(19);
    let mut capture = Capture::default();
    capture.append(
        Duration::ZERO,
        button(MouseButton::Left, true, TEST_X, TEST_Y),
    );
    capture.append(
        FIRST_MOVE,
        EventKind::Move {
            x: TEST_X,
            y: TEST_Y,
        },
    );
    capture.append(
        FINAL_MOVE,
        EventKind::Move {
            x: FINAL_X,
            y: FINAL_Y,
        },
    );
    capture.append(
        AFTER_PAUSE,
        EventKind::Move {
            x: TEST_X,
            y: FINAL_Y,
        },
    );
    let document = saved(&mut capture);
    assert_eq!(
        document
            .events
            .iter()
            .map(|event| event.at_ms)
            .collect::<Vec<_>>(),
        vec![
            milliseconds(Duration::ZERO),
            milliseconds(FIRST_MOVE),
            milliseconds(FINAL_MOVE),
            milliseconds(AFTER_PAUSE)
        ]
    );
}

#[test]
/// Dense continuous motion retains sample-spaced positions plus the first/final endpoints at original offsets.
fn dense_motion_is_bounded_by_the_source_sample_interval() {
    const SAMPLE_WINDOWS: u64 = 2;
    let sample_ms = milliseconds(crate::input::MOVE_SAMPLE);
    let final_ms = sample_ms * SAMPLE_WINDOWS + milliseconds(FINAL_MOVE);
    let mut capture = Capture::default();
    capture.append(
        Duration::ZERO,
        button(MouseButton::Left, true, TEST_X, TEST_Y),
    );
    for offset in milliseconds(FIRST_MOVE)..=final_ms {
        capture.append(
            Duration::from_millis(offset),
            EventKind::Move {
                x: TEST_X + offset as i32,
                y: TEST_Y,
            },
        );
    }
    let document = saved(&mut capture);
    let first_ms = milliseconds(FIRST_MOVE);
    assert_eq!(
        document
            .events
            .iter()
            .map(|event| event.at_ms)
            .collect::<Vec<_>>(),
        vec![
            milliseconds(Duration::ZERO),
            first_ms,
            first_ms + sample_ms,
            first_ms + sample_ms * SAMPLE_WINDOWS,
            final_ms
        ]
    );
}

#[test]
/// A modifier flushes pending motion and releasing one button cannot end the other button's drag or change release pixels.
fn modifier_boundaries_and_independent_buttons_preserve_curved_drag_order() {
    const NEXT_MOVE: Duration = Duration::from_millis(3);
    const RELEASE_X: i32 = FINAL_X + 100;
    const RELEASE_Y: i32 = FINAL_Y - 50;
    let mut capture = Capture::default();
    for (at, kind) in [
        (
            Duration::ZERO,
            button(MouseButton::Left, true, TEST_X, TEST_Y),
        ),
        (
            FIRST_MOVE,
            EventKind::Move {
                x: TEST_X,
                y: FINAL_Y,
            },
        ),
        (
            FINAL_MOVE,
            EventKind::Move {
                x: FINAL_X,
                y: FINAL_Y,
            },
        ),
        (FINAL_MOVE, test_support::key(true)),
        (
            NEXT_MOVE,
            EventKind::Move {
                x: FINAL_X,
                y: TEST_Y,
            },
        ),
        (NEXT_MOVE, button(MouseButton::Right, true, FINAL_X, TEST_Y)),
        (
            NEXT_MOVE,
            button(MouseButton::Left, false, RELEASE_X, RELEASE_Y),
        ),
        (
            NEXT_MOVE,
            EventKind::Move {
                x: TEST_X,
                y: TEST_Y,
            },
        ),
        (NEXT_MOVE, test_support::key(false)),
        (
            NEXT_MOVE,
            button(MouseButton::Right, false, FINAL_X, FINAL_Y),
        ),
        (
            NEXT_MOVE,
            EventKind::Move {
                x: RELEASE_X,
                y: RELEASE_Y,
            },
        ),
    ] {
        capture.append(at, kind);
    }
    let document = saved(&mut capture);
    const EXPECTED_EVENTS: usize = 10;
    const MODIFIER_INDEX: usize = 3;
    const LEFT_RELEASE_INDEX: usize = 6;
    const RIGHT_MOVE_INDEX: usize = 7;
    assert_eq!(document.events.len(), EXPECTED_EVENTS);
    let EventData::KeyDown { .. } = document.events[MODIFIER_INDEX].data else {
        panic!("Expected modifier after pending drag point");
    };
    let EventData::MouseUp {
        button: Button::Left,
        x,
        y,
    } = document.events[LEFT_RELEASE_INDEX].data
    else {
        panic!("Expected independent left release");
    };
    assert_eq!((x, y), (RELEASE_X, RELEASE_Y));
    assert_move(&document.events[RIGHT_MOVE_INDEX], (TEST_X, TEST_Y));
    let EventData::MouseUp {
        button: Button::Right,
        x,
        y,
    } = document.events.last().expect("Right release").data
    else {
        panic!("Expected final right release");
    };
    assert_eq!((x, y), (FINAL_X, FINAL_Y));
}

#[test]
/// Replacing a same-size tail refunds its old byte charge; an oversized replacement leaves the prior budget and endpoint intact.
fn replacement_bytes_are_refunded_and_rejected_growth_preserves_the_tail() {
    let mut recording = Recording::new(environment()).expect("Recording metadata");
    let mut capture = Capture::default();
    for (at, kind) in [
        (
            Duration::ZERO,
            button(MouseButton::Left, true, TEST_X, TEST_Y),
        ),
        (
            FIRST_MOVE,
            EventKind::Move {
                x: TEST_X,
                y: TEST_Y,
            },
        ),
        (
            FINAL_MOVE,
            EventKind::Move {
                x: FINAL_X,
                y: FINAL_Y,
            },
        ),
    ] {
        assert!(recording.accept_capture(&capture, at, &kind));
        capture.append(at, kind);
    }
    let charged = recording.bytes;
    let same = EventKind::Move {
        x: FINAL_X,
        y: FINAL_Y,
    };
    assert!(recording.accept_capture(&capture, FINAL_MOVE, &same));
    capture.append(FINAL_MOVE, same);
    assert_eq!(recording.bytes, charged);
    recording.bytes = macro_format::MAX_FILE_BYTES as usize;
    let larger = EventKind::Move {
        x: i32::MIN,
        y: i32::MAX,
    };
    assert!(!recording.accept_capture(&capture, FINAL_MOVE, &larger));
    assert_eq!(recording.bytes as u64, macro_format::MAX_FILE_BYTES);
    let document = saved(&mut capture);
    assert_move(
        document.events.last().expect("Previous valid endpoint"),
        (FINAL_X, FINAL_Y),
    );
}
