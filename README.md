# MacroLoom

MacroLoom is a Windows desktop application for recording mouse and keyboard actions across ordinary applications and replaying them as reusable macros. The intended workflow is to record a sequence, save it locally, then select and play it whenever the task needs repeating. The planned library also supports naming, playback speed, repetition, and deletion.

The initial release targets Windows 11. See the [product requirements](docs/agents/PRD.md) for the feature scope and design decisions.

## Project layout

- `app/` contains the Tauri/Rust backend and React/TypeScript frontend.
- `docs/` contains product, engineering, and validation documents.

## Develop on Windows 11 x64

Follow the [pinned toolchain and dependency setup](docs/agents/quality-gate.md#reproducible-setup), including the [Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/) (Rust with the MSVC target, Microsoft C++ Build Tools, and WebView2). From the repository root, run:

```powershell
cd app
npm.cmd ci
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
npm.cmd run tauri -- dev
```

To build an executable with the frontend embedded, run from `app/`:

```powershell
npm.cmd run tauri -- build --no-bundle
```

The executable is written to `app/src-tauri/target/release/macroloom.exe`, relative to the repository root.

Before completing a code task, run `npm.cmd --prefix app run verify` from the
repository root. It fetches `origin` and runs the source-preserving quality checks.
See the [quality gate](docs/agents/quality-gate.md) for individual commands,
complexity rules, base overrides, and owner-approved exceptions.
