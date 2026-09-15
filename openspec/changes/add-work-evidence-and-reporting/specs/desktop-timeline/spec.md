## Purpose

Add a week review surface as the primary reporting view, present evidence from every source without
conflating attention time with artifact counts, and keep the existing day timeline as the detail
view behind it.

## ADDED Requirements

### Requirement: The full history window provides a week review surface
The full history window SHALL provide a week surface listing project-level work items for a selected
week, each showing a title, observed attention duration, contributing sources, and a summary slot,
ordered deterministically.

#### Scenario: User opens the week surface
- **WHEN** the user opens the week surface for a week containing activity
- **THEN** work items are listed with durations and contributing sources, and the surface identifies
  the week range being shown

#### Scenario: Week contains no activity
- **WHEN** a selected week contains no retained activity
- **THEN** the surface distinguishes an empty week from a week outside the retention period

#### Scenario: User navigates between weeks
- **WHEN** the user moves to an adjacent week
- **THEN** the surface updates without losing the current selection state for the newly shown week

### Requirement: Work items open their contributing evidence
Selecting a work item SHALL open its contributing evidence, and selecting an evidence entry SHALL
open the day timeline positioned on that entry.

#### Scenario: User inspects a reported work item
- **WHEN** the user selects a work item and then one of its commits
- **THEN** the day timeline opens on the day and segment containing that commit

### Requirement: Attention time and artifact counts are presented as separate quantities
Surfaces SHALL present observed attention duration separately from artifact counts such as commits,
agent sessions, and meetings, and MUST NOT combine them into a single elapsed figure.

#### Scenario: A work item has commits and observed attention
- **WHEN** a work item carries both observed attention and artifact evidence
- **THEN** the surface shows duration and counts as distinct values

#### Scenario: A work item has artifacts but no observed attention
- **WHEN** an agent session produced evidence with no observed window activity
- **THEN** the work item is shown with its artifact counts and states that no attention data was
  observed, rather than showing a zero duration without explanation

### Requirement: Report drafts are exportable from the week surface
The week surface SHALL provide export of a report draft for the displayed range, and SHALL indicate
when the export contains model-authored text.

#### Scenario: User exports from the week surface
- **WHEN** the user exports the displayed week
- **THEN** a Markdown draft for that range is produced and the user is told where it was written

#### Scenario: Export runs while summarization is disabled
- **WHEN** the user exports with no summarization engine enabled
- **THEN** the export proceeds using deterministic content and the surface states that no generated
  text is included
