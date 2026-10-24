# ML-18 safe playback targeting validation

Scope: [owner-reviewed specification](../specs/ML-18-safe-playback-targeting.md).

## Deterministic boundaries

Saved-library session tests use real bounded files/repository reads and substitute
only desktop effects. They establish rejection before a keyboard prefix can
dispatch, preservation of the valid saved document, and rejection of all four
positioned variants at gaps, exclusive edges and extreme signed coordinates.
Valid inclusive origins and last pixels on negative/offset monitors reach the
dispatch boundary unchanged at simulated 100%, 125%, 150% and 175% scales.
Existing startup, environment interruption, cancellation and owned-input cleanup
tests continue to apply. These checks do not establish physical Windows landing.

The original successful-playback fixture moved from x=-20 to x=30 even though its
only recorded monitor ended at exclusive x=0. Its movement now stays on that
monitor; deliberately invalid destinations have their own rejection tests.

## Native Windows observations

October 23, 2026, Windows 11 x64 build 26200, interactive desktop:

| Display   | Physical bounds        | Scale |
| --------- | ---------------------- | ----- |
| Primary   | (0, 0), 3840 × 2160    | 175%  |
| Secondary | (3840, 0), 1920 × 1080 | 100%  |

`cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline --lib native_view_tests -- --ignored --nocapture --test-threads=1`
— **PASS**, exit 0; four native tests passed. Full local output:
`app/.quality-output/ML-18-native-targeting.log`.

- Exact physical injection: 64 diagonal pixel samples on each attached monitor
  landed at their requested pixels. The old endpoint-based conversion landed one
  pixel left of (120, 457); targeting each pixel's normalized interval center
  corrected that regression. Samples below the compact frame and inside the full
  window's former library area were permitted.
- Controlled native caption messages: nonactivating response, moved physical
  bounds, collision rejection at the moved frame, and ordinary full-window
  activation behavior after restoration passed.
- An actual injected mouse press/move/release on a disposable title bar moved it
  by the requested displacement, preserved the pre-gesture foreground, and left
  no button held. Both the native no-activation extended style and caption message
  handling are needed; the original extended style is restored with the full view.
- Normal/maximized restoration, retained compact placement, work-area fitting,
  topmost state, repeated status updates and failed-restoration rollback passed.

Native checks use disposable registered frames, release their own button on
failure and restore the original cursor. Run them serially with physical input
released: shared input remains enabled and can interfere with observations. One
combined run observed a two-pixel movement discrepancy and then held input; the
isolated drag and subsequent complete native run passed. No independent manual
computer use is claimed. No Google Play Games testing or interaction occurred.

## Remaining ordinary-application evidence

The current desktop does not supply 125%, 150% or negative-coordinate displays.
Those physical configurations remain pending; simulated metadata is not physical
evidence. Do not change the owner's live display layout for an unattended check.

Also retain explicit manual checks for record/save/play clicks, wheel and drags in
ordinary applications at those configurations; real layout-change interruption;
moving the compact toolbar during a recorded wait while playback continues;
recording-time manual dragging/control exclusion; and collision cleanup during an
active drag. Existing deterministic or disposable-window checks do not establish
those complete workflows. ML-18 remains open until its physical acceptance matrix
has evidence.

## Completion gate and review

Final repository verification and independent Standards/Spec review results will
be recorded after implementation checks complete. The review base is `06ed1e9`.
