## Why

Computer History is useful because it turns fragmented desktop activity into resumable work context, but the reference product is restricted by platform, account, workspace, and regional availability and cannot be used as a standalone API-key capability. OpenHistory will provide an open, local-only alternative that works as a lightweight macOS menu-bar and Windows system-tray application while exposing the resulting history to approved local AI agents.

## What Changes

- Introduce a cross-platform desktop activity collector for macOS and Windows that records semantic interaction events without screen recording, audio recording, or storage of raw keystrokes.
- Normalize platform-specific accessibility events into a stable, versioned event model.
- Group short-lived events into human-scale work sessions and task segments instead of presenting an unfiltered event stream.
- Provide deterministic non-AI titles and summaries, with optional pluggable AI summarization that uses an on-device model only.
- Add a compact macOS menu-bar popover and Windows tray flyout inspired by the supplied WeekLens reference: current activity, recording state, daily timeline, source applications, navigation, and review entry point.
- Add a full history window for reviewed summaries, source context, corrections, search, export, and deletion.
- Add explicit consent, pause/resume, per-app and per-website exclusions, retention controls, and clear indicators whenever collection is active.
- Keep capture, storage, indexing, search, and summarization on device, retain raw events for a configurable period that defaults to 48 hours, and keep derived summaries until the user deletes them.
- Expose an authenticated loopback API and read-only MCP server so Codex and other agents can query approved history without depending on an OpenAI-specific account or region.
- Support import adapters for compatible external activity sources, while keeping the native collector and canonical schema independent of ActivityWatch or screenpipe.
- Exclude cloud processing or synchronization, telemetry, remote history APIs, mobile clients, screenshots, OCR, microphone capture, system-audio capture, and autonomous action execution from the first release.

## Capabilities

### New Capabilities

- `semantic-activity-capture`: Consent-based macOS and Windows collection, normalization, idle detection, source attribution, and resilient operation.
- `privacy-and-retention`: Collection visibility, exclusions, local protection, redaction, deletion, and retention behavior.
- `task-segmentation`: Deterministic grouping of normalized events into sessions and task segments with provenance and correction support.
- `summary-generation`: Non-AI fallback summaries plus optional, transparent on-device AI enrichment.
- `desktop-timeline`: Menu-bar/tray surfaces and a full history window for reviewing current and historical activity.
- `agent-history-access`: Authenticated local API, read-only MCP tools, query scoping, and prompt-injection-safe data boundaries.
- `history-interchange`: Versioned Markdown/JSON export and optional import adapters for compatible external sources.

### Modified Capabilities

None. This is a greenfield project.

## Impact

- Creates a new Tauri 2 desktop project with a Rust core, a web-based presentation layer, and platform adapters implemented with macOS Accessibility APIs and Windows UI Automation APIs.
- Requires macOS Accessibility permission and equivalent Windows UI Automation access, with browser URL enrichment provided only through a separately consented extension or adapter.
- Adds local SQLite storage, operating-system keychain integration for secrets, an optional on-device summarizer interface, and loopback-only HTTP/MCP endpoints.
- Introduces privacy-sensitive test fixtures, platform-specific integration tests, accessibility checks, retention/deletion tests, and macOS/Windows packaging workflows.
- Establishes `wellorbetter/open-history` as the private planning and implementation repository.
