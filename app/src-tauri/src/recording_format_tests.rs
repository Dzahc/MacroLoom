//! Deterministic serialization limits and millisecond ordering, independent of native capture.
use super::*;
use test_support::{environment, key};
const TEST_EVENT_COUNT: usize = 1;
const FIRST_OFFSET: Duration = Duration::from_micros(1200);
const SECOND_OFFSET: Duration = Duration::from_micros(1800);
const EXPECTED_OFFSET_MS: u64 = 1;
const TRAILING_DURATION: Duration = Duration::from_millis(30);
const EXPECTED_EVENT_COUNT: usize = 2;

#[test]
/// File-size exhaustion and event-count exhaustion both reject the next event without consuming budget.
fn bounds_reject_the_next_event_without_mutating_the_valid_prefix() {
    let mut recording = Recording::new(environment()).expect("Metadata");
    let bytes_before = recording.bytes;
    assert!(!recording.accept(Duration::ZERO, &key(true), macro_format::MAX_EVENTS));
    assert_eq!(recording.bytes, bytes_before);
    recording.bytes = macro_format::MAX_FILE_BYTES as usize;
    assert!(!recording.accept(Duration::ZERO, &key(true), TEST_EVENT_COUNT));
    assert_eq!(recording.bytes as u64, macro_format::MAX_FILE_BYTES);
}
#[test]
/// Quantization retains tie order, native metadata and trailing duration without adding releases.
fn quantized_offsets_preserve_observed_order_and_duration() {
    let mut capture = Capture::default();
    capture.append(FIRST_OFFSET, key(true));
    capture.append(SECOND_OFFSET, key(false));
    capture.finish(
        TRAILING_DURATION,
        (test_support::TEST_X, test_support::TEST_Y),
    );
    let mut recording = Recording::new(environment()).expect("Metadata");
    recording.freeze(&capture);
    let bytes = serde_json::to_vec(&recording.document).expect("Serialize");
    let document = macro_format::decode(&bytes).expect("Valid boundary stream");
    assert_eq!(document.events.len(), EXPECTED_EVENT_COUNT);
    assert_eq!(document.events[0].at_ms, EXPECTED_OFFSET_MS);
    assert_eq!(document.events[1].at_ms, EXPECTED_OFFSET_MS);
    assert_eq!(document.duration_ms, milliseconds(TRAILING_DURATION));
    let EventData::KeyUp { native, .. } = &document.events[1].data else {
        panic!("Expected observed release");
    };
    assert_eq!(native.scan_code, test_support::TEST_SCAN);
}
