# ML-13 validation

Implementation follows the October 3, 2026 interview decisions, including the
name-derived filename amendment to PRD sections 4.2 and 9. Scope is loading,
selection, diagnostics, and validated action snapshots; later stories connect
recording saves, configuration, deletion, and native playback.

## Automated evidence

Tests exercise public Rust repository and frontend library/action/toast boundaries.
Repository fixtures use real isolated directories and never inject native input.
Coverage includes mixed committed/temporary files, field validation, progressive
updates, executable-relative directory creation, deterministic duplicates/order,
restart reload, lazy snapshot integrity, and initial filename collision handling.
Frontend coverage includes stable selection during progressive updates, stale
revision suppression, persistent per-file diagnostics, clicked-ID actions, and
listener/timer cleanup.

Completion command on October 3, 2026: `npm.cmd --prefix app run verify` — **PASS**
(exit 0), against refreshed `origin/develop` merge base `84e6596`. Every gate passed:
formatting, types, lint, production frontend build, Clippy, 24 frontend/tooling
tests, 19 Rust tests (including 12 repository tests), 33 Python quality tests, and
complexity ≤10 with no exceptions. Empty, mixed-file, restart, and executable-path
checks passed through isolated real filesystem fixtures.

The initial gate identified formatting and unsupported JSON/repetition-macro test
syntax; these were corrected using plain JSON fixtures and ordinary iterator
expressions. The final gate passed without quality-tooling changes.

## Standards

Review against starting commit `84e6596` found one named-constants violation.
Button-state count/slots now use named constants, and the valid repository fixture
derives its schema version from the source constant. Re-review found no remaining
Standards findings or additional code-smell findings.

## Spec

Review found one metadata-preservation defect: unknown event fields were dropped
from prepared documents. They now survive serialization, verified by a regression
test. Unknown playback metadata is also preserved in action documents while being
excluded from the library cache. Re-review found no remaining Spec findings.

Review totals: Standards 1 resolved / 0 remaining; Spec 1 resolved / 0 remaining.

## Native and manual evidence

No target-application recording/playback claims are made by deterministic tests.
Manual desktop checks and the release cold-launch p95 performance measurement
remain distinct from automated repository evidence. Live Record/Stop/Play and
configuration/deletion persistence belong to subsequent stories.
