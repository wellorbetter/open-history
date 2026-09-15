## Purpose

Convert captured strings and artifact records into typed entities with stable identity before
segmentation, so that continuity, search, correlation, and digests match on identity rather than on
formatted text, and so that identity remains deterministic and testable without a model.

## ADDED Requirements

### Requirement: Entities are resolved deterministically and without a model
The system SHALL derive project, file, host, meeting, and agent-thread entities from captured
evidence using deterministic rules only. Repeated resolution of identical input MUST produce
identical entities and identifiers.

#### Scenario: Same input is processed twice
- **WHEN** an identical event is resolved in two separate runs
- **THEN** both runs produce byte-equivalent entity records and identifiers

#### Scenario: No rule matches the input
- **WHEN** a window title matches no known application pattern
- **THEN** the event retains its raw application and window metadata, receives no fabricated entity,
  and is marked as unresolved rather than guessed

### Requirement: Entity identity is stable across sources and presentation changes
An entity identifier SHALL be derived from durable properties such as a repository path, document
path, canonical host, or calendar entry identifier. Formatting changes in a window title MUST NOT
produce a new identifier for the same underlying entity.

#### Scenario: Editor changes its window title format
- **WHEN** the same file is observed under a different title format
- **THEN** it resolves to the same file entity and the same parent project entity

#### Scenario: One project is evidenced by several sources
- **WHEN** window activity, repository commits, and an agent session all reference one repository
- **THEN** all three resolve to the same project entity so that their evidence aggregates together

### Requirement: Resolution reports confidence and never infers beyond recorded evidence
Each resolved entity SHALL carry a confidence value reflecting the strength of its evidence. The
resolver MUST NOT infer a project, file, or meeting that is not present in the input it was given.

#### Scenario: Title contains a partial project reference
- **WHEN** a window title suggests a project name without a corroborating path or repository
- **THEN** the entity is resolved at reduced confidence and marked as title-derived

#### Scenario: Downstream consumer requires high confidence
- **WHEN** a digest is generated with a minimum confidence threshold
- **THEN** entities below the threshold are excluded from work-item attribution while remaining
  visible in the day timeline

### Requirement: Raw captured strings are read only by entity resolution
Segmentation, search indexing, digests, and agent-facing responses SHALL consume resolved entities.
Downstream stages MUST NOT re-parse raw window titles or document paths to determine identity.

#### Scenario: Segmentation scores continuity
- **WHEN** continuity is scored between two adjacent events
- **THEN** scoring uses entity identity and MUST NOT compare raw title tokens to establish identity

#### Scenario: Excluded content reaches the resolver
- **WHEN** an event is excluded by privacy policy
- **THEN** it is excluded before resolution, and no entity is produced or persisted from it
