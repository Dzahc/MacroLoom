//! Native mouse adapter behavior with controlled Windows messages; tests never inject desktop input.
use super::*;
use crate::recording::test_support::{TEST_X, TEST_Y};
use windows_sys::Win32::UI::WindowsAndMessaging::{WM_MOUSEHWHEEL, WM_MOUSEWHEEL};

const PARTIAL_WHEEL: i16 = 15;
const NEGATIVE_WHEEL: i16 = -240;
const LOW_WORD_DATA: u32 = 0xabcd;

/// Constructs one callback payload at signed physical fixture pixels, without retaining any native pointer.
fn native_input(mouse_data: u32) -> MSLLHOOKSTRUCT {
    MSLLHOOKSTRUCT {
        pt: POINT {
            x: TEST_X,
            y: TEST_Y,
        },
        mouseData: mouse_data,
        flags: 0,
        time: 0,
        dwExtraInfo: 0,
    }
}

/// Freezes native adapter output and decodes its serialized document through the production validator, without disk or injection.
fn saved_native_capture(
    mut capture: crate::input::Capture,
    duration: Duration,
) -> crate::macro_format::MacroDocument {
    capture.finish(duration, (TEST_X, TEST_Y));
    let mut recording =
        crate::recording::Recording::new(crate::recording::test_support::environment())
            .expect("Recording metadata");
    recording.freeze(&capture);
    let bytes = serde_json::to_vec(&recording.document).expect("Serialize native capture");
    crate::macro_format::decode(&bytes).expect("Valid native mouse stream")
}

#[test]
/// Native wheel conversion sign-extends the high word, ignores unrelated low bits, and excludes horizontal scrolling.
fn vertical_wheel_decodes_the_exact_signed_high_word() {
    for delta in [PARTIAL_WHEEL, NEGATIVE_WHEEL, i16::MIN, i16::MAX] {
        let input = native_input((u32::from(delta as u16) << WHEEL_DATA_SHIFT) | LOW_WORD_DATA);
        let kind = mouse_kind(WM_MOUSEWHEEL, &input).expect("Vertical wheel capture");
        let EventKind::Wheel {
            delta: amount,
            x,
            y,
        } = kind
        else {
            panic!("Expected native wheel payload");
        };
        assert_eq!(amount, i32::from(delta));
        assert_eq!((x, y), (TEST_X, TEST_Y));
        assert!(mouse_kind(WM_MOUSEHWHEEL, &input).is_none());
        let injected = mouse_wheel(i32::from(delta));
        // SAFETY: mouse_wheel initializes the mi member and INPUT_MOUSE discriminator; reading that member is valid.
        let mouse = unsafe { injected.Anonymous.mi };
        assert_eq!(mouse.dwFlags, MOUSEEVENTF_WHEEL);
        assert_eq!(mouse.mouseData as i32, i32::from(delta));
        assert_eq!(mouse.dwExtraInfo, EXTRA_INFO);
    }
}

#[test]
/// External right drags retain movement/wheel/release across controls while complete control-origin left gestures stay excluded.
fn external_drag_crosses_controls_without_recording_the_stop_gesture() {
    const EXPECTED_RETAINED_EVENTS: usize = 4;
    let mut mouse = OwnMouseSequences::default();
    let mut capture = crate::input::Capture::default();
    let input = native_input(u32::from(PARTIAL_WHEEL as u16) << WHEEL_DATA_SHIFT);
    for (message, own, included) in [
        (WM_RBUTTONDOWN, false, true),
        (WM_MOUSEMOVE, true, true),
        (WM_LBUTTONDOWN, true, false),
        (WM_LBUTTONUP, false, false),
        (WM_MOUSEWHEEL, true, true),
        (WM_RBUTTONUP, true, true),
        (WM_MOUSEMOVE, false, true),
        (WM_MOUSEWHEEL, true, false),
        (WM_RBUTTONDOWN, true, false),
        (WM_RBUTTONUP, false, false),
    ] {
        let kind = mouse.capture(message, &input, own);
        assert_eq!(
            kind.is_some(),
            included,
            "Unexpected filtering for message {message}"
        );
        if let Some(kind) = kind {
            capture.append(Duration::ZERO, kind);
        }
    }
    let document = saved_native_capture(capture, Duration::ZERO);
    assert_eq!(document.events.len(), EXPECTED_RETAINED_EVENTS);
    let crate::macro_format::EventData::MouseUp {
        button: crate::macro_format::Button::Right,
        x,
        y,
    } = document.events.last().expect("Observed release").data
    else {
        panic!("Expected external right release over controls");
    };
    assert_eq!((x, y), (TEST_X, TEST_Y));
}

