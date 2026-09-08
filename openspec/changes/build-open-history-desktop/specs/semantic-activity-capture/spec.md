## Purpose

Define a consent-based, cross-platform stream of useful desktop interaction events without turning the product into a screen, audio, or keystroke recorder.

## ADDED Requirements

### Requirement: Collection is explicit and observable
The system SHALL keep collection disabled until the user completes consent and the required operating-system permission is granted. While collection is enabled, the system SHALL expose a persistent recording-state indicator in the menu bar or system tray.

#### Scenario: First launch without permission
- **WHEN** the application starts before collection consent or platform permission has been granted
- **THEN** it records no activity and displays the setup state

#### Scenario: Permission is revoked
- **WHEN** the operating system revokes required accessibility access during collection
- **THEN** collection stops, the visible state changes to an actionable error, and no synthetic gap activity is created

### Requirement: Capture semantic events only
The system SHALL capture application activation, window changes, accessible control actions, navigation, selected command metadata, and idle or resume transitions when exposed by the platform. It MUST NOT capture screenshots, audio, or raw keystroke sequences in the first release.

#### Scenario: User types into an editor
- **WHEN** accessibility events indicate editing activity in an allowed application
- **THEN** the event stream records the application, window, control role, action category, and permitted contextual text without storing the raw keys pressed

#### Scenario: Unsupported application
- **WHEN** an application exposes only application and window metadata
- **THEN** the collector records the available metadata and marks deeper semantic context as unavailable rather than fabricating it

### Requirement: Events have stable provenance
Every normalized event SHALL include a schema version, event identifier, source adapter, device-local timestamp with time-zone offset, application identity, privacy classification, and capture-quality level.

#### Scenario: Events arrive from different platform adapters
- **WHEN** equivalent application-switch events are received on macOS and Windows
- **THEN** downstream consumers receive the same canonical event type while retaining platform-specific provenance

### Requirement: Idle and lifecycle transitions are represented
The system SHALL detect user-configurable inactivity, sleep, lock, logout, and application shutdown boundaries so elapsed time is not attributed to active work.

#### Scenario: User leaves the computer
- **WHEN** the idle threshold is crossed with no qualifying interaction
- **THEN** the active interval closes at the last qualifying activity and an idle boundary is emitted
