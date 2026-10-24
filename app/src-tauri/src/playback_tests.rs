//! Deterministic behavior checks for validated conversion, semantic compatibility and the production scheduler.
use super::*;
use crate::recording::{test_support, Recording};
use std::cell::{Cell, RefCell};

const FIRST: Duration = Duration::from_millis(100);
const SECOND: Duration = Duration::from_millis(350);
const LAST: Duration = Duration::from_millis(900);
const END: Duration = Duration::from_millis(1200);
const OVERHEAD: Duration = Duration::from_millis(40);
const WHEEL: i32 = -30;
const FAILURE: &str = "Injected partial action failure";
const EXTRA_FIELD: &str = "futureMetadata";
const SCALE_CHANGE: f64 = 1.0;
const EXPECTED_MICROSECONDS: [u64; 5] = [4000, 2000, 1000, 500, 250];
const SAMPLE_MILLISECOND: u64 = 1;
const REPEAT_COUNT: u64 = 3;
const INTERVAL: Duration = Duration::from_secs(2);
const DOUBLE_SPEED: f64 = 2.0;
const EXPECTED_STARTS_MS: [u64; 3] = [0, 2640, 5280];
const EXPECTED_END_MS: u64 = 5920;
const CANCEL_INTERVAL_MS: u64 = 2640;
const ZERO_RUN_LIMIT: usize = 10;

#[test]
/// Saved presets scale ties, holds and trailing duration exactly, retaining quarter-millisecond deadlines.
fn saved_speed_scales_every_deadline_without_millisecond_rounding() {
    for (speed, micros) in crate::macro_format::SPEEDS
        .into_iter()
        .zip(EXPECTED_MICROSECONDS)
    {
        let mut document = document();
        document.playback.speed = speed;
        document
            .events
            .iter_mut()
            .for_each(|event| event.at_ms = SAMPLE_MILLISECOND);
        document.duration_ms = SAMPLE_MILLISECOND;
        let capture = capture(&document);
        let expected = Duration::from_micros(micros);
        assert_eq!(capture.duration, expected);
        assert!(capture.events.iter().all(|event| event.at == expected));
    }
}

#[test]
/// Actual cleanup completion anchors each full unscaled interval; finite playback omits the final interval.
fn repeated_deadlines_preserve_scaled_timing_and_full_intervals_after_cleanup() {
    let mut document = document();
    document.playback.speed = DOUBLE_SPEED;
    let capture = capture(&document);
    let clock = TestClock::default();
    let mut starts = Vec::new();
    let mut intervals = Vec::new();
    repeat(
        &capture,
        RepeatPlan {
            total: Some(REPEAT_COUNT),
            interval: INTERVAL,
        },
        &clock,
        &AtomicBool::new(false),
        |step| {
            match step {
                Step::Run(number) => starts.push((number, clock.now())),
                Step::Event(_) | Step::Finish => clock.elapsed.set(clock.now() + OVERHEAD),
                Step::Interval(deadline) => intervals.push(deadline),
            }
            Ok(())
        },
    )
    .expect("Repeated schedule");
    let expected = EXPECTED_STARTS_MS.map(Duration::from_millis);
    assert_eq!(
        starts,
        [
            (FIRST_RUN, expected[0]),
            (FIRST_RUN + FIRST_RUN, expected[1]),
            (REPEAT_COUNT, expected[2])
        ]
    );
    assert_eq!(intervals, [expected[1], expected[2]]);
    assert_eq!(clock.now(), Duration::from_millis(EXPECTED_END_MS));
}

#[test]
/// Indefinite repetition can be stopped inside an interval without starting or dispatching another run.
fn indefinite_interval_cancellation_prevents_the_next_run() {
    let mut document = document();
    document.playback.speed = DOUBLE_SPEED;
    let clock = TestClock {
        cancel_at: Some(Duration::from_millis(CANCEL_INTERVAL_MS)),
        ..TestClock::default()
    };
    let mut runs = Vec::new();
    repeat(
        &capture(&document),
        RepeatPlan {
            total: None,
            interval: INTERVAL,
        },
        &clock,
        &AtomicBool::new(false),
        |step| {
            match step {
                Step::Run(number) => runs.push(number),
                Step::Finish => clock.elapsed.set(clock.now() + OVERHEAD),
                _ => {}
            }
            Ok(())
        },
    )
    .expect("Cancelled interval");
    assert_eq!(runs, [FIRST_RUN]);
}

#[test]
/// Zero-duration indefinite runs remain cancellable without an artificial wait or a finite-count substitution.
fn zero_duration_indefinite_runs_stop_without_adding_delays() {
    let mut capture = Capture::default();
    capture.events.push(crate::input::Event {
        at: Duration::ZERO,
        kind: test_support::key(true),
    });
    let clock = TestClock::default();
    let cancel = AtomicBool::new(false);
    let mut count = 0;
    repeat(
        &capture,
        RepeatPlan {
            total: None,
            interval: Duration::ZERO,
        },
        &clock,
        &cancel,
        |step| {
            if let Step::Event(_) = step {
                count += 1;
                if count == ZERO_RUN_LIMIT {
                    cancel.store(true, Ordering::Release);
                }
            }
            Ok(())
        },
    )
    .expect("Cancellable zero-duration playback");
    assert_eq!(count, ZERO_RUN_LIMIT);
    assert_eq!(clock.now(), Duration::ZERO);
}