#[test]
/// Recording startup preserves an external hold over the toolbar while excluding a control activation held outside it.
fn held_button_snapshot_preserves_gesture_origins_and_clears_released_holds() {
    const INITIAL_BUTTONS: usize = 1;
    let mut mouse = OwnMouseSequences::default();
    let input = native_input(0);
    assert!(mouse.capture(WM_RBUTTONDOWN, &input, false).is_some());
    assert!(mouse.capture(WM_LBUTTONDOWN, &input, true).is_none());
    let initial = mouse.initial_snapshot(
        input.pt,
        true,
        // Both physical buttons remain held at startup, but only the external-origin button belongs to the stream.
        |key| [LEFT_MOUSE_KEY, RIGHT_MOUSE_KEY].contains(&key),
    );
    assert_eq!(initial.len(), INITIAL_BUTTONS);
    let EventKind::Mouse {
        button: MouseButton::Right,
        down: true,
        x,
        y,
    } = initial.first().expect("External held button")
    else {
        panic!("Expected external right hold in initial snapshot");
    };
    assert_eq!((*x, *y), (TEST_X, TEST_Y));
    assert!(mouse.capture(WM_RBUTTONUP, &input, true).is_some());
    assert!(mouse.capture(WM_LBUTTONUP, &input, false).is_none());
    mouse.capture(WM_RBUTTONDOWN, &input, false);
    assert!(mouse
        .initial_snapshot(
            input.pt,
            false,
            // A later startup sees no holds and must replace stale ownership even if a release was missed.
            |_| false
        )
        .is_empty());
    assert!(mouse.capture(WM_MOUSEWHEEL, &input, true).is_none());
    assert!(mouse.capture(WM_RBUTTONDOWN, &input, true).is_none());
}

#[test]
/// A double click remains exactly two timed ordinary button pairs, with no synthetic gesture or intervening free-motion path.
fn double_click_is_two_observed_pairs_without_extra_actions() {
    const CLICK_COUNT: usize = 4;
    const CLICK_TIMES: [Duration; CLICK_COUNT] = [
        Duration::from_millis(1),
        Duration::from_millis(2),
        Duration::from_millis(4),
        Duration::from_millis(5),
    ];
    let mut mouse = OwnMouseSequences::default();
    let input = native_input(0);
    let mut capture = crate::input::Capture::default();
    for (message, at) in [WM_LBUTTONDOWN, WM_LBUTTONUP, WM_LBUTTONDOWN, WM_LBUTTONUP]
        .into_iter()
        .zip(CLICK_TIMES)
    {
        capture.append(
            at,
            mouse
                .capture(message, &input, false)
                .expect("Ordinary click transition"),
        );
        if message == WM_LBUTTONUP {
            capture.append(
                at,
                EventKind::Move {
                    x: TEST_X,
                    y: TEST_Y,
                },
            );
        }
    }
    let document = saved_native_capture(capture, *CLICK_TIMES.last().expect("Final click time"));
    assert_eq!(document.events.len(), CLICK_COUNT);
    for (event, expected) in document.events.iter().zip(CLICK_TIMES) {
        assert_eq!(event.at_ms, crate::recording::milliseconds(expected));
    }
}
