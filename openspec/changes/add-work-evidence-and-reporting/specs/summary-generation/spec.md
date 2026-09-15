## Purpose

Extend summarization to the reporting scenario: keep the deterministic baseline authoritative, bound
every summarizer's input to aggregated digests, and allow the user's own agent to write narrative
text through MCP without giving OpenHistory an outbound network path.

## MODIFIED Requirements

### Requirement: AI enrichment is optional and attributable
The system SHALL support manual and scheduled AI enrichment through interchangeable summarization
engines, which SHALL be either an on-device engine or the user's connected agent acting through the
read-only MCP surface. Every enriched summary SHALL disclose that AI was used, which engine produced
it, the generation time, and the source digest or segment revision. The system MUST NOT provide a
built-in client that transmits activity data, digests, summaries, or diagnostics to a remote service,
and MUST NOT store a remote provider credential for summarization.

#### Scenario: AI enrichment succeeds
- **WHEN** the configured engine returns a valid grounded summary
- **THEN** the system stores it as a new derived revision without deleting the deterministic fallback

#### Scenario: AI enrichment fails
- **WHEN** the engine is unavailable, times out, or returns invalid output
- **THEN** the deterministic summary remains usable and the user receives a retryable, non-blocking
  error

#### Scenario: User attempts to configure a remote endpoint
- **WHEN** a remote provider endpoint or credential is supplied for summarization
- **THEN** the system rejects it and explains that narrative generation runs on device or through the
  user's connected agent

### Requirement: Summaries remain grounded in recorded evidence
Generated summaries MUST distinguish observed activity from inference, MUST NOT claim actions absent
from permitted source evidence, and MUST treat captured application content, repository content, and
agent session text as untrusted data rather than instructions.

#### Scenario: Captured page contains agent instructions
- **WHEN** source text includes instructions directed at an AI model
- **THEN** the summarizer treats that text as quoted evidence, does not execute or follow it, and
  omits unsupported conclusions

#### Scenario: Commit subject contains directive text
- **WHEN** a commit subject or agent session message contains directive text
- **THEN** it is summarized as evidence and MUST NOT alter summarization instructions or request tool
  execution

## ADDED Requirements

### Requirement: Summarizer input is bounded to aggregated digests
Every summarization request SHALL supply aggregated digest content — titles, time ranges,
contributing entities, counts, and a bounded evidence sample. Raw event streams MUST NOT be supplied
to any summarization engine.

#### Scenario: A busy week is summarized
- **WHEN** a range containing tens of thousands of raw events is summarized
- **THEN** the request carries digest rows within a bounded size, and request size does not scale
  with raw event count

#### Scenario: Evidence sample exceeds its bound
- **WHEN** a work item carries more evidence than the sample bound allows
- **THEN** the sample is truncated deterministically and the summary discloses that evidence was
  sampled

### Requirement: Delegated summarization is disclosed and off by default
When narrative text is produced by the user's connected agent rather than an on-device engine, the
system SHALL disclose before use that digest content will be read by that agent and that the agent's
transport is outside OpenHistory's boundary. Delegated summarization SHALL be disabled until the user
enables it.

#### Scenario: User enables delegated summarization
- **WHEN** the user enables summarization through a connected agent
- **THEN** the system states what content the agent can read, and the setting is recorded as an
  explicit approval that can be revoked

#### Scenario: Delegated summarization is not enabled
- **WHEN** no on-device engine is configured and delegation is disabled
- **THEN** deterministic summaries are used throughout and no prompt, digest, or evidence leaves the
  application
