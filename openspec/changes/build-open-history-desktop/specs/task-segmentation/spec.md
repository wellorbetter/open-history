## Purpose

Turn noisy, short-lived interaction events into stable sessions and task segments that reflect meaningful work while preserving inspectable source provenance.

## ADDED Requirements

### Requirement: Events are grouped deterministically
The system SHALL produce the same session and task boundaries when given the same ordered events and segmentation settings. Grouping SHALL consider temporal proximity, application and window continuity, project or document identity, and explicit idle boundaries.

#### Scenario: Deterministic replay
- **WHEN** the same fixture event stream is processed twice with identical settings
- **THEN** both runs produce equivalent segment identifiers, boundaries, source ordering, and non-AI labels

### Requirement: Cross-application work can form one task
The system SHALL be able to group activity from multiple allowed applications into one task when the evidence indicates a continuous goal, while retaining every contributing source.

#### Scenario: Coding and browser research alternate
- **WHEN** editor, terminal, and browser events reference the same project within the continuity window
- **THEN** they may form one task segment whose source list includes all three applications

### Requirement: Gaps and exclusions do not invent continuity
The system MUST NOT infer task details from excluded content and SHALL reduce confidence or split a task when the remaining evidence cannot support continuity.

#### Scenario: Excluded interval separates two activities
- **WHEN** allowed events occur on both sides of a private interval with insufficient shared context
- **THEN** the system creates separate segments or marks the relationship as uncertain

### Requirement: Users can correct task boundaries
The full history view SHALL let users rename, split, merge, and recategorize task segments. Corrections SHALL preserve provenance and MUST NOT mutate the original raw events.

#### Scenario: User merges adjacent segments
- **WHEN** the user confirms merging two compatible adjacent segments
- **THEN** a new derived segment replaces them in the timeline, retains their source references, and can be undone during the current editing session

### Requirement: Current activity updates promptly
The active task projection SHALL update within ten seconds of a qualifying event under normal local operation.

#### Scenario: User switches projects
- **WHEN** a strong project-identity change is captured
- **THEN** the compact current-activity card reflects the new project or a processing state within ten seconds
