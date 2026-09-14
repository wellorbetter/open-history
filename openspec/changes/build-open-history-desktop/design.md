## Context

This is a greenfield project; no source implementation is available. The supplied WeekLens screenshot is the visual and interaction reference for the compact macOS surface. See `proposal.md` for motivation and `specs/` for observable behavior.

The design must bridge two different accessibility stacks. macOS exposes application accessibility objects and notifications through AXUIElement/AXObserver after explicit Accessibility permission. Windows exposes foreground/window events and semantic control data through WinEvent hooks and UI Automation. Both platforms contain applications that expose incomplete or inconsistent accessibility trees, so the system must represent confidence and missing data rather than assume parity.

The desktop process is long-lived but visually quiet. Collection continues while its main window is closed, the compact surface must open quickly from the menu bar or tray, and all core behavior must remain useful without a model. Activity processing never requires outbound network access, and captured content is sensitive and potentially adversarial.

## Goals / Non-Goals

**Goals:**

- Keep platform-specific collection behind one canonical Rust domain model.
- Make the compact surface feel native on macOS and familiar on Windows without maintaining two complete UI codebases.
- Keep task boundaries deterministic and inspectable; reserve on-device AI for optional wording enrichment.
- Minimize persistent sensitive data and make every collection and local-processing state visible and reversible.
- Provide a stable local integration boundary for the first-party UI, browser adapters, exports, and AI agents.
- Keep an idle background build within a target of 2% average CPU and 150 MB working memory on representative supported hardware, subject to platform integration measurement.

**Non-Goals:**

- Reproduce undocumented OpenAI storage formats or private APIs.
- Achieve semantic parity in applications that do not expose accessibility information.
- Use global keyboard hooks to reconstruct typed text.
- Let an agent execute desktop actions through the history service.
- Support remote network clients, account-based cloud sync, team administration, or mobile clients in V1.
- Transmit activity data, summaries, diagnostics, or telemetry to a remote service.

## Decisions

### 1. Use Tauri 2 with a Rust workspace and a React/TypeScript presentation layer

The repository will contain a Tauri desktop package plus reusable Rust crates for domain types, storage, segmentation, summarization, platform adapters, and agent access. React/TypeScript renders the compact and full-window routes from one tokenized component system.

Tauri provides native tray integration, tray-relative window positioning, multiple webview windows, packaging, and a narrow command permission model while keeping the long-running domain logic in Rust. It also avoids the larger resident runtime and native menu-bar bridging required by Flutter for this particular utility.

Alternatives considered:

- **Flutter + Rust:** strong cross-platform layout productivity and aligned with the user's existing stack, but adds a second FFI boundary and less direct tray-popover behavior.
- **SwiftUI + WinUI:** best native fidelity, but duplicates the entire presentation and state-management layer.
- **Electron:** mature tray APIs, but a heavier baseline is a poor fit for an always-running history collector.

### 2. Ship one background-capable desktop application before introducing a separate daemon

The Rust core lives for the lifetime of the tray application. Webview windows are created on demand and may close without stopping collection. Single-instance startup, autostart, panic isolation around adapters, and bounded queues provide V1 resilience.

A separate OS service would improve crash isolation, but it substantially complicates installation, permission ownership, upgrades, and debugging. Core interfaces will avoid direct UI dependencies so the collector can be moved to a service later without changing specs.

### 3. Implement narrow platform adapters that discard forbidden data at the boundary

The macOS adapter uses workspace activation notifications and AXObserver/AXUIElement notifications. The Windows adapter uses foreground WinEvents and UI Automation event handlers. Adapter output enters a bounded normalization channel.

Adapters emit action categories and permitted accessibility context, never raw key sequences. Shortcut activity may be represented only when the platform exposes a command or accelerator semantically; the product will not infer characters from low-level keyboard events. Screenshots and audio have no capture path in V1.

A separately consented Chromium-family extension enriches browser events with active-tab URL, title, and private-mode state through a loopback adapter endpoint. Safari and unsupported browsers contribute application/window semantics only in V1. The canonical core has no dependency on any browser vendor.

### 4. Use a versioned event envelope with privacy and quality metadata

The canonical envelope contains:

- `event_id`, `schema_version`, `occurred_at`, `timezone_offset`, and monotonic ordering data
- device-local `source_id`, adapter type, application identity, and optional window/document identity
- semantic event kind and a typed payload
- privacy classification, capture-quality level, and redaction flags
- correlation identifiers for imported or browser-enriched events

