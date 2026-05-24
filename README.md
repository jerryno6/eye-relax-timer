# Eye Relax Timer

Eye Relax Timer is a tray-first desktop app that helps you rest your eyes on a regular schedule. Start a countdown, get a large break reminder when time is up, then let the app repeat the cycle automatically if repeat mode is enabled.

## Screenshots

![Eye Relax Timer settings overview](screenshots/eye-relax-timer.png)

![Eye Relax Timer duration and behavior settings](screenshots/eye-relax-timer-2.png)

![Eye Relax Timer tray menu](screenshots/eye-relax-timer-3.png)

## Features

- Tray/status bar app designed to stay out of the way.
- Configurable timer presets: 5, 10, 15, 20, 30, 45, and 60 minutes.
- Custom timer duration from 1 to 240 minutes.
- Start, pause/resume, and stop controls from the settings window.
- Tray menu with status, start, pause/resume, stop, settings, and quit actions.
- Eye-rest popup that covers about 80% of the primary screen.
- 60-second break countdown.
- Optional repeat mode to automatically start the next timer after a break.
- Optional autostart at login.
- Settings persistence in the app config directory.

## Plan

- [ ] Display version on the Title of the UI.
- [ ] Auto Update

## Development

### Package manager

Local development may use **bun** (`bun install`, `bun run dev`) for faster installs.
**CI and release builds always use npm** against `package-lock.json`.

Do not commit `bun.lockb`, `pnpm-lock.yaml`, or `yarn.lock` — they are gitignored. When you change dependencies, run `npm install` once so `package-lock.json` stays in sync, then commit it.

[← Back to main README-DEVELOPMENT](README-DEVELOPMENT.md)
