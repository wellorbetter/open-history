## Purpose

Aggregate segments across days into project-level work items with linked evidence, and produce a
report draft from a selected range, so that reconstructing what was worked on does not depend on
memory or on commit history alone.

## ADDED Requirements

### Requirement: A range digest aggregates segments into project-level work items
The system SHALL aggregate segments over a requested date range into work items grouped by project
entity, each carrying a deterministic title, total observed attention duration, contributing
entities, artifact counts, and the days on which activity occurred.

#### Scenario: Work on one project spans several days
- **WHEN** a range digest is generated for a week containing activity for one project on three days
- **THEN** one work item is produced with the combined duration and the three contributing days
  identified

#### Scenario: Range contains work with no artifacts
- **WHEN** a range contains reading, browsing, and meetings that produced no commits or sessions
- **THEN** those work items still appear, attributed to their observed sources

#### Scenario: Digest is regenerated
- **WHEN** an unchanged range is aggregated twice
- **THEN** both runs produce byte-equivalent work items and ordering

### Requirement: Every work item is traceable to its evidence
Each work item SHALL link to the segments, commits, agent sessions, and meetings that produced it,
and the user SHALL be able to open that evidence from the work item.

#### Scenario: User questions a reported work item
- **WHEN** the user opens a work item in the review surface
- **THEN** the contributing evidence is listed with its source and time, and selecting an item opens
  the corresponding day timeline filtered to it

#### Scenario: Evidence is deleted after a digest is generated
- **WHEN** underlying evidence is deleted through existing deletion controls
- **THEN** the digest no longer reports the removed evidence on its next generation and MUST NOT
  retain an orphaned copy of it

### Requirement: Report drafts are exportable and marked as drafts
The system SHALL export a range digest as Markdown containing work items, durations, and evidence
references. Exported content SHALL be identified as a generated draft, and model-authored narrative
text SHALL be distinguishable from deterministic content.

#### Scenario: User exports the current week
- **WHEN** the user exports a report draft for the current week
- **THEN** the Markdown contains the week's work items with durations and evidence references and is
  usable without further processing

#### Scenario: Narrative summaries are present
- **WHEN** a work item carries a model-authored summary
- **THEN** the export marks that text as generated and retains the deterministic digest content
  alongside it

#### Scenario: Range contains excluded periods
- **WHEN** a range includes periods removed by exclusion or retention
- **THEN** the export represents them as gaps and MUST NOT reconstruct or estimate the missing work

### Requirement: Digest generation requires no network access
Range digests and report drafts SHALL be produced from local data without any outbound request. A
digest MUST remain available when no summarization engine is configured.

#### Scenario: No summarization engine is configured
- **WHEN** a report draft is exported with summarization disabled
- **THEN** the draft is produced from deterministic content only and is usable as written
