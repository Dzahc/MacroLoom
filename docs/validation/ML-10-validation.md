# ML-10 verification

## Try the desktop views

From the repository root, run:

```powershell
npm.cmd --prefix app run tauri -- dev
```

1. Click **Open development pane**. A separate controls window stays available
   while the main window is compact. These controls change presentation directly;
   Record, Play, and Stop session actions are outside this story.
2. Select **Recording (no name)**: the same main window shrinks to its native
   frame, toolbar, red indicator, and elapsed time. The library disappears.
3. Try **Playback / long name**, **Indefinite playback**, **Between runs**, and
   **Stopping / cleanup**. Check run totals, elapsed time, interval countdown,
   full-name tooltip, stable banner height, and visible Stop button.
4. Move the compact window. It stays topmost and cannot be resized. Choose
   **Idle / restore full view** or **Saving / full view** to restore the original full
   placement. Re-enter compact: its last position returns. Repeat from a
   maximized full window and check maximized restoration.
5. With library entries present, select a macro and scroll the list before
   entering compact. Check that selection and scroll return afterward.
6. Enable **Apply after 3 seconds** and progress updates, then select a state
   and focus another application before the delay expires. Entry, frequent
   status updates, and return to full view should leave that application focused.
   Deliberate clicks and title-bar dragging may activate MacroLoom normally.
7. Close the controls window while compact: the main window returns to idle/full
   so development controls can be opened again.

The browser preview (`npm.cmd --prefix app run dev`) provides sample library,
status, and notification controls. It verifies presentation only; native bounds,
topmost behavior, and focus require the desktop app. Ordinary notifications hide
and retain their remaining lifetime while compact. Transition failures use timed
toasts below the toolbar, without persistent errors or Retry buttons.

## Recorded checks

On October 6, 2026, the completion gate passed:

```powershell
npm.cmd --prefix app run verify
```

The gate covers formatting, TypeScript, ESLint, production frontend build,
rustfmt, Clippy, maintained frontend/Rust/Python tests, and changed-file
complexity. Local output is retained in the ignored
`app/.quality-output/ML-10-verify.log`.

Final result: **PASS** — 46 frontend/tooling tests, 30 Rust tests, and 33 Python
quality fixtures passed; changed-file complexity had zero failures. The native
desktop test is intentionally excluded from the unattended gate and passed
separately below. Production output contains no development controls.

The interactive Windows adapter check passed separately:

```powershell
cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline native_compact_bounds -- --ignored --test-threads=1 --nocapture
```

This check used disposable native frames at **DPI 168 / 175% scaling**. It passed
normal and maximized restoration, physical frame sizing, topmost/non-resizable
styles, verified off-screen setup and full-frame work-area correction, partial
restoration rollback, and external foreground preservation
through entry, repeated updates, and restoration. It exercises the actual Win32
adapter, independently of the Tauri/WebView integration.

Browser checks at 360 logical pixels confirmed no horizontal overflow, hidden
library with preserved selection and scroll (67.43 pixels), full long-name
tooltip and accessible text, and identical recording/playback/interval banner
height (67.69 pixels).

Deterministic tests cover status formatting, infinite runs, ordinary notification
pause/resume, serialized transitions, rollback, failed restoration, and prevention
of repeated failure notifications. No input capture, injection, or recording
files were used by these demonstrations.

Still requiring manual evidence: the complete Tauri UI checklist above, additional
display/text scaling settings, multiple monitors and cross-monitor DPI changes,
actual negative-coordinate monitors, and screen-reader use. Negative coordinates
and restoration failures have deterministic policy coverage, separate from this
remaining Windows evidence.

## Final review

The code-review skill ran separate Standards and Spec reviews against
`4c437431f9ea42dea0f3db865337d72572ab1d83`.

### Standards

Three findings: unchecked native test setup, added semantic literals bypassing
named constants, and outdated changed-function documentation. Addressed by
checking actual off-screen placement and every frame edge, reusing/adding named
constants, and updating the affected contracts. No independent code-smell
changes were requested.

Follow-up Standards review: zero remaining actionable findings.

### Spec

One finding: a later restoration failure could leave the native frame at full
bounds while the presentation still reported compact. Addressed by restoring
the prior compact placement, constraints, styles, and stacking state after a
partial failure. The interactive native regression injects a constraint failure
after the full placement has already changed, then checks compact bounds,
topmost, frame styles, external focus, and subsequent successful restoration.

Follow-up Spec review: zero remaining actionable findings.
