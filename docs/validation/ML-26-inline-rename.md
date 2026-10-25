# ML-26 — Inline rename validation

Spec: [confirmed behavior](../specs/ML-26-inline-rename.md). Issue: [#38](https://github.com/Dzahc/MacroLoom/issues/38).

## Deterministic evidence

- `app/scripts/rename.test.ts`: save-triggered validation, Unicode limits/control restrictions, ordinary versus discarded failures, cancellation on accepted global start, lost acknowledgement recovery, uncertain-result Retry, and Stop preserving a failed draft after cancelling the waiting start. Regression tests also keep unknown-outcome recovery accessible after session startup and serialize repeated Retry queries.
- `app/scripts/library.test.ts`: production inline editor markup has labelled independent controls, hides duration, and avoids an incompatible UTF-16 maxlength.
- Session command tests: native Stop registration before deferred arrival, immediate cancellation of a deferred start, first-request exclusion, recording after a failed rename, rename exclusion during a session, and close success/failure ownership. Record arriving during native Stop arming takes priority; unavailable Stop rejects deferred starts while preserving ordinary metadata saving.
- Repository integration test: name-only updates preserve every other property and extension, survive a fresh repository load, and keep normalized no-op saves quiet.

## Desktop and visual checks

No browser or native computer surface was connected in this session, so interactive visual checks were unavailable. Static production rendering is covered above. The following checks remain manual evidence, separate from deterministic tests:

- At the minimum 360px window width, rename through right-click and F2; verify full selection, boxed icons, readable errors, focus, and no horizontal overflow in light/dark themes.
- Exercise Enter/Escape/Tab, click-through cancellation, application switching, IME entry, and double-click Play after cancelling.
- With another application focused, F9 cancels an unsaved draft and starts recording without activating MacroLoom. Verify F8 can cancel a start waiting for an intentionally delayed save.
- Delay/fail a rename write and exercise Record/Play, repeated starts, Stop, Close, and lost IPC acknowledgement. Confirm no duplicate writes, unintended later starts, or false success feedback.

## Selected-row pencil extension

- `app/scripts/rename-pencil.test.ts` covers explicit selected-target activation, loading exclusion, and disabled explanations for sessions, saves, dialogs, deletion, unavailable storage, and a missing editor owner.
- `app/scripts/library.test.ts` covers one pencil on the selected row, reserved slots across all normal rows, independent sibling buttons, preserved selection during deletion, native disabled semantics, and reason tooltips restricted to the non-focusable icon hover target.
- Interactive Chrome access was attempted for this extension, but app access approval timed out. Hover/focus behavior, tooltip timing, and minimum-width visual layout remain manual checks. Check selected versus unselected rows, stable name/duration positions, keyboard Tab activation, and that double-clicking the pencil never starts playback. After mouse selection, moving the pointer away must hide the pencil; keyboard-visible focus must reveal it. The enabled Rename tooltip and disabled-reason tooltip belong only to the icon hover target.
- Completion verification and the separate Standards/Spec reviews for this extension are recorded after the final gate below.

## Completion gate

Final command on 2026-10-25: `npm.cmd --prefix app run verify` — **PASS**, exit code 0, against code commit `ad3429a`.

Every gate passed: origin fetch, pinned environments, Prettier, TypeScript, ESLint, production build, maintained frontend/tooling tests, rustfmt, Clippy, Rust tests, Python quality environment, complexity fixtures, and changed-file complexity. The final complexity report checked 1,074 functions across index/worktree snapshots with zero failures at the limit of 10. Seven existing desktop-only Rust tests remained explicitly ignored.

## Standards review

Reviewed separately against `30a6673...ad3429a`. The concurrent-Retry ownership/timer issue and the unavailable-Stop precondition were fixed with regressions. Final report: **0 outstanding documented violations; 0 actionable smell findings**.

## Spec review

Reviewed separately against the confirmed spec. The inaccessible unknown-result recovery state and the early F8 registration gap were fixed with regressions. Final report: **0 unresolved findings**. No unrelated scope expansion was identified.
