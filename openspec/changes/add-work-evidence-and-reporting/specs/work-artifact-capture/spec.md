## Purpose

Capture the local evidence that desktop observation cannot see — Git repository state and AI
coding-agent session records — as consent-scoped canonical events with independent provenance, so
that work performed through an agent is represented as faithfully as work performed on screen.

## ADDED Requirements

### Requirement: Repository observation is opt-in for each repository
The system SHALL observe a Git repository only after the user adds it explicitly. Discovering a
repository through other captured activity SHALL NOT enable observation of it.

#### Scenario: Repository is observed in window activity but not opted in
- **WHEN** captured window activity references a repository the user has not added
- **THEN** no repository evidence is read or persisted for it, and the repository MAY be offered as a
  suggestion that requires explicit confirmation

#### Scenario: User removes a repository
- **WHEN** the user removes a repository from observation
- **THEN** observation stops before the next collection cycle and previously derived evidence is
  deleted on the user's existing deletion controls

### Requirement: Repository evidence is limited to metadata and change statistics
The system SHALL read commit identifiers, timestamps, subjects, branch names, and changed-path
statistics. The system MUST NOT read working-tree file contents, diff hunks, staged content, or
remote credentials.

#### Scenario: Repository contains uncommitted sensitive files
- **WHEN** a repository holds uncommitted files containing secrets
- **THEN** their contents are never read, and only paths of committed changes are represented

#### Scenario: Excluded path pattern matches a changed file
- **WHEN** a committed change touches a path matching an active exclusion pattern
- **THEN** that path is omitted from persisted evidence while the commit itself MAY be retained with
  a reduced-coverage indicator

### Requirement: Agent session records are captured incrementally from local storage
The system SHALL derive evidence from local AI coding-agent session records without network access,
reading only content appended since the previously derived position for that session.

#### Scenario: Long-running session grows during capture
- **WHEN** a session record has grown since the previous collection cycle
- **THEN** only the appended portion is parsed, and derived evidence reflects the session's current
  intent, latest request, result, and state

#### Scenario: Session record is truncated or rewritten
- **WHEN** a session record no longer contains the previously derived position
- **THEN** the source re-derives from the start of the record without emitting duplicate evidence for
  already-represented activity

### Requirement: Unrecognized upstream formats degrade visibly without fabricating evidence
Each artifact parser SHALL declare the upstream layout versions it recognizes. When a record does not
match a recognized layout, the parser MUST emit no evidence for that record, MUST record a
source-level diagnostic, and MUST NOT block capture from any other source.

#### Scenario: Agent storage format changes after an upstream upgrade
- **WHEN** session records no longer match a recognized layout
- **THEN** the agent-session source reports a degraded status with the observed version, emits no
  events, and window, repository, and calendar capture continue unaffected

#### Scenario: A single record is malformed
- **WHEN** one record within a recognized store fails to parse
- **THEN** that record is skipped with a diagnostic and the remaining records are still derived

### Requirement: Artifact evidence carries independent provenance and does not consume attention time
Artifact events SHALL carry their own source identity and provenance, distinct from platform capture.
Derived durations from artifact evidence MUST NOT be added to observed attention duration for the
same period.

#### Scenario: Commits land during an observed editing session
- **WHEN** repository commits and window activity cover the same time range for one project
- **THEN** the segment reports observed attention duration and artifact counts as separate
  quantities, and elapsed time is counted once

#### Scenario: Agent session runs with no corresponding window activity
- **WHEN** an agent session produces evidence while no related window activity is observed
- **THEN** the work is still represented, attributed to the agent-session source, and marked as
  having no observed attention data
