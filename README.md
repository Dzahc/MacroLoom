# MacroLoom

MacroLoom is a Windows desktop application for recording mouse and keyboard actions across ordinary applications and replaying them as reusable macros. The intended workflow is to record a sequence, save it locally, then select and play it whenever the task needs repeating. The planned library also supports naming, playback speed, repetition, and deletion.

The initial release targets Windows 11. See the [product requirements](docs/agents/PRD.md) for the feature scope and design decisions.

## Project layout

- `app/` contains the Tauri/Rust backend and React/TypeScript frontend.
- `docs/` contains product, engineering, and validation documents.

## Develop on Windows 11 x64

Install Node.js and the [Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/) (Rust with the MSVC target, Microsoft C++ Build Tools, and WebView2). From the repository root, run:

```powershell
cd app
npm.cmd install
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
npm.cmd run tauri -- dev
```

To build an executable with the frontend embedded, run from `app/`:

```powershell
npm.cmd run tauri -- build --no-bundle
```

The executable is written to `app/src-tauri/target/release/macroloom.exe`, relative to the repository root.