Payloads are minimized before entering persistence. An exclusion policy engine runs between adapter normalization and the durable event writer. Policy outcomes are auditable without preserving the discarded content or excluded source identity.

### 5. Separate deterministic segmentation from optional on-device AI summarization

An online sessionizer maintains the current projection used by the compact card. A reconciliation job reruns the same deterministic algorithm when late events arrive or settings change.

The initial algorithm uses:

- an idle boundary of five minutes by default;
- strong boundaries for sleep, lock, shutdown, and explicit project/document changes;
- a continuity score derived from project/document identity, normalized window tokens, recent source transitions, and time distance;
- lower confidence across private gaps or low-quality sources;
- stable segment IDs derived from ordered source-event IDs and segmentation-policy version.

On-device AI output can rename or summarize a segment but cannot silently alter its time boundaries. User split/merge operations create derived revisions linked to immutable source events.

### 6. Store local history in an encrypted SQLite database

The core uses SQLite in WAL mode with schema migrations and full-text indexing over permitted derived summaries. The database is encrypted using SQLCipher; its randomly generated key is stored in macOS Keychain or Windows Credential Manager. Local-client credentials use the same OS credential facilities and never enter the database in plaintext.

Principal records are `sources`, `raw_events`, `sessions`, `task_segments`, `segment_event_links`, `summary_revisions`, `privacy_policies`, `approved_clients`, `imports`, and `schema_migrations`. A retention worker deletes expired raw rows and related indexes transactionally. User-facing deletion traverses all derived links in one transaction and queues secure cleanup of managed temporary exports.

Plain SQLite relying only on FileVault or BitLocker was rejected because the application must not assume full-disk encryption is enabled. Per-field encryption was rejected for V1 because it complicates migrations and search while leaving structural metadata exposed.

### 7. Provide deterministic summaries first and on-device AI enrichment second

The deterministic formatter uses task boundaries, dominant project/document labels, action categories, and source transitions to create a title, outline, duration, and source list immediately.

The summarizer interface accepts a minimized typed record and returns validated structured output from an on-device engine. No model is required or enabled by default. The product does not accept remote endpoint configuration and does not include an outbound activity-data transport. The UI identifies model-generated revisions and reports which local engine created them.

Captured text is delimited as untrusted evidence, stripped of control characters, size limited, and never concatenated into system or tool instructions. Output is checked for unsupported entities and preserved as a revision beside the deterministic fallback.

### 8. Expose loopback HTTP for adapters and a read-only stdio MCP companion

The desktop core exposes a versioned HTTP API on a random loopback port for the first-party UI, browser adapter, diagnostics, and approved local integrations. Authentication uses rotatable per-client bearer credentials with scopes. Origin checks, request-size limits, rate limits, and no wildcard CORS reduce local cross-origin attacks.

The `openhistory-mcp` companion communicates with MCP clients over stdio and with the running core through the authenticated local channel. It exposes bounded read-only tools/resources for search, recent tasks, task details, daily recap, and sources. It does not expose mutation or raw-event operations in V1. Returned source-derived strings carry provenance and an explicit untrusted-content marker.

An unauthenticated fixed local port was rejected because any browser page or local process could probe history. A remote HTTP server was rejected because it would require a substantially different identity, authorization, and threat model.

### 9. Treat the compact surface as a functional glass layer over stable content

The supplied reference establishes the hierarchy: identity and recording state, current activity card, selected-day heading/navigation, then a dense vertical timeline. OpenHistory preserves that structure while reducing decorative blur inside content.

- **macOS:** a tray-anchored, borderless popover approximately 360 points wide and up to 520 points high, clamped to the active display work area.
- **Windows:** a tray-adjacent flyout approximately 380 by 560 logical pixels, repositioned for taskbar edge and display scaling.
- **Shared:** a fixed top control region; current card; scrollable timeline; no editing controls or complex settings inside the compact surface.
- **Full window:** a resizable history workspace, initially 960 by 680 logical pixels with a 760 by 520 minimum, containing date navigation, timeline/search results, and a task detail inspector.

Liquid-glass treatment is limited to the outer transient surface and top-level controls. Content rows use stable semantic surfaces so multiple translucent cards do not compete with the timeline. Regular material is preferred over clear material because the interface contains dense text. Reduced Transparency switches to opaque semantic backgrounds.

