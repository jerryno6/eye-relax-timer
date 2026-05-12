# Eye Relax Timer — Development Guide

[← Back to main README](README.md)

## Tech Stack

- Tauri v2 for the desktop shell, tray integration, windows, persistence, and native app build.
- Rust for timer state, countdown lifecycle, tray menu updates, break popup geometry, autostart, and tests.
- React 18 and TypeScript for the settings window and break popup UI.
- Vite for frontend development and production builds.
- lucide-react for UI icons.

## Project Structure

- `src/` contains the React frontend.
- `src/App.tsx` renders either the settings window or break popup based on the window query string.
- `src-tauri/src/lib.rs` contains the app state, timer commands, tray menu, popup window lifecycle, settings persistence, autostart integration, and Rust tests.
- `src-tauri/tauri.conf.json` contains the Tauri v2 app and bundle configuration.
- `screenshots/` contains the images used in this README.

## Getting Started

### Prerequisites

- Node.js and npm.
- Rust and Cargo.
- Tauri platform prerequisites for your operating system.

### Install Dependencies

```bash
npm install
```

### Run the Frontend Only

```bash
npm run dev
```

This starts Vite at `http://127.0.0.1:1420`.

### Run the Desktop App

```bash
npm run tauri:dev
```

This starts the Tauri app with the Vite frontend.

## Build

### Build the Frontend

```bash
npm run build
```

### Build the Tauri App

```bash
npm run tauri:build
```

The Tauri build creates platform-specific desktop bundles under `src-tauri/target`.

## Test

Run the Rust test suite:

```bash
cd src-tauri
cargo test
```

There is no frontend test script configured currently. Use `npm run build` to type-check and build the React/Vite frontend.

## Version Update & Release

`package.json` is the single source of truth for the app version. A sync script keeps `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` in sync automatically during build.

### Update the version

1. Change `"version"` in `package.json` only.
2. Commit and open a PR to `develop`.

```
"version": "0.2.0"
```

### CI/CD Pipeline

| Event | Workflow | What happens |
| --- | --- | --- |
| PR opened to `develop` | `ci.yml` | Build + typecheck frontend, cargo check + test |
| PR merged to `main` | `tag.yml` | Reads version from `package.json`, creates and pushes tag `v{x.y.z}` |
| Tag `v*` pushed | `release.yml` | Builds artifacts for macOS (ARM/x64) and Windows, creates GitHub Release |

### Full release flow

```
Edit version in package.json
     │
     ▼
Open PR to develop → CI runs build + check
     │
     ▼
Merge PR to develop → then open PR to main
     │
     ▼
Merge PR to main → tag.yml creates tag v0.2.0
     │
     ▼
Tag push triggers release.yml → builds artifacts → GitHub Release
```
