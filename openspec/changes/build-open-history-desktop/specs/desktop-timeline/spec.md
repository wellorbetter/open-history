## Purpose

Provide a calm, compact desktop surface for current activity and recent history, plus a focused full window for review, correction, search, and settings.

## ADDED Requirements

### Requirement: Platform-appropriate compact entry point
The application SHALL run from a macOS menu-bar item and a Windows notification-area icon. Activating the item SHALL open a compact surface visually anchored to that item without requiring a taskbar or Dock presence by default.

#### Scenario: Open from macOS menu bar
- **WHEN** the user activates the menu-bar item
- **THEN** a popover opens beneath the item and receives keyboard focus without activating an unrelated application window

#### Scenario: Open from Windows tray
- **WHEN** the user activates the notification-area icon
- **THEN** a flyout opens adjacent to the taskbar notification area and remains fully within the active display work area

### Requirement: Compact surface prioritizes current state
The compact surface SHALL show product identity, collection status, a pause or resume action, current task title, time range, duration, contributing-source icons, the selected day's timeline, date navigation, and an entry to the full review window.

#### Scenario: Current task has multiple sources
- **WHEN** more source applications contributed than fit in the current card
- **THEN** the card shows a bounded source preview and an accessible overflow count without shrinking the task title below the minimum readable size

### Requirement: Timeline rows remain scannable
Each visible row SHALL present start time, task title, source summary, duration or merge status, and current or historical state with consistent alignment. Full titles SHALL remain reachable when visual truncation is required.

#### Scenario: Long task title
- **WHEN** a task title exceeds the available row width
- **THEN** the row truncates it to one line, exposes the full title to assistive technology, and shows the full text on focus, hover, or detail opening

### Requirement: Detailed review uses a full window
Selecting a task or Review SHALL open a resizable full history window rather than expanding complex editing controls inside the compact surface.

#### Scenario: Open task details
- **WHEN** the user selects a timeline task
- **THEN** the full window opens on that task with summary, source provenance, corrections, export, and deletion actions

### Requirement: Appearance adapts to desktop accessibility settings
The application SHALL support light and dark appearance, increased contrast, reduced transparency, reduced motion, full keyboard navigation, and screen-reader labels. Essential status MUST NOT rely on color alone.

#### Scenario: Reduce transparency is enabled
- **WHEN** the operating system requests reduced transparency
- **THEN** translucent functional surfaces switch to an opaque semantic background while preserving hierarchy and contrast

#### Scenario: Keyboard-only navigation
- **WHEN** focus enters either desktop surface
- **THEN** all interactive controls and timeline rows are reachable in logical reading order with a visible focus indicator
