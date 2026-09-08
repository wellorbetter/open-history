## Purpose

Make summarized history portable and inspectable through stable Markdown and JSON formats while allowing optional imports without coupling the core to another tracker.

## ADDED Requirements

### Requirement: Users can export selected history
The system SHALL export a selected time range as human-readable Markdown, machine-readable JSON, or both. Exports SHALL include schema version, local time-zone information, summary revisions, and permitted provenance.

#### Scenario: Export a daily recap
- **WHEN** the user exports one day as Markdown
- **THEN** the file contains the day heading, ordered task summaries, durations, sources, and an explicit indication of AI-generated or user-edited content

### Requirement: Export respects current privacy policy
The system MUST exclude credentials, excluded-source details, private-mode browser content, and expired raw events from all exports.

#### Scenario: Export overlaps private activity
- **WHEN** the requested range includes excluded intervals
- **THEN** the export contains no identifying data from those intervals

### Requirement: JSON format is versioned and forward compatible
The JSON export SHALL declare a schema version, preserve unknown fields during supported round trips, and reject unsupported breaking versions with an actionable error.

#### Scenario: Import a newer incompatible version
- **WHEN** the selected JSON declares an unsupported breaking schema version
- **THEN** the import makes no changes and explains which application version or migration is required

### Requirement: Imports are explicit and idempotent
Imports from OpenHistory or an enabled external adapter SHALL run only after preview and confirmation and SHALL avoid duplicating records when the same source data is imported again.

#### Scenario: Repeat an ActivityWatch-compatible import
- **WHEN** the user imports the same externally identified events twice
- **THEN** the second import reports the existing records and creates no duplicate task time

### Requirement: Imported content remains distinguishable
Imported events and summaries SHALL retain their adapter identity, original identifiers where permitted, and capture-quality metadata.

#### Scenario: Imported and native events form one task
- **WHEN** imported and native events are grouped into the same task
- **THEN** the task provenance distinguishes both sources and supports filtering by origin
