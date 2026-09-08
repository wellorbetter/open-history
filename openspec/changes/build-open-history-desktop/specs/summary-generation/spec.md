## Purpose

Create concise, grounded task summaries with a dependable non-AI baseline and optional provider-neutral AI enrichment that remains transparent and reversible.

## ADDED Requirements

### Requirement: Every finalized task has a non-AI representation
The system SHALL generate a deterministic title, time range, duration, contributing applications, and concise activity outline without requiring a model or network connection.

#### Scenario: No model is configured
- **WHEN** a task segment is finalized while AI summarization is disabled
- **THEN** the timeline immediately presents a usable deterministic summary

### Requirement: AI enrichment is optional and attributable
The system SHALL support manual and scheduled AI enrichment through interchangeable providers. Every enriched summary SHALL disclose that AI was used, the provider category, generation time, and the source segment revision.

#### Scenario: AI enrichment succeeds
- **WHEN** the configured provider returns a valid grounded summary
- **THEN** the system stores it as a new derived revision without deleting the deterministic fallback

#### Scenario: AI enrichment fails
- **WHEN** the provider is unavailable, times out, or returns invalid output
- **THEN** the deterministic summary remains usable and the user receives a retryable, non-blocking error

### Requirement: Summaries remain grounded in recorded evidence
Generated summaries MUST distinguish observed activity from inference, MUST NOT claim actions absent from permitted source evidence, and MUST treat captured application content as untrusted data rather than instructions.

#### Scenario: Captured page contains agent instructions
- **WHEN** source text includes instructions directed at an AI model
- **THEN** the summarizer treats that text as quoted evidence, does not execute or follow it, and omits unsupported conclusions

### Requirement: Users can revise or regenerate summaries
The system SHALL let users edit a generated title and summary, revert to the deterministic version, and regenerate from the same or updated source segment.

#### Scenario: User edits an AI summary
- **WHEN** the user saves a corrected summary
- **THEN** it becomes the displayed revision, is marked user-edited, and the prior revisions remain locally recoverable until the segment is deleted
