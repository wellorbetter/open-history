## Purpose

Give connected agents a digest-oriented read surface with conservative defaults, so that the common
request returns aggregated work items over a recent window rather than raw event detail, and
widening that scope is an explicit user decision.

## ADDED Requirements

### Requirement: Agent access defaults to a recent window and segment-level granularity
Agent-facing read operations SHALL default to a bounded recent time window and SHALL return
segment-level and digest-level records. Raw event access MUST NOT be available by default.

#### Scenario: Agent requests history without a range
- **WHEN** a connected agent queries history with no explicit time range
- **THEN** the response covers the default recent window and states the range it covers

#### Scenario: Agent requests raw events
- **WHEN** a connected agent requests raw event detail without an approval that grants it
- **THEN** the request is refused with an explanation, and segment-level records remain available

#### Scenario: Agent requests a range beyond the default
- **WHEN** a connected agent requests a range wider than the client's approved scope
- **THEN** the response is limited to the approved scope and identifies that it was narrowed

### Requirement: Scope widening is an explicit, revocable user action
Extending a client's time range or granularity SHALL require user approval recorded against that
client, and SHALL be revocable independently of collection and of other clients.

#### Scenario: User widens scope for one client
- **WHEN** the user approves a wider scope for one connected agent
- **THEN** only that client's subsequent requests receive the wider scope

#### Scenario: User revokes a widened scope
- **WHEN** the user revokes a previously approved scope
- **THEN** the next request from that client is served at the default scope

### Requirement: Agents can read range digests for summarization
The agent surface SHALL expose range digests containing work items, durations, contributing
entities, counts, and a bounded evidence sample, so that a connected agent can produce narrative
summaries without reading raw events.

#### Scenario: Agent summarizes a week
- **WHEN** a connected agent requests a digest for a week within its approved scope
- **THEN** it receives bounded digest rows sufficient to write a summary, and the response size does
  not scale with raw event count

#### Scenario: Digest contains excluded periods
- **WHEN** a requested range includes excluded or expired periods
- **THEN** those periods are represented as gaps and no excluded content is returned

### Requirement: Digest responses carry untrusted-content and provenance markers
Digest and evidence content returned to agents SHALL be marked as untrusted data with its source
provenance, consistent with existing agent-facing results.

#### Scenario: Evidence sample contains directive text
- **WHEN** a returned evidence sample contains text directed at an AI model
- **THEN** it is delivered as marked untrusted data and MUST NOT alter tool schemas or enable
  mutations
