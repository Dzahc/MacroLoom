# ML-21 validation

Implementation follows the October 6, 2026 interview decisions. Confirmed Delete
removes the loaded stable ID's trusted file association without reading or
comparing its contents. Missing files fail. Both success and error notifications
use the ordinary four-second toast timeout; there is no persistent deletion-error
banner or Retry control. These decisions supersede the original issue's
persistent-error requirement and are reflected in PRD section 4.6.

The existing named confirmation remains shared by toolbar and context-menu
actions. Cancel, Escape, and Close retain the confirmation story's behavior.
During disk deletion the dialog closes, the banner names the pending target,
the entry remains until success, and Record/Play/Configure/Delete are disabled.
Selection and scrolling remain available. Removing the selected macro clears
selection; selecting another entry while waiting preserves that selection.

## Automated evidence

- Public repository tests use isolated real filesystem fixtures for deletion,
  unchanged neighbors, externally edited content, missing-file failures,
  operation exclusion, and restart absence.
- A Windows-specific integration test holds a real file handle denying sharing;
  failed deletion preserves its bytes and entry, then succeeds after unlock.
- Controller/confirmation tests verify stable-ID dispatch, cancel/no deletion,
  duplicate suppression, named progress, action exclusion, selection changes,
  timed success/failure toasts, event/result revision races, and reconnect cleanup.
- Rendering tests verify disabled toolbar controls, enabled rows, and accessible
  progress announcements. Existing tests cover confirmation cancellation and
  accessible outcome text/dismissal.

The native delete command runs on a blocking worker. The backend reserves one
deletion and rejects unfinished discovery, unavailable storage, unknown IDs,
and lazy action reads during removal. Input-prototype mode rejects saved-library
deletion entirely: its capture/playback engine has a separate in-memory library,
and native input cannot start in normal saved-library mode.

## Completion and review

Completion command on October 6, 2026: `npm.cmd --prefix app run verify` — **PASS**
(exit 0), against refreshed `origin/develop` merge base `216113e`. Every gate
passed: formatting, types, lint, production frontend build, rustfmt, Clippy,
38 frontend/tooling tests, 24 Rust tests (including the real Windows locked-file
test), 33 Python quality tests, and complexity ≤10 with no exceptions.

Independent Standards and Spec reviews use starting commit `216113e` as their
fixed point.

### Standards

One documented named-constants finding was resolved: the unavailable-selection
message is now shared, and deletion fixture counts have names. No additional
code-smell or architecture/safety findings were reported.

### Spec

No findings. The implementation matches the user's amended deletion requirements,
including metadata revision races, selection reconciliation, disconnect cleanup,
filesystem failures, and operation exclusion within the current mode boundary.

Review totals: Standards 1 resolved / 0 remaining; Spec 0 findings.

## Manual evidence

No interactive native-window confirmation, focus, or scrolling check was run in
this task. The automated Windows filesystem test is integration evidence, while
controller/rendering tests are deterministic evidence. Native input/session
integration remains governed by the separate session stories.
