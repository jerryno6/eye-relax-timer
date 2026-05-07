# Guidance for agents/devs

## Project idea

Eye Relax Timer is a tray-first desktop app built with Tauri v2. Its goal is to remind users to rest their eyes on a cycle: run a countdown, show an eye-rest popup covering about 80% of the screen, count down the break time, then automatically repeat if repeat is enabled. The app has a small settings window for choosing a preset, entering a custom duration, and toggling repeat and autostart.

## Overall architecture

- Frontend: React 18 + TypeScript + Vite in `src/`. The UI has two modes based on the query string:
  - `index.html?window=settings`: settings/timer controls window.
  - `index.html?window=break`: eye-rest popup.
- Backend: Rust in `src-tauri/src/lib.rs`, managing timer state, settings, tray/status bar, popup window, autostart, and persistence.
- Tauri bridge: the frontend uses `invoke()` to call Rust commands and `listen("timer-state")` to receive new timer snapshots.
- Main state lives in Rust in `SharedState`:
  - `settings`: `durationMinutes`, `repeatEnabled`, `autostartEnabled`.
  - `timer`: `status`, `remainingSeconds`, `breakRemainingSeconds`.
  - `generation`: cancels old async countdowns when starting, pausing, resetting, or finishing a break.
  - `tray_menu`: handle used to update status/menu items based on the timer.
- Settings are saved in the app config directory under `settings.json`.

## Main file structure

- `package.json`: npm scripts, React/Tauri/lucide dependencies.
- `vite.config.ts`: Vite configuration for Tauri, strict dev port, platform-specific build target.
- `index.html`: HTML shell for the frontend.
- `src/main.tsx`: mounts the React app.
- `src/App.tsx`: settings window, break popup, frontend types, duration presets, timer formatting, Tauri command/event calls.
- `src/styles.css`: styles for the settings window and break popup.
- `src-tauri/Cargo.toml`: Rust crate, dependencies `tauri`, `tauri-plugin-autostart`, `tokio`, `serde`.
- `src-tauri/src/main.rs`: entrypoint that calls the library `run`.
- `src-tauri/src/lib.rs`: main app logic: commands, tray menu, window creation, async countdown, validation, persistence, Rust tests.
- `src-tauri/tauri.conf.json`: Tauri v2 configuration, product name, bundle icon, dev URL, frontend dist.
- `src-tauri/capabilities/default.json`: permissions for the `settings` and `break` windows.
- `src-tauri/icons/icon.png`: bundle/tray icon.
- `src-tauri/gen/schemas/`: generated Tauri schemas.

## Run/build/test commands

First run:

```bash
npm install
```

Run the Vite frontend separately:

```bash
npm run dev
```

Run the Tauri desktop app:

```bash
npm run tauri:dev
```

Build the frontend:

```bash
npm run build
```

Build the Tauri bundle:

```bash
npm run tauri:build
```

Run Rust tests:

```bash
cd src-tauri
cargo test
```

There is currently no frontend test script in `package.json`. When verifying TypeScript/frontend changes, use `npm run build`; when verifying Rust logic, use `cargo test` in `src-tauri`.

## Important development notes

- Only change files related to the task. The repo may already contain changes from other people; do not revert or broadly reformat them.
- When following local instructions, prefix shell commands with `rtk`, because the repo root AGENTS includes `/Users/vule/.codex/RTK.md`.
- The Rust backend is the source of truth for the timer, settings persistence, tray menu, and window lifecycle. Avoid duplicating important countdown logic in the frontend.
- `TimerStatus` is serialized as camelCase, so frontend names must match: `stopped`, `running`, `paused`, `breakVisible`.
- Valid duration is from 1 to 240 minutes. If this range changes, update both Rust validation and the frontend UX.
- Break duration is currently hardcoded in Rust as `BREAK_SECONDS = 60`.
- The break popup is created undecorated, always-on-top, skipped from the taskbar, and sized to 80% of the primary monitor. If changing geometry, test on multi-monitor/scale-factor setups.
- Closing the settings window is intercepted to hide it instead of exiting the app. The app only truly exits through the tray menu `Quit`, when `allow_exit` is set.
- Pause/resume/reset use `generation` to invalidate old countdown tasks. When adding any new async flow, respect the generation mechanism to avoid race conditions.
- Autostart uses `tauri-plugin-autostart` with `MacosLauncher::LaunchAgent`; consider platform behavior when modifying it.
- The Tauri app declares `windows: []` in config and creates windows at runtime in Rust. If adding a new window, update capabilities as needed.
- The frontend does not use global Tauri (`withGlobalTauri: false`); import APIs from `@tauri-apps/api`.
- The UI currently uses `lucide-react` for icon buttons. When adding new controls, keep the style compact and make disabled/error/saved states clear.
