# MacroLoom

ML-8 is a Windows input feasibility prototype. It records keyboard input, positioned left/right clicks, and held-button movement from another desktop application, then replays the take from memory. The shell shrinks and stays on top during a session without activating itself over the target.

The runnable desktop project lives in `app/` (React/TypeScript frontend and Tauri/Rust backend). Repository planning and validation documents live in `docs/`.

## Run on Windows 11 x64

Install Node.js and the [Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/) (Rust with the MSVC target, Microsoft C++ Build Tools, and WebView2). Then:

```powershell
cd app
npm.cmd install
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
npm.cmd run tauri -- dev
```

To build an executable with the frontend embedded:

```powershell
npm.cmd run tauri -- build --no-bundle
```

From the repository root, the result is `app/src-tauri/target/release/macroloom.exe`. This prototype does not save recordings or package a portable ZIP.

## Try a take

1. Open a normal, same-privilege target application and put its window at the desired screen position.
2. Press **F9** while that application has focus, or click **Record** in MacroLoom.
3. Click the target's input field before typing. Left/right clicks and held-button moves are recorded with physical screen positions.
4. Press **F8** to stop. The take remains in memory.
5. Keep the target window at its original position and click **Play**. Press **F8** to cancel, including during a drag or a wait.

The prototype uses the shared Windows cursor and keyboard focus. Closing the application discards the take. See [ML-8 validation](docs/prototype/ML-8-validation.md) for measurements and remaining checks.
