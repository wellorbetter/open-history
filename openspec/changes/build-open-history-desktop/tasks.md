## 1. Repository and Toolchain Foundation

- [x] 1.1 Scaffold the Tauri 2 desktop package, React/TypeScript UI, and Rust workspace crates for domain, storage, segmentation, summarization, platform adapters, API, and MCP; verify clean Rust and frontend builds on the development host.
- [ ] 1.2 Add pinned formatting, linting, unit-test, and dependency-audit commands; verify the commands fail on intentional fixture violations and pass after reverting them.
- [x] 1.3 Add macOS and Windows CI matrices for Rust, frontend, and packaging checks; verify both jobs reach the unsigned artifact stage from a clean checkout.
- [ ] 1.4 Add project architecture, privacy model, contribution, and local development documentation; verify every documented setup command works in a clean temporary checkout.

## 2. Canonical Event Domain

- [x] 2.1 Implement the versioned canonical event envelope and typed semantic payloads; verify serialization round trips and compatibility fixtures for every V1 event kind.
- [x] 2.2 Implement source identity, capture-quality, privacy-classification, redaction, and correlation metadata; verify missing and partial platform data is represented without fabricated values.
- [x] 2.3 Implement monotonic ordering and local timestamp/time-zone normalization; verify daylight-saving changes, clock rollback, late arrival, and equal timestamp fixtures.
- [x] 2.4 Define the platform adapter contract and bounded event channel; verify fake adapters can be started, stopped, faulted, and restarted without blocking the core.

## 3. Privacy Policy and Encrypted Storage

- [x] 3.1 Implement the pre-persistence application, window-pattern, website, and private-context policy evaluator; verify excluded fixture content never reaches the storage mock or diagnostics logs.
- [x] 3.2 Implement SQLCipher-backed SQLite initialization, WAL mode, migrations, and OS credential-store key retrieval; verify a database cannot be opened without its generated key and reopens across app restarts.
- [ ] 3.3 Implement repositories for sources, raw events, sessions, segments, links, summary revisions, policies, clients, and imports; verify transactional CRUD and rollback tests for every repository.
- [ ] 3.4 Implement the retention worker with a 48-hour default and shorter/no-raw-history options; verify boundary timestamps and linked search indexes are deleted transactionally.
- [ ] 3.5 Implement single-segment, recent-period, current-day, date-range, and all-history deletion; verify each integration test removes raw, derived, indexed, and managed temporary-export data in scope only.
- [ ] 3.6 Implement redacted diagnostics output; verify automated secret scanning finds no database key, bearer token, authorization header, or excluded content.

## 4. Deterministic Session and Task Segmentation

- [ ] 4.1 Implement idle, sleep, lock, logout, shutdown, and explicit project/document boundary handling; verify fixture durations never include inactive time.
- [ ] 4.2 Implement deterministic continuity scoring across time, project/document identity, window tokens, and application transitions; verify repeated processing produces byte-equivalent segment projections.
- [ ] 4.3 Implement online current-task projection and late-event reconciliation; verify qualifying fixture activity appears within ten seconds and reconciliation preserves stable identifiers where inputs are unchanged.
- [ ] 4.4 Implement confidence reduction and splitting across private or low-quality gaps; verify excluded content is neither inspected nor inferred by segmentation tests.
- [ ] 4.5 Implement rename, split, merge, recategorize, and same-session undo as derived revisions; verify corrections preserve immutable source links and deterministic replay.

## 5. Summary Pipeline

- [ ] 5.1 Implement the deterministic title, outline, duration, and source formatter; verify useful summaries are produced offline for single-source, multi-source, sparse, and long-running task fixtures.
- [ ] 5.2 Define the on-device structured summarizer interface and output validator; verify malformed, timed-out, hallucinated-entity, and oversized responses fall back without blocking the timeline.
- [ ] 5.3 Implement minimized prompt assembly that treats captured text as delimited untrusted evidence; verify prompt-injection fixtures cannot alter system instructions or request tool execution.
- [ ] 5.4 Implement an on-device model adapter with a local-only transport; verify no remote endpoint or model credential can be configured and network-deny tests observe no outbound activity-data request.
- [ ] 5.5 Implement manual, scheduled, retry, edit, regenerate, and revert summary revisions; verify attribution and source-segment revision remain visible through every transition.

## 6. Desktop Design System and Fixture UI

- [ ] 6.1 Create semantic color, typography, spacing, radius, material, motion, and focus tokens with macOS and Windows variants; verify light, dark, increased-contrast, reduced-transparency, and reduced-motion fixture pages.
- [ ] 6.2 Build reusable status, source-icon stack, current-activity card, timeline row, date navigation, empty/error/loading, and destructive-confirmation components; verify keyboard and screen-reader component tests plus long-title and many-source snapshots.
- [x] 6.3 Build the fixture-driven compact route at the specified macOS and Windows logical sizes; verify header controls remain fixed, timeline scroll remains independent, and all content fits low-height work areas.
- [ ] 6.4 Build the resizable full history window with date navigation, search/results, task inspector, settings, export, and deletion flows; verify behavior at minimum, default, and expanded window sizes.
- [ ] 6.5 Connect UI state to typed Tauri commands and event subscriptions using a fake core; verify recording, paused, permission-needed, adapter-error, current-task, deletion, and summary-revision interaction tests.
- [x] 6.6 Run visual and accessibility review against the supplied WeekLens reference and platform variants; verify contrast thresholds, desktop control sizes, focus order, truncation recovery, and opaque reduced-transparency rendering.