#[derive(Default)]
/// Run-relative deterministic time; production scheduling remains unchanged while desktop waits are substituted.
struct TestClock {
    elapsed: Cell<Duration>,
    waits: RefCell<Vec<Duration>>,
    cancel_at: Option<Duration>,
}
impl Clock for TestClock {
    /// Returns controlled monotonic time, including dispatch and cleanup overhead.
    fn now(&self) -> Duration {
        self.elapsed.get()
    }
    /// Advances to an absolute deadline without accumulating dispatch overhead; can cancel during a chosen wait.
    fn wait(&self, at: Duration, cancel: &AtomicBool) -> Result<bool, String> {
        self.waits.borrow_mut().push(at);
        self.elapsed.set(self.elapsed.get().max(at));
        if self.cancel_at == Some(at) {
            cancel.store(true, Ordering::Release);
        }
        Ok(!cancel.load(Ordering::Acquire))
    }
}

/// Builds a saved document through the real capture/format boundary without consulting the desktop.
fn document() -> MacroDocument {
    let mut recording = Recording::new(test_support::environment()).expect("Fixture environment");
    recording.document.duration_ms = END.as_millis() as u64;
    recording.document.events = [
        (FIRST, test_support::key(true)),
        (FIRST, test_support::key(true)),
        (
            SECOND,
            EventKind::Wheel {
                delta: WHEEL,
                x: test_support::TEST_X,
                y: test_support::TEST_Y,
            },
        ),
        (LAST, test_support::key(false)),
    ]
    .iter()
    .map(|(at, kind)| crate::recording::saved_event(*at, kind))
    .collect();
    recording.document
}

#[test]
/// Repeats and equal-time ties survive conversion; one absolute schedule preserves initial/trailing waits despite dispatch work.
fn absolute_schedule_preserves_ties_and_does_not_accumulate_overhead() {
    let capture = capture(&document());
    let clock = TestClock::default();
    let mut observed = Vec::new();
    run(&capture, &clock, &AtomicBool::new(false), |kind| {
        observed.push((clock.elapsed.get(), kind.clone()));
        clock.elapsed.set(clock.elapsed.get() + OVERHEAD);
        Ok(())
    })
    .expect("Schedule succeeds");
    assert_eq!(
        observed.iter().map(|(at, _)| *at).collect::<Vec<_>>(),
        [FIRST, FIRST + OVERHEAD, SECOND, LAST]
    );
    assert_eq!(
        observed
            .iter()
            .map(|(_, kind)| kind.clone())
            .collect::<Vec<_>>(),
        capture
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(*clock.waits.borrow(), [FIRST, FIRST, SECOND, LAST, END]);
    assert_eq!(clock.elapsed.get(), END);
}

#[test]
/// Stop during an event wait prevents all later dispatch, and injection failure skips subsequent events/trailing wait.
fn cancellation_and_injection_failure_stop_the_schedule() {
    let capture = capture(&document());
    let clock = TestClock {
        cancel_at: Some(SECOND),
        ..TestClock::default()
    };
    let mut sent = Vec::new();
    run(&capture, &clock, &AtomicBool::new(false), |kind| {
        sent.push(kind.clone());
        Ok(())
    })
    .expect("Cancellation is orderly");
    assert_eq!(sent, [test_support::key(true), test_support::key(true)]);
    let clock = TestClock::default();
    let error = run(&capture, &clock, &AtomicBool::new(false), |_| {
        Err(FAILURE.into())
    })
    .expect_err("Injection must fail");
    assert_eq!(error, FAILURE);
    assert_eq!(*clock.waits.borrow(), [FIRST]);
}

#[test]
/// Extension metadata does not block replay; actual geometry, scale or layout changes do, independently of display power.
fn environment_comparison_uses_semantic_geometry_and_layout() {
    let expected = test_support::environment();
    let mut actual = expected.clone();
    actual.extra.insert(EXTRA_FIELD.into(), true.into());
    actual.displays[0]
        .extra
        .insert(EXTRA_FIELD.into(), true.into());
    assert!(compatible(&expected, &actual));
    actual.displays[0].x = test_support::TEST_X;
    assert!(!compatible(&expected, &actual));
    actual = expected.clone();
    actual.displays[0].scale_factor = test_support::TEST_SCALE + SCALE_CHANGE;
    assert!(!compatible(&expected, &actual));
    actual = expected.clone();
    actual.keyboard_layout = EXTRA_FIELD.into();
    assert!(!compatible(&expected, &actual));
}

#[test]
/// Double-click pairs, both buttons, signed partial wheel input and consecutive retained drag points round-trip unchanged.
fn saved_input_conversion_preserves_every_v1_transition_without_resampling() {
    let position = (test_support::TEST_X, test_support::TEST_Y);
    let mut stream = Vec::new();
    for button in MouseButton::ALL {
        for down in [true, false, true, false] {
            stream.push(EventKind::Mouse {
                button,
                down,
                x: position.0,
                y: position.1,
            });
        }
    }
    stream.extend([
        EventKind::Mouse {
            button: MouseButton::Left,
            down: true,
            x: position.0,
            y: position.1,
        },
        EventKind::Move {
            x: position.0,
            y: position.1,
        },
        EventKind::Move {
            x: position.0,
            y: position.1,
        },
        EventKind::Wheel {
            delta: WHEEL,
            x: position.0,
            y: position.1,
        },
        test_support::key(true),
        test_support::key(false),
    ]);
    let mut document = document();
    document.events = stream
        .iter()
        .map(|kind| crate::recording::saved_event(FIRST, kind))
        .collect();
    let replay = capture(&document);
    assert_eq!(
        replay
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        stream
    );
    assert!(replay.events.iter().all(|event| event.at == FIRST));
    assert_eq!(replay.duration, END);
}