Typography uses the platform system stack: SF on macOS and Segoe UI on Windows. The compact base size is 13, secondary text is never below 11, and Regular/Medium/Semibold weights replace thin weights. Primary timeline controls target 28 by 28 logical points/pixels with a 20 by 20 absolute desktop minimum. Status uses text and shape in addition to the green accent.

The accent color communicates active collection and selection only. Light, dark, increased-contrast, reduced-motion, keyboard-focus, screen-reader, localization, and long-title states are first-class component variants. Clicking a task opens the full detail window; it does not expand a bottom sheet inside the flyout.

### 10. Make permission and privacy setup progressive

First launch explains the local-only model and collection categories without imitating the operating-system alert. The user chooses Continue before the real platform permission request. Capture detail, browser enrichment, on-device AI, autostart, and local agent access are separate later choices and are not preselected.

The compact header always shows Recording, Paused, Permission needed, or Error in text as well as iconography. Exclusion and deletion controls live in the full settings/history window, with destructive actions showing the affected time range and data types.

### 11. Test the domain core independently from privileged platform integration

Recorded synthetic fixtures drive normalization, segmentation, retention, export, API, and prompt-injection tests on every CI platform. Platform adapter contracts use fake accessibility trees for deterministic unit tests and a small signed/manual integration harness for real permission tests.

The presentation layer receives fixture-backed Storybook-style states for empty, recording, paused, permission denied, long localized text, many sources, high contrast, reduced transparency, and low-height displays. Visual snapshots are platform-specific rather than forcing pixel identity across operating systems.

Packaging workflows build unsigned development artifacts on macOS and Windows. Release signing/notarization remains gated on repository secrets and owner-provided credentials.

## Risks / Trade-offs

- **Accessibility coverage varies by application and framework** → Store quality metadata, test representative native/web/cross-platform apps, and fall back to app/window history without invented detail.
- **A foreground app can expose unexpectedly sensitive text** → Minimize at the adapter boundary, ship conservative default exclusions, and make exclusions visible before collection starts.
- **Local websites or processes may probe the API** → Use random loopback binding, per-client scoped credentials, origin checks, rate limits, and revocation.
- **Captured text can contain prompt injection** → Treat all capture content as untrusted evidence, separate it structurally from instructions, constrain tools to read-only operations, and validate summaries.
- **SQLCipher increases cross-platform build complexity** → Pin reproducible native dependencies and exercise clean macOS/Windows packaging in CI from the first milestone.
- **One-process V1 can stop collecting if the desktop process crashes** → Isolate adapters, persist through bounded batches, restart on recoverable faults, and keep the core separable for a later service process.
- **A custom Tauri flyout may not perfectly match future platform materials** → Use semantic tokens and platform variants, test reduced-transparency modes, and prefer native tray behavior over pixel-identical styling.
- **Browser private-mode detection is adapter-dependent** → Never infer safety; if a supported adapter cannot positively establish an allowed normal context, store only application/window-level metadata.
- **Signing permissions and accessibility trust complicate distribution** → Build permission diagnostics and unsigned test artifacts early, then add notarized/signed release lanes when credentials are available.

## Migration Plan

1. Establish the Rust workspace, schema fixtures, deterministic domain tests, and empty desktop shell without requesting permissions.
2. Add local storage, privacy policy evaluation, and simulated event ingestion; validate the entire timeline using synthetic data.
3. Add macOS collection and permission diagnostics behind an opt-in developer flag, then enable the menu-bar experience.
4. Add the Windows adapter and tray experience against the same contract suite.
5. Add browser enrichment, deterministic segmentation, retention/deletion, export, and the full history window.
6. Add optional on-device model engines, the loopback API, and the read-only MCP companion after privacy and injection tests pass.
7. Produce unsigned private beta artifacts, collect coverage/performance data, then gate signed releases on platform credentials.

Rollback is a normal application downgrade only while the database schema remains compatible. Every migration must retain a pre-migration encrypted backup until the new version opens and validates it; incompatible downgrade attempts stop with an export/recovery path rather than modifying data.

## Open Questions

- The final public product name and icon can change after the functional prototype; repository and package internals use `OpenHistory` meanwhile.
- The open-source license must be selected before the private repository becomes public.
- Safari-specific URL enrichment can be evaluated after the native macOS collector proves which contexts Accessibility APIs expose reliably.
