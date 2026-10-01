# ML-8 validation — Windows input prototype

## Build and test setup

Tested September 30, 2026 on Windows 11 Home x64, build 10.0.26200, with an Intel Core i7-11700F (16 logical processors) and 31.9 GiB RAM. The available setup had one 2560 × 1440 logical display at 150% scaling, giving a 3840 × 2160 physical virtual desktop. MacroLoom and Notepad ran at the same privilege level. A PowerShell Win32 input probe supplied repeatable input; the Notepad editor and MacroLoom status were read through Windows UI Automation.

`npm.cmd run build`, `cargo check`, `cargo test`, `cargo build`, and `npm.cmd run tauri -- build --no-bundle` passed. Run npm commands from `app/` and Cargo commands from `app/src-tauri/`; the implementation moved into `app/` after this validation. The production build yielded a runnable x64 executable with the frontend embedded. The native test covers conversion of signed virtual-desktop pixel coordinates to the absolute `SendInput` range. Tauri's debug executable requires Vite at `127.0.0.1:1420`.

## Observations

| Check | Observed result |
| --- | --- |
| Global start and stop | F9 began recording while Notepad retained foreground focus. F8 stopped without changing that focus. The compact window returned to its prior bounds. |
| Key and positioned click replay | A 0.4 s, 8-event take changed Notepad text from `ML8 probeQ` to `ML8 probeQQ` after replay. The recorded click preceded the key input. |
| Held-button drag and release pixel | A 0.5 s, 13-event take included 5 sampled drag moves. The recorded release was `(2928, 510)` physical pixels; the cursor observed immediately after replay release was also `(2928, 510)`. Notepad text changed from `R` to `RR` on replay. |
| Wait cancellation | F8 cancelled a replay during a long initial wait. Backend response from Stop recognition through held-input cleanup was 1 ms. |
| Active-drag cancellation | A 2.3 s, 22-event take included 20 drag moves. During replay, Windows reported the left button down; after F8 it reported up. Backend Stop response was 2 ms. |
| Capture spacing | The smallest observed gap between captured hook callbacks was 1 ms in the drag take. This is an observed gap rounded to milliseconds, not a clock-accuracy claim. |
| Dispatch lateness | The short key/click take reported p95/max 0/0 ms. The drag and active-drag takes reported 5/5 ms. Lateness is measured immediately before dispatch against monotonic `Instant` deadlines and rounded to milliseconds. |
| Visible UI response | The frontend's click-to-next-animation-frame samples included 25, 27, 39, and 67 ms. These include command and render time and are individual observations, not a 95th-percentile estimate. |
| Production-mode startup | Five launches reached an enabled Record control in 1338, 709, 806, 774, and 742 ms. The first was the coldest observed process start; the other four benefited from OS/WebView caching. This prototype has no saved library to load. |

## Stack decision and limits

The Tauri 2 + React/TypeScript + Rust stack produced a responsive Windows shell and working native capture/replay. Keep it as the foundation for S2. This decision remains provisional for release-scale performance and display coverage.

The available machine exercised one display at 150% scaling. The 100% and 125% cases, a second monitor with negative coordinates, a clean-machine build, and a wider range of ordinary applications remain for S11. The input probe generated repeatable Windows input; a separate hand-operated pass would add confidence for varied hardware. The prototype keeps takes only in memory and does not implement the final library, persistence, property editing, or release lifecycle handling.
