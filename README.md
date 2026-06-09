# Odysseus Mac App

A native macOS desktop wrapper for [Odysseus](https://github.com/mrmps/odysseus), built with [Tauri v2](https://tauri.app).

Odysseus is a self-hosted AI assistant that runs a local Python/FastAPI server. This app gives it a proper macOS home: it starts the server in the background on launch, shows a loading screen while it warms up, then opens the full Odysseus UI in a native window. Closing the window stops the server cleanly.

## What it does

- Launches the Odysseus uvicorn server automatically on app open
- Shows an animated loading screen while the server starts
- Navigates the native window to the Odysseus UI once it is ready
- Kills the server on close (window button, Cmd+Q, or SIGTERM)
- Cleans up any stale server process left over from a previous crash on the next launch

## Prerequisites

You need the following installed before building:

- **Rust** — [rustup.rs](https://rustup.rs)
- **Node.js** and **npm** — [nodejs.org](https://nodejs.org)
- **Odysseus** — cloned and set up via its own `./start-macos.sh` at least once (this creates the Python venv and installs dependencies)

## Setup

Clone this repo next to your Odysseus directory (or anywhere — see Configuration below):

```bash
git clone https://github.com/1xxalexx1/odysseus-mac-app
cd odysseus-mac-app
npm install
```

## Configuration

By default the app expects Odysseus at `$HOME/odysseus`. If your Odysseus repo lives somewhere else, set the `ODYSSEUS_DIR` environment variable before launching or building:

```bash
export ODYSSEUS_DIR=/path/to/your/odysseus
```

The app resolves:
- **Odysseus directory** → `$ODYSSEUS_DIR` or `$HOME/odysseus`
- **uvicorn binary** → `$ODYSSEUS_DIR/venv/bin/uvicorn`
- **Server port** → `7860` (hardcoded; note that macOS AirPlay Receiver occupies port 7000)

## Building

```bash
npm run tauri -- build
```

The `.app` bundle is produced at:

```
src-tauri/target/release/bundle/macos/Odysseus.app
```

You can drag it to `/Applications` and launch it like any other Mac app.

## Development

To run in dev mode (with hot-reload of the loading screen):

```bash
npm run tauri -- dev
```

## Project structure

```
.
├── src/
│   └── index.html          # Loading and error screen shown before the server is ready
└── src-tauri/
    ├── src/
    │   ├── main.rs         # Entry point
    │   └── lib.rs          # Server lifecycle: spawn, poll, navigate, kill
    ├── capabilities/
    │   └── default.json    # Tauri v2 permission set for the main window
    ├── icons/              # App icons (icns + png sizes)
    └── tauri.conf.json     # Window, bundle, and security configuration
```

## How it works

1. On launch, the Rust backend spawns `venv/bin/uvicorn app:app --host 127.0.0.1 --port 7860` from the Odysseus directory.
2. It polls the TCP port every second (up to 90 seconds). If the process exits early, an error is shown immediately.
3. Once the port accepts connections, a short grace period allows the ASGI stack to finish wiring up, then the webview navigates to `http://127.0.0.1:7860`.
4. On window close or SIGTERM, the uvicorn process is killed and waited on before the app exits.

## License

MIT
