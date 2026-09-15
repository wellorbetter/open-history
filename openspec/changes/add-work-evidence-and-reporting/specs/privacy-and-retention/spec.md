## Purpose

Extend the privacy boundary to the added evidence sources so that repository, calendar, and
agent-session data are subject to the same pre-persistence exclusion, visibility, retention, and
deletion guarantees as platform capture.

## ADDED Requirements

### Requirement: Added sources are subject to pre-persistence exclusion
Repository, calendar, and agent-session evidence SHALL pass the exclusion evaluator before
persistence, using repository opt-in state, calendar exclusions, path patterns, and existing
application and website exclusions.

#### Scenario: Excluded path appears in artifact evidence
- **WHEN** an excluded path pattern matches content in a commit or agent session record
- **THEN** the matching content never reaches storage, the search index, or diagnostics output

#### Scenario: Exclusion is added after capture
- **WHEN** the user excludes a repository or calendar that already contributed evidence
- **THEN** collection stops for that source and previously stored evidence is removed through the
  existing deletion controls

### Requirement: Collection visibility covers every enabled source
The system SHALL show which evidence sources are enabled, which are degraded, and when each last
produced evidence, without exposing excluded content in that surface.

#### Scenario: One source is degraded
- **WHEN** an artifact parser cannot recognize its upstream format
- **THEN** that source is shown as degraded with the reason, and the remaining sources are shown as
  active

#### Scenario: A source has produced nothing
- **WHEN** an enabled source has produced no evidence in the current period
- **THEN** the surface distinguishes "no activity" from "not collecting" so that missing evidence is
  never read as absent work

### Requirement: Agent session evidence is redacted before persistence
Captured agent session records SHALL retain intent, request and result text, and state, and MUST have
credential-bearing content redacted before persistence, including tokens, keys, and authorization
headers appearing in session text.

#### Scenario: Session text contains a credential
- **WHEN** an agent session record contains an API key or bearer token
- **THEN** it is redacted before persistence and automated secret scanning of the database and
  diagnostics finds no occurrence of it

### Requirement: Derived digests obey the retention of their evidence
Range digests and report drafts SHALL be derived data. When evidence is removed by retention or
deletion, subsequent digests MUST NOT include it, and stored digests MUST NOT preserve a copy of
removed evidence.

#### Scenario: Raw evidence expires under retention
- **WHEN** raw events expire while derived segments are retained
- **THEN** digests continue to report retained derived work items and MUST NOT reconstruct expired
  raw detail

#### Scenario: User deletes a day covered by an exported draft
- **WHEN** the user deletes a day that a previously exported draft covered
- **THEN** stored digest data for that day is removed, and a previously exported file outside the
  application is identified to the user as outside its control
