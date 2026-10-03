# MacroLoom — Product Requirements Document

| Document | Value                                                                                   |
| -------- | --------------------------------------------------------------------------------------- |
| Version  | 0.13                                                                                    |
| Date     | October 3, 2026                                                                         |
| Status   | Draft for review; v1 scope and distribution defined, technical defaults remain proposed |

## Table of contents

- [1. Product and purpose](#1-product-and-purpose)
- [2. Decisions and working defaults](#2-decisions-and-working-defaults)
- [3. Release scope](#3-release-scope)
  - [Initial release](#initial-release)
  - [Later releases](#later-releases)
- [4. Functional requirements](#4-functional-requirements)
  - [4.1 Record input](#41-record-input)
  - [4.2 Stop and automatically save](#42-stop-and-automatically-save)
  - [4.3 Load and display macros](#43-load-and-display-macros)
  - [4.4 Play a macro](#44-play-a-macro)
  - [4.5 Configure properties](#45-configure-properties)
  - [4.6 Delete a macro](#46-delete-a-macro)
  - [4.7 Global shortcuts and application exit](#47-global-shortcuts-and-application-exit)
  - [4.8 Operation outcome toasts](#48-operation-outcome-toasts)
- [5. Coordinates and playback environment](#5-coordinates-and-playback-environment)
  - [5.1 Shared input, ghost pointers, and background playback](#51-shared-input-ghost-pointers-and-background-playback)
- [6. Timing model](#6-timing-model)
- [7. UI and interaction design](#7-ui-and-interaction-design)
- [8. Technology and architecture](#8-technology-and-architecture)
- [9. Saved macro format](#9-saved-macro-format)
- [10. Reliability and distribution](#10-reliability-and-distribution)
- [11. Acceptance criteria and validation](#11-acceptance-criteria-and-validation)
- [12. Ticket planning and implementation sequence](#12-ticket-planning-and-implementation-sequence)
  - [12.1 Convert the PRD into Epics and Stories first](#121-convert-the-prd-into-epics-and-stories-first)
  - [12.2 Git workflow for implementation Stories](#122-git-workflow-for-implementation-stories)
  - [12.3 Suggested feature implementation order](#123-suggested-feature-implementation-order)
- [13. Remaining technical decisions](#13-remaining-technical-decisions)

## 1. Product and purpose

MacroLoom is a desktop application for recording mouse and keyboard input, saving recordings as reusable macros, configuring playback, replaying macros, and deleting them. The initial release targets Windows 11. The interface and macro engine should accommodate additional operating systems without a rewrite of the product.

The intended user wants to repeat a routine sequence across ordinary desktop applications without writing an automation script. The primary workflow is Record → perform actions → Stop and automatically save → select a macro → Play.

The interface must feel modern, professional, minimal, and easy to understand. Its main window contains an icon toolbar, a message banner immediately below it, and the saved macro list below the banner. During recording and playback, hide the list and shrink the window to a compact toolbar-and-banner view that stays on top.

Responsiveness is a primary product requirement. The user is comfortable with multiple languages and is open to the stack that best serves a quick, responsive application; prior language experience does not constrain the implementation to TypeScript.

## 2. Decisions and working defaults

This document distinguishes requirements supplied by the user from recommendations made during brainstorming. Recommended defaults make the draft actionable and can be revised without reopening the core scope.

| Item                         | Decision or working default                                                                                                                                                        | Basis                                                                                       |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Initial operating system     | Windows 11; first packaged build targets x64                                                                                                                                       | Windows 11 is the user's priority; x64 is a proposed packaging default                      |
| Input scope                  | Positioned left/right clicks, double clicks, vertical scrolling, basic click-and-drag, and keyboard input                                                                          | Original requirements and subsequent clarification                                          |
| Click meaning                | A click includes its pixel position; the proposed system-input backend moves the shared cursor there before clicking                                                               | User clarified positioned clicks and deferred independent playback from v1                  |
| Movement scope               | Record timed cursor positions while a supported mouse button is held for basic dragging; free cursor paths, hover replay, and drawing precision remain deferred                    | Latest user instruction permits basic dragging in v1 when it is a straightforward extension |
| User's technology experience | Mostly Java, TypeScript, and Python; some C# experience from longer ago                                                                                                            | Explicit user clarification                                                                 |
| Technology priority          | Quick response; no required programming language                                                                                                                                   | Explicit user clarification                                                                 |
| Selected stack               | Tauri 2, TypeScript, React, Vite, and a Rust backend, subject to prototype validation                                                                                              | User accepted the stack and confirmed prototype validation as the first ticket work         |
| Timing format                | Millisecond offsets from recording start, measured with a monotonic clock                                                                                                          | Recommendation from timing research                                                         |
| Coordinate reference         | Absolute physical screen pixels in v1; window-relative mode and its window selector deferred to v2                                                                                 | Explicit user decision to keep v1 simple                                                    |
| Typical workload             | A macro usually stays within one application window; several instances of the same application may be open                                                                         | User described use including four Google Play Games windows                                 |
| Playback concurrency         | One active macro in v1; architecture supports distinct future playback sessions                                                                                                    | Original requirements                                                                       |
| Input independence           | V1 shares the desktop cursor and keyboard focus; independent/background playback and ghost-pointer input are outside v1                                                            | Explicit user decision after discussing isolation requirements                              |
| Virtualization               | V1 requires no VM software or hypervisor configuration; VM integration is outside scope                                                                                            | Explicit user decision to omit the isolated-playback approach from v1                       |
| Storage                      | A fixed `macros` directory beside the executable                                                                                                                                   | Original requested save location                                                            |
| Distribution                 | Portable ZIP for v1; setup executable in a later release                                                                                                                           | Explicit user decision                                                                      |
| Global Stop                  | Fixed global Stop shortcut in v1; F8 is the proposed key assignment                                                                                                                | User approved the feature; exact key remains a recommended default                          |
| Global Record                | Fixed global Record shortcut in v1; F9 is the proposed key assignment                                                                                                              | User requested a Record shortcut; global behavior and exact key are recommended defaults    |
| Playback startup             | No added startup countdown in v1; optional three-second countdown in a later release                                                                                               | Explicit user decision                                                                      |
| Playback focus               | Include a click into the intended window before typing in the recording; playback replays that click. No global Play shortcut or automatic previous-window focus restoration in v1 | Explicit user decision                                                                      |
| Active-session window        | Hide the macro list and shrink the window to the toolbar and message banner during recording/playback; keep this compact view on top                                               | Explicit user direction                                                                     |
| Main layout                  | Icon toolbar, then message banner, then macro list                                                                                                                                 | Explicit user decision                                                                      |
| Operation feedback           | Brief success/failure toasts for saving recordings, saving property changes, and deleting macros                                                                                   | Explicit user decision                                                                      |
| Delivery workflow            | Convert the PRD into numbered Epics and Stories first; the user manually assigns implementation Stories to agents                                                                  | Explicit user decision                                                                      |
| Visual defaults              | Follow the OS light/dark preference; restrained neutral surfaces and one accent color                                                                                              | Proposed design direction                                                                   |

## 3. Release scope

### Initial release

- Launch a desktop GUI with a toolbar and saved macro list.
- Record timed keyboard input, positioned mouse clicks/scrolling, and basic click-and-drag across ordinary desktop applications.
- Start recording through the Record button or a global shortcut while the target application has focus.
- Automatically save a completed recording with a unique default display name.
- Load macros from the fixed local directory at startup.
- Play a selected macro through the toolbar or by double clicking its list entry.
- Start playback without an added startup countdown, preserving recorded event timing.
- Configure its name, playback speed, repetition, and interval between runs.
- Stop recording or playback through the main window or a global shortcut.
- Hide the macro list and shrink the window during recording/playback; keep the toolbar and message banner visible and on top.
- Confirm deletion, then remove the file and list entry.
- Show brief success/failure toasts for recording saves, property saves, and deletion.
- Preserve valid files and recoverable recordings when storage operations fail.
- Distribute a portable ZIP that runs without installing a programming language or development tools.

### Later releases

| Area                           | Deferred capability                                                                                                                               |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Input                          | Cursor paths with no button held, hover replay, precision drawing/path fidelity, middle/extra buttons, and horizontal scrolling                   |
| Editing                        | Individual event editing, event insertion/deletion, timeline trimming, and pause/resume                                                           |
| Window-relative targeting (v2) | Window-relative coordinates and selection of a specific playback window, including multiple instances of the same application                     |
| Targeting                      | Automatic target-window activation, optional automatic focus restoration to the previously active window, and adapting to changed display layouts |
| Automation                     | Waiting for application state, image recognition, conditions, and macro composition                                                               |
| Organization                   | Search, folders, tags, import/export workflows, and a file browser                                                                                |
| Preferences                    | Custom save directories, configurable shortcuts, explicit theme selection, and a general options menu                                             |
| Platforms                      | macOS and Linux input backends; Windows ARM64 packaging                                                                                           |
| Execution                      | Optional three-second playback startup countdown, scheduling, queues, and coordinated concurrent playback                                         |
| Independent playback           | Application-specific background automation or isolated execution; possible later investigation, with no committed release                         |
| Distribution                   | Setup executable, automatic updates, and additional distribution channels                                                                         |

Direct copying of compatible macro files between local folders remains possible; dedicated import/export UI is deferred.

Independent/background playback and ghost-pointer input are excluded from v1 by user decision. V1 uses ordinary desktop input and requires no virtual-machine setup. Basic click-and-drag remains included. See section 5.1 for the shared-input limitation.

## 4. Functional requirements

### 4.1 Record input

1. Clicking Record or pressing the global Record shortcut while idle starts the recording clock and capture immediately through the same workflow. The Record interaction itself, including shortcut key-down, repeats, and key-up, is excluded. Shortcut activation must preserve the target application's focus.
2. Hide the macro list and shrink the main window to its compact toolbar-and-banner view, which becomes topmost. Show a red recording indicator, elapsed time, and an enabled Stop button. The layout transition must not take focus from the target application or delay capture.
3. Capture keyboard down/up events, modifier keys, system-key combinations, and observed repeated key-down events. This preserves shortcuts, held keys, and recorded key repetition.
4. Capture left/right button down/up timing and the physical pixel position of each event. Preserve the pressed state across intervening movement, keyboard, or scroll events.
5. Represent a double click as two ordered press/release pairs with their recorded positions and timing. Do not add a second synthetic double-click action.
6. Capture vertical wheel input with signed wheel delta, cursor position, and timestamp. Preserve the recorded amount rather than replacing every scroll with one fixed notch.
7. Moving the cursor between completed clicks is allowed. Movement with no recorded supported mouse button held is not stored or replayed; each subsequent click/scroll still includes its own position.
8. Include basic click-and-drag by recording timed `mouse_move` positions while a recorded left/right button is held, followed by the release at its actual location. Preserve modifier transitions during a drag. This supports ordinary drag-and-drop, text selection, sliders, and window movement/resizing without adding a separate gesture editor. Full cursor-path replay and drawing precision are outside v1.
9. Exclude MacroLoom's own control interactions and reserved Record/Stop shortcuts from saved events. Establish the recording end at the initiating Stop interaction so its input is not appended to the macro.
10. Preserve event order when timestamps are equal. Do not synthesize missing key presses for inputs already held before recording began.
11. Recording and playback cannot overlap in v1. Both the GUI and backend enforce this rule.

V1 records key actions rather than inferring text. Reproduction assumes the same keyboard layout; semantic text entry and IME-specific behavior are deferred.

Basic dragging extends the existing mouse event stream rather than introducing a distinct drag command: button down → timed movement points → button up. Windows exposes movement and button transitions through the same low-level mouse callback and supports replaying movement while the button remains down. This makes basic dragging a modest architectural extension, although target-application behavior still needs integration validation. [Microsoft mouse capture documentation](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc), [Microsoft mouse input documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput).

Limit movement volume through modest sampling/coalescing, with an initial target of approximately one retained position per 8 ms during movement. Preserve the first movement after a press, the final movement position, and the exact button transitions. Coalesce only consecutive movement events; flush a pending position before any keyboard, button, or wheel event so ordering and modifier behavior remain intact. Store retained points at their original capture times. Validate this density against the supported drag cases before freezing it; high-rate drawing fidelity is deferred.

### 4.2 Stop and automatically save

1. Clicking Stop or pressing the global Stop shortcut ends capture and freezes the event sequence and duration.
2. Release capture resources promptly, restore the full library view and normal window size/topmost state, and show “Saving…” in the banner until persistence completes. Restoring the view must not activate the window or take focus from another application.
3. Save nonempty recordings automatically to `<executable_directory>/macros/` with no Save As dialog.
4. Generate a default display name such as `Macro 2026-09-28 14-32-08`; append a numeric suffix when needed to keep generated names unique.
5. Assign a stable UUID independently of the filename. Derive the initial filename from the display name, replacing whitespace and Windows-prohibited characters with underscores, handling reserved device names and trailing periods/spaces, and using `macro` if the resulting stem is empty. Resolve case-insensitive collisions with `_2`, `_3`, and subsequent numeric suffixes; never replace another macro.
6. Write the file through a temporary file and atomic commit where supported, so an interrupted write does not replace a valid file with partial JSON.
7. After successful save, add the macro to the list, select it, show a brief success toast naming the saved macro, and return to idle.
8. An empty recording produces “No input recorded” and no macro file.
9. If saving fails, show a brief failure toast, retain the recording in memory, and keep the error with Retry and Discard visible after the toast disappears. Do not claim it was saved or silently lose it. Discard requires an explicit user action.
10. Create the save directory when absent. If it is unwritable, explain the problem before recording starts; do not silently switch storage directories.

Resolve the storage path from the executable's directory, never the process's current working directory. No custom-directory preference is included in v1.

### 4.3 Load and display macros

1. At startup, discover committed macro JSON files in the default directory and validate them independently.
2. Show each valid macro's display name as its primary label and recorded duration as secondary information. Sort by most recently created first, with a deterministic tie-breaker.
3. Clicking an entry selects it. Right clicking an entry selects that entry and opens Configure and Delete actions.
4. Double clicking an entry starts that macro through the same playback workflow as Play.
5. When no macros exist, show “No macros yet. Record your first macro.”
6. Ignore temporary save files. Unsupported or malformed files must not prevent other macros from loading. Immediately enqueue one manually dismissed error toast per failed file, naming its filename, failing field or part, reason, and correction/restart guidance. Preserve those files. When no valid entries remain, use the usual first-recording empty state even if files failed validation.
7. Validate committed files sequentially in deterministic alphabetical filename order. Add each valid macro immediately in display order; loaded entries are usable while discovery continues and selection follows stable ID. The first valid file for a duplicated ID wins; later duplicates remain untouched and produce error toasts. If discovery is interrupted, retain validated entries and explain the incomplete scan; restart retries discovery.
8. Keep summaries, properties, file associations, and content fingerprints in memory. Load full events only for an action and validate the same bytes used by that action. Unexpected external changes reject the action with restart guidance; successful application saves refresh cached properties and fingerprints. No lifetime file locks or polling are required. Playback retains its own stable session snapshot.
9. Readable but unwritable storage still permits loading and playback; operations requiring writes fail with an explanation. Never silently switch storage directories. Show loading feedback until discovery completes; reserve the first-recording state until a completed scan has no valid entries.
10. Reload the library on the next launch. Live watching of externally changed files is deferred.

### 4.4 Play a macro

1. Play is enabled only when a valid macro is selected and the application is idle.
2. Copy the macro and its properties into a playback session so a run uses a stable configuration.
3. Hide the macro list, shrink the main window to its compact toolbar-and-banner view, and keep that view on top. Start the playback session immediately, without an added startup countdown. Honor any recorded delay before the first event. The layout transition must not take focus from the target application.
4. Begin the elapsed-time clock when the session starts. It includes recorded waits and intervals between runs.
5. Before each mouse press, release, or scroll, move the cursor directly to that event's recorded pixel position, then inject its action. Replay retained `mouse_move` points at their deadlines while preserving the recorded button state. Do not generate additional interpolated points; movement between completed gestures is represented only by the next positioned action.
6. Replay keyboard down/up, mouse movement, and mouse press/release events in order according to scaled event deadlines. Playback runs independently of the frontend's timers. At the end of each run, release any remaining session-owned held input before starting a repeat interval or another run.
7. Show a busy indicator, macro name, current run number, total runs where finite, and elapsed time. Examples: `Playing “Fill form” · Run 2/5 · 00:18` and `Playing “Fill form” · Run 12/∞ · 03:40`.
8. Between runs, show the completed run number and remaining interval: `Run 2/5 complete · Next run in 3.0 s`. Increment the run number when the next run starts.
9. Stop remains available during event waits, active input, and repeat intervals. It cancels queued work and releases any keys/buttons still held by that playback session.
10. On completion, cancellation, or error, restore the macro list, previous full window size/position, and normal topmost state after input cleanup. Preserve the library selection and scroll position and return to idle. Progress updates and restoring the view must not take focus away from the target application.
11. Disable starting another recording or playback while a session is active. Configure and Delete are unavailable while recording, saving, or playing.
12. If input injection fails or partially succeeds, stop the session, clean up owned held input, and show a failure message. Do not display successful completion.
13. If the system suspends, the session is interrupted; do not replay a backlog of overdue events on resume.

The compact view is movable and must fit the toolbar and readable message banner without leaving the hidden list's empty window area on screen. It must be positioned away from recorded click locations before playback; topmost visibility must not be implemented by repeatedly activating the window. If a playback mouse target overlaps the currently visible compact window, stop before injecting that mouse action and explain that the window must be moved. The hidden list's former bounds must not count as an obstruction. Do not let playback trigger MacroLoom's controls accidentally.

For a macro that types into another application, the user includes a click into the intended window/input field before typing while recording. Playback repeats that click before the recorded text keystrokes; users do not need to race playback with an additional manual focus click. V1 starts through Play or double click and does not insert a focus click, automatically restore the previously active window, or offer a global Play shortcut. Include this recording guidance in the UI/help. The compact toolbar remains topmost without continually taking focus.

### 4.5 Configure properties

Configure opens a separate owned window for the selected macro. The main toolbar and context menu open the same window.

| Field                 | Initial value            | Behavior                                                                                  |
| --------------------- | ------------------------ | ----------------------------------------------------------------------------------------- |
| Name                  | Generated recording name | Editable, trimmed, 1–120 characters; reject empty values and control characters           |
| Playback speed        | 1×                       | Presets: 0.25×, 0.5×, 1×, 2×, 4×                                                          |
| Repeat mode           | Once                     | Once, fixed count, or indefinitely                                                        |
| Total runs            | 1                        | Enabled for fixed count; a positive integer; includes the first run                       |
| Interval between runs | 0 seconds                | Enabled for fixed/infinite modes; finite nonnegative seconds, up to millisecond precision |

Save validates and persists the complete property change before updating the list, then shows a brief success toast naming the macro. Cancel, Escape, or closing the configuration window discards edits. Renaming changes metadata without changing the ID or filename. An unsuccessful Save shows a brief failure toast and leaves the prior persisted properties intact; keep the edits and actionable field/storage errors visible for correction or retry.

“Once” means one run. A fixed count of 3 means three runs total. The interval begins after a run finishes, applies only when another run follows, and is not scaled by playback speed. Indefinite playback continues until stopped or an error occurs.

### 4.6 Delete a macro

1. Delete opens a confirmation dialog naming the selected macro and stating that its saved file will be removed.
2. Provide Cancel and Delete; Cancel is the default action and Escape cancels.
3. On confirmation, delete only the file belonging to that macro ID within the library directory.
4. Remove its list entry only when deletion succeeds, then show a brief success toast naming the deleted macro. A failure leaves the entry intact, shows a brief failure toast, and keeps an actionable error visible after the toast disappears.
5. Deletion is permanent in v1; undo/trash handling is deferred.

### 4.7 Global shortcuts and application exit

V1 includes a fixed global Record shortcut; the proposed key assignment is **F9**. Display it beside the Record control or in its tooltip. Register it while idle and recording so the user can start capture while another application has focus. It starts recording only when the same preconditions as the Record button are met. Pressing or holding it during an existing recording does not restart capture, toggle recording, or create additional sessions; use Stop to finish. Suppress shortcut auto-repeat and exclude its complete key sequence from the recording. Release its reservation during playback, saving, and other unavailable states. If registration conflicts with another application, show that the Record shortcut is unavailable; the Record button remains usable when the required Stop shortcut can be registered. Shortcut customization is deferred.

V1 includes a fixed global Stop shortcut; the proposed key assignment is **F8**, displayed beside the Stop control and in active status. It works while another application has focus, so users can stop mouse-intensive playback without reaching the GUI's Stop button. It is reserved only during recording, playback, and repeat intervals. It must stop the backend directly without waiting for the web UI to process a callback. Shortcut customization is deferred. If the shortcut cannot be reserved, show the conflict and keep recording/playback unavailable until it can be registered.

Closing the main window while recording stops capture and attempts to save before exit. A save failure keeps the recording available and the application open unless the user explicitly discards it. Closing during playback cancels it and releases session-owned held input before exiting.

### 4.8 Operation outcome toasts

Saving a recording, saving macro properties, and deleting a macro must each display a brief, nonmodal success or failure toast after the outcome is known. Use clear text identifying the action and macro, such as `Saved “Fill form”`, `Updated “Fill form”`, `Deleted “Fill form”`, or `Could not save “Fill form”`. A successful retry produces a success toast only after persistence succeeds. Canceling a dialog, discarding edits, and stopping an empty recording are not successful save/delete operations.

Use application-rendered toasts in the visible application window, with a proposed default lifetime of four seconds and an accessible dismiss control. They must not take focus, open a blocking dialog, cover the Stop control, or require the user to acknowledge success. Announce their text to assistive technology. Toasts supplement persistent actionable failure messages and recovery controls; a failure must remain understandable after its toast disappears. A successful save during application exit must not delay exit solely to keep a toast visible.

## 5. Coordinates and playback environment

V1 uses signed physical pixel coordinates in the Windows virtual desktop, including negative coordinates on monitors to the left of or above the primary monitor. Browser CSS pixels and device-independent GUI coordinates must not be used as recorded screen coordinates.

Capture and replay must use compatible DPI-aware coordinate handling. The Windows backend converts stored pixel positions into the normalized virtual-desktop coordinates required by input injection. Windows documents screen-position capture in [MSLLHOOKSTRUCT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-msllhookstruct) and absolute/virtual-desktop injection in [MOUSEINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput).

Save monitor bounds and scale factors as recording environment metadata. Playback requires the same display layout, target-window position/size, and keyboard layout. A detected display-layout change blocks playback with a clear explanation; automatically remapping positions is deferred. The application does not identify target windows or verify their contents in v1.

Window-relative mode and a playback-window selector are deferred to v2. With several instances of an application open, v1 replays clicks at the recorded screen positions; the intended instance must remain at its recorded position and size.

V1 targets ordinary applications on the interactive desktop at the same privilege level as MacroLoom. Elevated targets and secure desktop interaction are outside scope. Windows restricts input injection to equal or lower integrity levels, and input-injection failures do not reliably identify that restriction as their cause. [Microsoft SendInput documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).

Changing speed changes click spacing, drag timing, and key-hold durations. A target application's double-click threshold or response time can affect recognition at nondefault speeds. V1 reproduces scaled event timing; it does not wait for the target application's state.

### 5.1 Shared input, ghost pointers, and background playback

The proposed `SendInput` backend uses the Windows desktop's shared cursor and keyboard input stream. It does not disable or lock the user's physical mouse/keyboard, but concurrent manual input can move the cursor, change focus, alter held-key/button state, and interfere with playback. The user cannot reliably work independently elsewhere on that desktop while general pixel-based playback runs. A second monitor does not provide another independent input stream. Windows documents one system cursor and foreground routing of keyboard input. [Microsoft cursor documentation](https://learn.microsoft.com/en-us/windows/win32/menurc/using-cursors), [Microsoft keyboard input overview](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input).

A drawn ghost pointer would provide a visualization rather than an independent system pointer. Independent/background playback, ghost-pointer input/visualization, and virtual-machine integration are outside the initial release. Normal MacroLoom use must not require installing VM software, enabling hypervisor features, or changing firmware virtualization settings.

Possible later investigation can consider supported application automation or an isolated execution environment. This is not a committed feature. Supported UI Automation controls expose actions such as Invoke, Value, and Scroll; replacing pixel replay with those actions would require richer target/action metadata and application compatibility validation. [UI Automation control patterns](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controlpatternsoverview).

## 6. Timing model

Store each event's integer `atMs` offset from recording start, together with the recording's total `durationMs`. Preserve initial waiting time and trailing waiting time before Stop. The total duration is not inferred solely from the final event.

Established macro tools expose inter-action pauses: Pulover's Macro Creator records idle intervals, and Jitbit provides millisecond delay statements. This establishes their documented behavior, rather than proving a particular internal file format. [Pulover recording documentation](https://www.macrocreator.com/docs/Record.html), [Jitbit statements documentation](https://www.jitbit.com/docs/macrorecorder/macro-statements.htm).

For MacroLoom, timestamps and deltas contain equivalent timing information. Timestamps are the proposed stored representation because speed changes, duration calculations, and future timeline editing remain straightforward. Playback derives the remaining wait from an absolute deadline:

```text
event_deadline = run_start + event.atMs / speed
remaining_wait = max(0, event_deadline - current_monotonic_time)
run_end        = run_start + macro.durationMs / speed
```

For events at 100, 350, and 900 ms, 2× playback uses deadlines at 50, 175, and 450 ms. Scheduling against deadlines avoids adding processing overhead to every inter-event delay. Late events preserve sequence order; waits are cancellable, and system suspension cancels the session rather than causing catch-up playback.

Use Rust's monotonic `Instant` in the backend and retain higher-resolution values internally where helpful. Round to milliseconds only for the saved format. Rust documents `Instant` as monotonically nondecreasing, with platform-dependent suspend behavior, which is why explicit suspension handling is required. [Rust Instant documentation](https://doc.rust-lang.org/std/time/struct.Instant.html).

Millisecond timestamp resolution is required. Exact one-millisecond execution accuracy is not promised by the product. Measure dispatch lateness during validation and distinguish it from capture resolution.

## 7. UI and interaction design

The full library view uses native resize/close behavior, an icon toolbar with short labels, a message banner directly beneath the toolbar, and a single-column macro list beneath the banner. Icons need accessible names and tooltips. Status must use text as well as color or animation.

Full library view while idle:

```text
┌────────────────────────────────────────────────────┐
│ MacroLoom                                          │
│                                                    │
│ ● Record   ▶ Play   ■ Stop    ⚙ Configure   🗑 Delete │
│                                                    │
│ Ready · Record: F9                                  │
│                                                    │
│ Macros                                             │
│ ┌────────────────────────────────────────────────┐ │
│ │ Update report                           00:24  │ │
│ │ Fill form                               00:08  │ │
│ │ Macro 2026-09-28 14-32-08                00:17  │ │
│ └────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────┘
```

Compact view during recording or playback:

```text
┌────────────────────────────────────────────────────┐
│ MacroLoom                                          │
│ ● Record   ▶ Play   ■ Stop    ⚙ Configure   🗑 Delete │
│ ◌ Playing “Update report” · Run 2/5 · 00:38          │
│ Stop: F8                                           │
└────────────────────────────────────────────────────┘
```

Keep the toolbar order consistent between views; unavailable actions are disabled. The compact window contains only the normal window frame, toolbar, and message banner, with no macro list or unused list-sized area. Keep it compact during recording, playback, repeat intervals, and input cleanup. After capture stops or playback cleanup finishes, restore the full library view and its previous size/position, selection, and scroll position without activating the window. If saving is still in progress, show its status in the restored full view. Changing views does not close the application or unload the library.

Use neutral backgrounds, readable typography, modest corner radii, subtle separators, and consistent spacing. Follow the system light/dark setting. Use a restrained accent for selection and primary actions, red for recording, and a small busy indicator for playback/saving. Avoid decorative animation and large dashboard panels.

| State                | Window view                              | Toolbar and message banner behavior                                                 |
| -------------------- | ---------------------------------------- | ----------------------------------------------------------------------------------- |
| Idle, no selection   | Full library, normal topmost state       | Record enabled; Play, Configure, Delete, and Stop disabled                          |
| Idle, selected macro | Full library, normal topmost state       | Record, Play, Configure, and Delete enabled; Stop disabled                          |
| Recording            | Compact, topmost                         | Stop enabled; recording indicator and elapsed time; other mutating actions disabled |
| Saving               | Full library, normal topmost state       | Saving status; prevent new sessions and show Retry/Discard if saving fails          |
| Playing              | Compact, topmost                         | Stop enabled; run counter and elapsed time                                          |
| Between runs         | Compact, topmost                         | Stop enabled; next-run countdown and elapsed time                                   |
| Stopping             | Compact, topmost until cleanup completes | Show stopping status until held input and resources are cleaned up                  |

Use keyboard navigation, visible focus indicators, scalable text, and sufficient contrast. Honor reduced-motion preferences for decorative animation. Empty states and errors should describe the next useful action without exposing implementation details.

## 8. Technology and architecture

The user has experience primarily with Java, TypeScript, and Python, plus some older C# experience, and is open to any language that supports a responsive product. The selected stack is **Tauri 2 + TypeScript + React + Vite**, with a Rust backend for capture, timing, playback, and persistence. This combines familiar web UI development with a native engine. The user accepted the Rust maintenance tradeoff, with the first tickets devoted to validating the stack through a prototype shell.

The initial prototype must demonstrate a responsive shell and native input/timing integration before broader feature implementation. Tauri supports web frontends and recommends Vite for common SPA frameworks and plain TypeScript. Its Rust core manages OS access and communication with the web UI. [Tauri frontend documentation](https://v2.tauri.app/start/frontend/), [Tauri process model](https://v2.tauri.app/concept/process-model/).

Evaluate the selected stack against the response targets in section 10. If the prototype exposes a substantial unresolved limitation, revisit the selection before broader implementation; **C# + Avalonia** is the previously discussed alternative. Its UI thread also requires long-running work to be scheduled elsewhere. [Avalonia threading documentation](https://docs.avaloniaui.net/docs/app-development/threading).

| Component                 | Responsibility                                                                                                                                 |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| TypeScript UI             | Macro list, selection, toolbar, compact/full view transitions, message banner, configuration window, deletion confirmation, and outcome toasts |
| Application service       | Valid state transitions, command validation, and enforcing one active session in v1                                                            |
| Recorder                  | Event ordering, timing, input normalization, control exclusions, and unsupported-gesture handling                                              |
| Playback engine           | Per-session clock, speed scaling, repetitions, cancellable waits, and held-input ownership                                                     |
| Macro repository          | Directory resolution, schema validation, atomic writes, loading, property updates, and deletion                                                |
| Platform input backend    | OS capture, input injection, physical-coordinate conversion, and native lifecycle notifications                                                |
| Desktop input coordinator | Grants input ownership to the active session; boundary for future coordination                                                                 |

Use Windows capture APIs through a Windows-specific Rust adapter and `SendInput` for replay. Keep capture callbacks short, timestamp and enqueue events promptly, and perform file/UI work elsewhere. Microsoft notes that slow low-level keyboard hooks can be silently removed and recommends a dedicated thread for such hooks. [Microsoft low-level keyboard hook documentation](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc).

The global Record and Stop shortcuts can use Tauri's global-shortcut support, with recording startup and cancellation handled in Rust. That API registers shortcuts; general mouse/keyboard recording requires the separate platform input backend. [Tauri global-shortcut documentation](https://v2.tauri.app/plugin/global-shortcut/).

Each future playback session has its own ID, macro snapshot, clock, counters, cancellation state, and held-input tracking. V1 grants ownership to one session. Do not hard-code session state into individual GUI widgets. Concurrent sessions will still share desktop focus, cursor position, and keyboard state; a later release must define coordination before enabling concurrent input.

Input ownership here coordinates MacroLoom's own sessions; it does not lock the user's physical input. Future independence requires a compatible action backend or a separate execution environment, rather than merely another session ID or a drawn pointer. Background UI Automation would require richer target/action metadata than the v1 pixel-event format.

The portable data model and UI do not imply automatic cross-platform input support. macOS and Linux need their own native backends and environment validation. Windows-native key metadata must be retained alongside normalized key identifiers so unsupported cross-platform playback can be recognized rather than silently misinterpreted.

## 9. Saved macro format

Use one versioned JSON file per macro. Initial filenames derive from display names as specified in section 4.2; the embedded UUID supplies stable identity. A valid JSON file can be manually renamed without changing its identity. Configuration renames change metadata without changing the existing filename. No database or account service is required for v1.

Version one requires canonical lowercase UUIDs and UTC RFC 3339 creation/update timestamps with at most millisecond precision; creation must not follow update. Unknown metadata fields are permitted and preserved for later property saves. Each file is limited to 16 MiB and 100,000 events, with no fixed library-count cap. Reject empty event arrays; allow zero duration when all events occur at zero. Integer millisecond durations, event offsets, run counts and intervals must fit JavaScript's safe integer range. Current monitor/layout differences do not make structurally valid recording metadata malformed; playback checks compatibility separately.

Required information:

- Schema version, stable ID, display name, and UTC creation/update timestamps.
- Recording platform, coordinate reference, keyboard-layout metadata, and display configuration.
- Recording duration and the ordered event array.
- Playback speed, repeat mode, total runs for finite repetition, and interval milliseconds.
- Event time/type and event-specific key, button, position, or wheel data. `mouse_move` events carry their timestamp and physical pixel coordinates; preceding button transitions establish whether a drag is active.

Illustrative shape; finalize exact key-normalization and display-metadata fields during implementation:

```json
{
  "schemaVersion": 1,
  "id": "68833c94-3d18-4fb2-8a59-0cc72382b616",
  "name": "Fill form",
  "createdAt": "2026-09-28T21:32:08Z",
  "updatedAt": "2026-09-28T21:32:08Z",
  "recording": {
    "platform": "windows",
    "coordinateSpace": "screen_physical_pixels",
    "keyboardLayout": "00000409",
    "displays": [
      { "x": 0, "y": 0, "width": 1920, "height": 1080, "scaleFactor": 1.0 }
    ]
  },
  "durationMs": 1600,
  "playback": {
    "speed": 1.0,
    "repeatMode": "once",
    "totalRuns": 1,
    "intervalMs": 0
  },
  "events": [
    {
      "atMs": 100,
      "type": "mouse_down",
      "button": "left",
      "x": 1040,
      "y": 620
    },
    { "atMs": 200, "type": "mouse_up", "button": "left", "x": 1040, "y": 620 },
    {
      "atMs": 400,
      "type": "key_down",
      "key": "KeyA",
      "native": { "scanCode": 30, "virtualKey": 65, "extended": false }
    },
    {
      "atMs": 450,
      "type": "key_up",
      "key": "KeyA",
      "native": { "scanCode": 30, "virtualKey": 65, "extended": false }
    },
    {
      "atMs": 800,
      "type": "mouse_wheel",
      "axis": "vertical",
      "delta": 120,
      "x": 1040,
      "y": 620
    },
    {
      "atMs": 1000,
      "type": "mouse_down",
      "button": "left",
      "x": 400,
      "y": 350
    },
    { "atMs": 1100, "type": "mouse_move", "x": 450, "y": 350 },
    { "atMs": 1200, "type": "mouse_move", "x": 500, "y": 420 },
    { "atMs": 1300, "type": "mouse_move", "x": 700, "y": 500 },
    { "atMs": 1400, "type": "mouse_up", "button": "left", "x": 700, "y": 500 }
  ]
}
```

Validate nondecreasing event timestamps, event times within the recording duration, supported event types, valid property values, and supported schema versions. A stored drag movement requires a preceding supported button-down that has not yet been released. Treat files as data, never executable code. Recording boundaries and playback cleanup must handle presses still held at Stop without allowing input to remain held after a run or session ends. Cleanup for an interrupted drag releases the button at the last replayed position rather than moving to an unexecuted destination.

## 10. Reliability and distribution

Keep the GUI responsive while capture and playback run. Input hooks and playback scheduling run on dedicated backend workers; library loading and file operations must not block the web UI, the native window event loop, or those workers. Commands start/cancel work promptly and publish status without waiting for an entire recording or playback to finish. Tauri supports asynchronous commands and channels for communicating with the backend. [Tauri command documentation](https://v2.tauri.app/develop/calling-rust/).

Progress updates may be throttled to approximately 10 per second; frontend update frequency must not affect input timing. Timestamp capture callbacks promptly rather than after a frontend round trip. Keep the UI dependency footprint modest and avoid expensive rerenders for each recorded event.

These are proposed performance budgets for the implementation, not measurements of an existing application:

| Interaction                                             | Target                                                                                                                                     |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Selection, toolbar feedback, and opening a context menu | Visible response within 100 ms at the 95th percentile                                                                                      |
| Starting recording or playback                          | Recording or playing status visible within 100 ms at the 95th percentile; no added startup countdown, with recorded event timing preserved |
| Global Stop                                             | Cancellation and release of session-owned held input within 100 ms at the 95th percentile under normal desktop conditions                  |
| Cold launch                                             | Usable main window and library within 2 seconds at the 95th percentile with 100 macros of up to 1,000 events each on local SSD storage     |
| Active-session interaction                              | Moving/resizing the window and using Stop remain responsive throughout recording, playback, and repeat waits                               |

Document the reference Windows 11 x64 hardware and benchmark conditions with the first working build. Measure these targets while the engine is active and assess any failures before adding visual polish or more features. File operations may take longer than 100 ms; immediate progress feedback is required while they complete. Capture resolution, playback dispatch lateness, and UI response time are separate measurements.

Record only during an explicitly started recording. Macros stay local and operation requires no cloud service or account. The interface must not claim automatic password-field exclusion.

V1 ships as a portable Windows x64 ZIP containing `MacroLoom.exe` and all required application libraries/resources. The user extracts the entire ZIP into a writable folder and launches the executable; there is no setup executable in v1. Create the adjacent `macros` directory on first launch. Users do not need to install a programming language, compiler, SDK, Node.js, or Rust; development tools are required only to build the application.

The proposed Tauri build uses the WebView2 runtime on Windows. Verify its availability on the supported Windows 11 environment and document how to obtain it if absent; this rendering runtime is separate from programming-language development tools. Validate the ZIP's runtime/resource requirements on a clean machine before release. Tauri documents installer bootstrapper and bundled-runtime alternatives for the later setup release. [Tauri Windows distribution documentation](https://v2.tauri.app/distribute/windows-installer/).

WebView runtime/cache data may use per-user locations even when macro files are stored beside the executable; do not describe the application as leaving no data outside its folder.

A setup executable is deferred to a later release. Before implementing it, revisit the app-adjacent macro directory if the installation location is protected: per-user macro storage would be a deliberate change from the original request. Do not silently make that change. Installation and update behavior must preserve existing recordings.

## 11. Acceptance criteria and validation

| Area                    | Required evidence for the initial release                                                                                                                                                                                                                                                                                                                                                                                                        |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Startup and library     | Launch with an empty library, load several valid files, and load valid files alongside a malformed/unsupported one without modifying the bad file                                                                                                                                                                                                                                                                                                |
| Record and replay       | Record and reproduce left/right positioned clicks, a double click at 1×, vertical wheel amounts, text keystrokes, modifiers, a shortcut, a held key, and observed key repetition in ordinary Windows applications                                                                                                                                                                                                                                |
| Coordinates             | Verify cursor landing at recorded physical pixels at 100%, 125%, and 150% scaling and on a second monitor with negative coordinates under an unchanged display layout                                                                                                                                                                                                                                                                            |
| Basic dragging          | Record and replay a drag-and-drop, text selection, slider adjustment, and window movement/resize; preserve movement timing, the drop/release pixel, left/right button state, and modifier transitions where the target application supports them                                                                                                                                                                                                 |
| Movement sampling       | Coalescing preserves first/final positions and button/key/wheel ordering; verify a curved drag and a drag with a pause; moving with no button held creates no movement-path events                                                                                                                                                                                                                                                               |
| Global Record           | Start recording with the proposed F9 shortcut while another application has focus; preserve that focus and use the same validation as the Record button. Holding or pressing it again during capture creates no additional session; a registration conflict is explained                                                                                                                                                                         |
| Control exclusion       | Record and Stop controls and reserved F9/F8 key sequences, including repeats and releases, are absent from saved input; playback cannot trigger MacroLoom controls through recorded mouse targets                                                                                                                                                                                                                                                |
| Automatic save          | Stop creates one committed file and a selected list entry; immediate subsequent recordings receive distinct default names/IDs; an empty recording saves nothing                                                                                                                                                                                                                                                                                  |
| Timing                  | Verify deadline scaling and tie ordering deterministically with a controllable clock; measure dispatch lateness on Windows and confirm processing overhead does not accumulate into schedule drift                                                                                                                                                                                                                                               |
| Playback startup        | Play and double click start a session without an added three-second delay; preserve any recorded initial wait and the configured intervals between runs                                                                                                                                                                                                                                                                                          |
| Speed and repetition    | Validate each speed preset, exactly three runs for a count of 3, indefinite playback until Stop, preserved trailing duration, and an unscaled interval with no interval after the final run                                                                                                                                                                                                                                                      |
| Stop and cleanup        | Stop during a long event wait, key hold, active drag, and repeat interval, including via the global shortcut while another application has focus; under normal desktop conditions, target cancellation response within 100 ms and prevent further ordinary input dispatch after cleanup. Stopping recording mid-drag must still produce playback that releases its held button by the end of the run                                             |
| Compact view and layout | Toolbar → message banner → macro list in the full view. Recording, playback, repeat waits, and cleanup hide the list and shrink the window to the toolbar/banner. Restore the full view, prior size/position, selection, and scroll position on stopping/completion/error; saving uses the full view. Check collisions against the compact window's actual visible bounds                                                                        |
| Focus and status        | Only the compact active-session view stays on top; entering/restoring views and progress updates preserve target focus; run number and elapsed time match the defined semantics                                                                                                                                                                                                                                                                  |
| Recorded focus click    | Record a click into an external application's input field followed by typing. Starting with Play or double click reproduces the click and sends the typing to that field while the compact toolbar remains visible. Confirm this with several instances of the same application open. No global Play shortcut or automatic previous-window focus restoration is required in v1                                                                   |
| Shared input            | General system-input playback uses the shared Windows cursor; physical input remains enabled, with the reserved Stop shortcut as the control exception. Do not claim independent manual computer use for this mode                                                                                                                                                                                                                               |
| Responsiveness          | Measure visible interaction response and cold launch against the section 10 budgets; repeat interaction checks while recording/playback workers and persistence are active                                                                                                                                                                                                                                                                       |
| Configuration           | Save valid changes and verify them after restart; Cancel preserves old values; invalid fields are explained; failed persistence preserves the prior file and editable draft                                                                                                                                                                                                                                                                      |
| Deletion                | Cancel preserves the file and entry; successful Delete removes both; simulated deletion failure preserves the entry and shows an error                                                                                                                                                                                                                                                                                                           |
| Outcome toasts          | Successful and failed recording saves, property saves, and deletes each show an action-specific toast after the outcome is known. No false success on cancellation/failure; successful Retry reports success. Toasts dismiss automatically, preserve focus and access to Stop, and announce accessible text; failures retain their actionable messages/recovery controls after dismissal                                                         |
| Storage failure         | Unwritable storage prevents recording start; a mid-save failure retains the recording for Retry/Discard and does not corrupt an existing valid file                                                                                                                                                                                                                                                                                              |
| Lifecycle               | Closing while recording saves or retains a failed save; closing while playing releases session-owned input; suspend/resume produces no burst of overdue playback                                                                                                                                                                                                                                                                                 |
| Packaging               | Extract the portable ZIP and launch its executable on a clean Windows 11 x64 environment with the documented WebView2 dependency, without installing programming languages or development tools. Verify all required resources are included and macro storage resolves beside the executable regardless of the launch working directory. Normal operation requires no VM software, enabled hypervisor feature, or firmware virtualization change |

The 100 ms Stop response is a proposed usability target, not a hard real-time guarantee. Separate deterministic engine validation from Windows integration checks and manual target-application checks. No tests are required for this PRD itself; the criteria govern the future implementation.

## 12. Ticket planning and implementation sequence

### 12.1 Convert the PRD into Epics and Stories first

Before implementation, use `/to-tickets` to turn this reviewed PRD into a backlog of GitHub issues in `Dzahc/MacroLoom`, following `docs/agents/issue-tracker.md`. Review the proposed ticket breakdown and blocking relationships with the user before publishing. Publishing tickets does not assign agents or start implementation.

Each feature is an **Epic** issue containing **Story** issues for the implementation work. Supporting foundation, integration, and release work also belongs in an Epic. Stories should be bounded, end-to-end slices that can be completed and verified once their blockers are complete. Create one GitHub issue per Epic or Story; that issue is its canonical record. GitHub assigns each issue a unique number. After creation, prefix its title with `ML-<GitHub issue number>`; use the GitHub issue number for parent and blocker links, and do not maintain a separate ticket-number sequence.

Each ticket must have a brief name, a detailed summary of the outcome, and explicit Acceptance Criteria. Stories identify their parent Epic by GitHub issue number, cite the relevant PRD sections, and link to prerequisite Story issues. Publish blockers before the Stories they block; use GitHub's blocking relationships when available, or `Blocked by` issue references in the body. Include relevant failure and recovery behavior, and record validation evidence when work is completed. Epic Acceptance Criteria describe the complete feature outcome; Story Acceptance Criteria describe the work that contributes to it. Keep deferred-release work distinct from the v1 backlog.

Apply the `ready-for-agent` label to fully specified Story issues, using the mapping in `docs/agents/triage-labels.md`. The label means the Story is specified; its blockers still gate implementation. Epic issues organize the work and are not implementation assignments. The user manually assigns one or more agents to Stories; creating an issue does not assign anyone.

Track progress in GitHub: **Todo** is an open, unassigned Story; **In Progress** is an open, assigned Story with an implementation branch; **In Review** is an open Story with a linked PR under review; **Done** is a closed Story whose Acceptance Criteria have been verified. Blocked Stories remain in Todo until their prerequisites are complete. Use GitHub issue state, assignees, and linked PRs instead of a separate `Status` field in the issue body.

GitHub issue title after creation: `ML-<GitHub issue number> — <brief ticket name>`. Use this body template, omitting sections that do not apply to an Epic:

```markdown
Type: Epic | Story
Release: v1 | v2 | Later
PRD references: <relevant sections>

## Parent Epic

#<Epic issue number> (Stories only)

## What to build

Describe the end-to-end behavior, scope, and relevant error handling.
For an Epic, describe the complete feature outcome.

## Acceptance Criteria

- [ ] Observable, verifiable outcomes covering the ticket's scope.
- [ ] Applicable failure/recovery and interaction behavior.

## Blocked by

- #<prerequisite Story issue number> (one per blocker), or None (can start immediately).

## Validation

Record the checks performed and evidence for the completed Acceptance Criteria.

## Implementation branch

For Stories: feature/ML-<GitHub issue number>-<name-slug>
```

### 12.2 Git workflow for implementation Stories

Each Story being implemented starts from `develop` on a dedicated branch named `feature/ML-<GitHub issue number>-<name-slug>`. Use the issue's brief name as a lowercase, hyphen-separated slug; for example, Story `#12` titled `ML-12 — Compact session view` uses `feature/ML-12-compact-session-view`. Epic issues organize Stories and do not require an implementation branch. The repository currently has a `develop` branch.

Create the Story branch when implementation begins after the user's assignment. Agents working on the same Story coordinate their changes on that Story's branch; separate Stories use separate branches. Reference the GitHub issue number in commit descriptions and record validation results in the Story issue or linked PR. Integrate prerequisite Stories into `develop` before starting dependent Stories, and integrate completed changes back into `develop` after verifying their Acceptance Criteria. Do not implement Story work directly on `develop`.

### 12.3 Suggested feature implementation order

| Order | Epic / feature area                               | Suggested Story breakdown and dependency rationale                                                                                                                                                                                                         |
| ----- | ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Application foundation and Windows feasibility    | Scaffold the selected desktop shell; prototype native capture/injection, timing/cancellation, focus handling, and compact-view transitions; measure responsiveness. Validate the accepted stack and basic dragging before broader implementation           |
| 2     | Macro storage and library                         | Finalize the versioned format; implement executable-relative storage, validation, atomic writes, startup loading, list selection, and invalid-file handling. Later save/configure/delete features depend on this repository                                |
| 3     | Toolbar, message banner, compact view, and toasts | Implement the toolbar → banner → list layout, backend-driven UI states, actual window shrinking/restoration, accessible status, and reusable outcome toasts. Exercise state transitions with controlled session events before connecting live input        |
| 4     | Global shortcuts and session lifecycle            | Implement Record/Stop registration, conflict feedback, one-session enforcement, focus preservation, cancellation, and held-input cleanup. Establish Stop behavior before end-to-end playback                                                               |
| 5     | Recording and automatic save                      | Capture keyboard and positioned mouse events, held-button movement and sampling, control exclusions, unique naming, automatic persistence, and Retry/Discard with outcome toasts. Integrate storage, shortcuts, and compact recording status               |
| 6     | Single-run playback                               | Implement session snapshots, immediate startup, monotonic deadlines, coordinate conversion, input injection, drag replay, collision checks, cancellation/error handling, and compact playback status. Validate real record → save → replay workflows at 1× |
| 7     | Macro configuration and repeat playback           | Build property editing/persistence and toasts; add speed scaling, finite/infinite repetition, unscaled intervals, and run/elapsed status. Extend validated single-run playback                                                                             |
| 8     | Macro deletion                                    | Implement confirmation, file/list removal, failure preservation, and outcome toasts. This can be assigned earlier once storage, selection, and notifications are integrated                                                                                |
| 9     | Release validation and portable ZIP               | Validate DPI/display layouts, focus, input cleanup, suspend/exit, failures, accessibility, and performance; package the executable/resources and verify on a clean Windows 11 environment                                                                  |

This order is a recommendation for Story dependencies and integration. The user chooses assignments; independent Stories may proceed concurrently once their shared dependencies are available on `develop`.

## 13. Remaining technical decisions

Basic dragging is now included in the initial-release scope as a small extension of the button/movement event pipeline. Its effort and sampling behavior should be checked with the first Windows integration slice; if that reveals substantial work beyond the scope described here, revisit the release boundary rather than silently dropping the feature or promising precision path replay.

**Independent computer use during playback** is deferred from v1 by user decision. The first release uses normal desktop mouse/keyboard input, includes basic dragging, and has no ghost-pointer or virtual-machine integration requirement.

V1 distribution is a **portable ZIP**, with the originally requested app-adjacent `macros` folder. Fixed global Record and Stop shortcuts are included, and playback has no added startup countdown. An optional three-second startup countdown and a setup executable are deferred to later releases.

The user accepted the stack, with prototype validation as the first implementation work. Visual treatment, speed presets, x64 packaging, and F9/F8 as the exact Record/Stop key assignments remain proposed defaults that can be revised during review. The focus workflow is settled: record a target-window click before typing; global Play and automatic previous-window focus restoration are outside v1. Validate the recorded-click behavior in the first integration slice.

Coordinate targeting is settled for v1: use absolute physical screen coordinates, with the target window kept at its recorded position and size. Window-relative mode and its playback-window selector are deferred to v2 by user decision. The v1 prototype validates the absolute-coordinate workflow, including recorded focus clicks with several instances of the same application open.
