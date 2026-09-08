## Purpose

Give users understandable control over sensitive activity data throughout collection, local processing, optional AI processing, retention, export, and deletion.

## ADDED Requirements

### Requirement: Users control collection at all times
The system SHALL provide one-action pause and resume controls from both the compact surface and settings. Pausing MUST prevent new source events from entering storage within two seconds.

#### Scenario: Pause from the compact surface
- **WHEN** the user selects Pause from the menu-bar popover or tray flyout
- **THEN** new activity is not persisted, the status changes to Paused, and the user can resume without restarting the application

### Requirement: Source exclusions apply before persistence
The system SHALL support application, window-pattern, and website exclusions and SHALL apply them before writing event content to persistent storage.

#### Scenario: Excluded password manager is focused
- **WHEN** an event originates from an excluded application
- **THEN** its content is not persisted and the timeline discloses at most an optional private-time gap without identifying the excluded source

#### Scenario: Browser private mode is detected
- **WHEN** a supported browser adapter reports a private-browsing context
- **THEN** the URL, page title, and accessible page content are discarded before persistence

### Requirement: Retention is bounded and user configurable
Raw semantic events SHALL expire after 48 hours by default. The user SHALL be able to select a shorter retention period or disable retention of raw events after task summaries are finalized.

#### Scenario: Retention sweep runs
- **WHEN** a raw event passes the configured expiration boundary
- **THEN** the raw event and indexes that can reconstruct it are deleted while retained summaries preserve only their allowed provenance

### Requirement: Deletion is complete and understandable
The system SHALL let the user delete a single task segment, the last 10 minutes, the last hour, the current day, a selected date range, or all history. Destructive range deletion MUST show its scope and require confirmation.

#### Scenario: Delete the last hour
- **WHEN** the user confirms deletion of the last hour
- **THEN** matching raw events, derived segments, summaries, search entries, and exported temporary files managed by the application are removed

### Requirement: External processing is opt-in
The system SHALL keep summarization on device unless the user explicitly configures an external provider, and SHALL disclose what fields will leave the device before the first external request.

#### Scenario: External AI is enabled
- **WHEN** the user approves the first external summarization request
- **THEN** only the previewed, minimized payload is sent and the selected provider is visibly associated with the generated result

### Requirement: Secrets are protected
Provider credentials and local API credentials MUST be stored using the operating system's secure credential facility and MUST NOT appear in logs, exports, or plaintext configuration files.

#### Scenario: Diagnostics are exported
- **WHEN** the user creates a diagnostics bundle
- **THEN** all credentials and credential-derived authorization headers are absent or irreversibly redacted
