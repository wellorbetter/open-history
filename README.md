# OpenHistory

OpenHistory is an open, local-first desktop activity history for macOS and Windows. It turns
consented semantic accessibility events into deterministic, resumable work context without screen
recording, audio recording, or raw-key logging.

The compact menu-bar/tray surface shows current work and a daily timeline. A full window provides
review, search, correction, export, deletion, privacy settings, and separately approved read-only
agent access.

## Status

Private alpha implementation. The React fixture UI and Rust domain core are under active OpenSpec
development in `openspec/changes/build-open-history-desktop`.

## Start

```bash
npm ci
npm run dev
```

See [local development](docs/development.md), [architecture](docs/architecture.md),
[privacy](docs/privacy.md), and [contributing](CONTRIBUTING.md).