## 7. macOS Collection and Menu-Bar Integration

- [ ] 7.1 Implement macOS application activation, window, idle, lock, sleep, and wake observation; verify a signed integration harness emits canonical fixtures for Finder, Terminal, a browser, and an editor.
- [ ] 7.2 Implement AXObserver/AXUIElement semantic control observation and explicit quality fallbacks; verify no raw-key or screenshot API is linked and unsupported applications degrade to app/window metadata.
- [ ] 7.3 Implement Accessibility permission status, progressive setup, revocation detection, and recovery diagnostics; verify no events persist before consent and collection stops immediately after simulated revocation.
- [ ] 7.4 Implement the hidden-Dock menu-bar lifecycle and tray-anchored popover positioning across displays; verify open, dismiss, focus, Spaces, menu-bar relocation, notch-safe placement, and app restart behavior.

## 8. Windows Collection and Tray Integration

- [ ] 8.1 Implement foreground WinEvent, idle, session lock, sleep, and wake observation; verify canonical fixtures from Explorer, Terminal, a browser, and an editor.
- [ ] 8.2 Implement UI Automation semantic event observation and explicit quality fallbacks; verify unavailable or elevated targets do not crash collection or produce invented context.
- [ ] 8.3 Implement permission/access diagnostics and adapter recovery for elevated, protected, and inaccessible applications; verify collection status explains reduced coverage without exposing sensitive source details.
- [ ] 8.4 Implement notification-area lifecycle and flyout positioning for every taskbar edge, scaling level, and multi-display work area; verify the flyout remains visible and keyboard accessible after Explorer restart.

## 9. Browser Enrichment

- [ ] 9.1 Implement the authenticated browser-adapter registration and least-privilege event schema; verify unregistered extensions and invalid origins cannot submit activity.
- [ ] 9.2 Implement a Chromium-family extension for active-tab title, URL, navigation, and private-mode signaling; verify incognito content is discarded and permission copy matches actual data use.
- [ ] 9.3 Implement website exclusion matching before persistence, including normalized hosts and path patterns; verify international domains, subdomains, redirects, and malformed URLs cannot bypass exclusions.
- [ ] 9.4 Implement browser/native event correlation and duplicate suppression; verify enriched events retain both provenance records without double-counting elapsed time.

## 10. Local API and MCP Access

- [ ] 10.1 Implement random-port loopback startup, versioned routes, per-client scoped bearer credentials, origin checks, rate limits, and revocation; verify non-loopback, unauthenticated, revoked, oversized, and cross-origin requests are rejected.
- [ ] 10.2 Implement bounded read APIs for search, recent tasks, task detail, daily recap, sources, status, and health; verify time-range, limit, pagination, exclusion, and least-sensitive defaults.
- [ ] 10.3 Implement the `openhistory-mcp` stdio companion and read-only tools/resources; verify protocol conformance and interoperability with a reference MCP inspector/client.
- [ ] 10.4 Add untrusted-content and provenance markers to all agent-facing results; verify malicious source fixtures remain data and cannot modify tool schemas or invoke mutations.
- [ ] 10.5 Implement settings to approve, inspect, rotate, revoke, or disable API/MCP clients independently of collection; verify revocation takes effect on the next request.

## 11. History Interchange

- [ ] 11.1 Implement Markdown daily/range export with ordered tasks, durations, sources, and revision attribution; verify golden files across time zones, empty days, private gaps, and user-edited summaries.
- [ ] 11.2 Implement versioned JSON export/import with unknown-field preservation and breaking-version rejection; verify round trips, unsupported-version rollback, and no secret or excluded content leakage.
- [ ] 11.3 Implement previewed, confirmed, idempotent adapter imports and an ActivityWatch-compatible adapter; verify repeated imports create no duplicate events or task time.
- [ ] 11.4 Implement import provenance filters in the full history window; verify native, browser-enriched, OpenHistory-imported, and external-adapter records remain distinguishable.

## 12. End-to-End Validation and Private Beta Packaging

- [ ] 12.1 Run the complete synthetic journey from consent through capture, exclusion, segmentation, deterministic summary, timeline review, correction, export, agent query, retention, and deletion; verify all capability scenarios are mapped to passing automated tests.
- [ ] 12.2 Measure idle and active CPU, memory, event latency, database growth, and retention cleanup on representative macOS and Windows machines; verify the idle targets or document and approve any measured exception before release.
- [ ] 12.3 Test crash recovery, forced shutdown, database migration backup/restore, incompatible downgrade, adapter failure, and corrupted import handling; verify no silent data loss or privacy-policy bypass.
- [ ] 12.4 Build unsigned macOS and Windows private-beta packages with checksums and provenance metadata; verify installation, autostart choice, tray/menu-bar lifecycle, clean uninstall, and local-data preservation/deletion options on clean virtual machines.
- [ ] 12.5 Add release-signing and notarization placeholders that require repository secrets; verify unsigned CI remains functional and release jobs fail safely with clear missing-credential diagnostics.
- [ ] 12.6 Produce a concise acceptance report with screenshots, accessibility results, performance measurements, known application-coverage gaps, and package hashes; verify every V1 requirement is Passed, Failed, or Explicitly Blocked with evidence.
