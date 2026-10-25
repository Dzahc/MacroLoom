# ML-26 — Inline rename validation

Spec: [confirmed behavior](../specs/ML-26-inline-rename.md). Issue: [#38](https://github.com/Dzahc/MacroLoom/issues/38).

## Deterministic evidence

- `app/scripts/rename.test.ts`: save-triggered validation, Unicode limits/control restrictions, ordinary versus discarded failures, cancellation on accepted global start, lost acknowledgement recovery, uncertain-result Retry, and Stop preserving a failed draft after cancelling the waiting start.
- `app/scripts/library.test.ts`: production inline editor markup has labelled independent controls, hides duration, and avoids an incompatible UTF-16 maxlength.
- Session command tests: pending native Stop registration, immediate cancellation of a deferred start, first-request exclusion, recording after a failed rename, rename exclusion during a session, and close success/failure ownership.
- Repository integration test: name-only updates preserve every other property and extension, survive a fresh repository load, and keep normalized no-op saves quiet.

## Desktop and visual checks

No browser or native computer surface was connected in this session, so interactive visual checks were unavailable. Static production rendering is covered above. The following checks remain manual evidence, separate from deterministic tests:

- At the minimum 360px window width, rename through right-click and F2; verify full selection, boxed icons, readable errors, focus, and no horizontal overflow in light/dark themes.
- Exercise Enter/Escape/Tab, click-through cancellation, application switching, IME entry, and double-click Play after cancelling.
- With another application focused, F9 cancels an unsaved draft and starts recording without activating MacroLoom. Verify F8 can cancel a start waiting for an intentionally delayed save.
- Delay/fail a rename write and exercise Record/Play, repeated starts, Stop, Close, and lost IPC acknowledgement. Confirm no duplicate writes, unintended later starts, or false success feedback.

## Completion gate

`npm.cmd --prefix app run verify` passed all checks: origin fetch, pinned environments, formatting, TypeScript, ESLint, production build, maintained frontend/Rust/Python tests, and changed-file complexity (524 function checks, zero failures). Seven existing desktop-only Rust tests remained explicitly ignored. A subsequent deferred-start delivery regression passed focused session tests; final review and verification follow.
