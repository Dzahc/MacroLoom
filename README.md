# MacroLoom

MacroLoom is a Windows desktop application for recording mouse and keyboard actions across ordinary applications and replaying them as reusable macros. The intended workflow is to record a sequence, save it locally, then select and play it whenever the task needs repeating. The planned library also supports naming, playback speed, repetition, and deletion.

The initial release targets Windows 11. See the [product requirements](docs/agents/PRD.md) for the feature scope and design decisions.

## Project layout

- `app/` contains the Tauri/Rust backend and React/TypeScript frontend.
- `docs/` contains product, engineering, and validation documents.

## Develop on Windows 11 x64

Follow the [pinned toolchain and dependency setup](docs/agents/quality-gate.md#reproducible-setup), including the [Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/) (Rust with the MSVC target, Microsoft C++ Build Tools, and WebView2). Install frontend dependencies from the repository root:

```powershell
npm.cmd --prefix app ci
```

**Recommended: run the full desktop app** from the repository root:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
npm.cmd --prefix app run tauri -- dev
```

This starts Vite, compiles the Rust backend, and opens the Tauri application
window. From `app/`, the equivalent command is `npm.cmd run tauri -- dev`.

For a **browser-only frontend preview**, run from the repository root:

```powershell
npm.cmd --prefix app run dev
```

Open `http://127.0.0.1:1420` in your browser. This starts Vite only; native backend
features require the full desktop app.

To build an executable with the frontend embedded, run from `app/`:

```powershell
npm.cmd run tauri -- build --no-bundle
```

The executable is written to `app/src-tauri/target/release/macroloom.exe`, relative to the repository root.

## Saved macro library (ML-13)

The desktop window loads version-one JSON macros from `macros` beside the running
executable, independently of the launch working directory. Valid entries appear
progressively, newest first, with stable-ID selection. Invalid files remain
untouched and produce individually dismissed toasts naming the file and failing
field or part. Temporary `.json.tmp` files are ignored. Event data loads on demand;
external file changes require a restart before the affected action can proceed.

Three example JSON files are available in [docs/samples/macros](docs/samples/macros).
To restore them to the default development executable's library after cleaning the
build directory, run from the repository root:

```powershell
New-Item -ItemType Directory -Force app/src-tauri/target/debug/macros
Copy-Item docs/samples/macros/*.json app/src-tauri/target/debug/macros
```

The examples display durations of 5, 12, and 24 seconds and cover keyboard input,
dragging, and scrolling. They load as saved data without executing their events.

Configure, Delete, and Play prepare validated snapshots for their later stories;
dialogs, persistence mutations, and native Record/Stop/Play integration remain
outside ML-13. The loader never executes saved input. Browser-only previews retain
the ML-9 controlled samples because they cannot access native storage.

In development, **Open development pane** exposes empty,
populated, saving, and logged load-failure examples, six operation-toast examples,
and a callback log in the browser preview. The development pane is omitted from production builds.

The ML-8 live input prototype remains available explicitly in a debug build:

```powershell
npm.cmd --prefix app run tauri -- dev --features input-prototype
```

This feature starts the prototype's native hooks and F9/F8 shortcuts. Default
builds do not start hooks or register shortcuts, and reject live input commands.
Release builds disable the prototype even if the feature is supplied. See
[ML-9 validation](docs/validation/ML-9-validation.md) for checks and scope decisions.

Before completing a code task, run `npm.cmd --prefix app run verify` from the
repository root. It fetches `origin` and runs the source-preserving quality checks.
See the [quality gate](docs/agents/quality-gate.md) for individual commands,
complexity rules, base overrides, and owner-approved exceptions.
