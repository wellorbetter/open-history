## Why

`build-open-history-desktop` builds a window-activity timeline. Two findings change what the first
usable release must deliver.

First, window observation alone is blind to AI-assisted development. When work happens through a
coding agent in a terminal, accessibility capture observes one unchanging window for hours while the
actual intent, changes, and results are recorded in the agent's local session log and in Git. A
timeline built only on window observation reports `Terminal, 3h` for a day that produced a feature.
The capture stack is not wrong; it is watching the wrong surface for this class of work.

Second, resuming an interrupted task is a weaker motivation than it appears. After an interruption
the editor, terminal, and browser usually still hold the context, so little is actually lost. The
durable pain is recall across days and weeks: reconstructing what was worked on for a status report,
where memory is genuinely gone and the present alternative is guessing from commit history. That
recall has a fixed trigger (a report is due) and a measurable failure mode (work that produced no
commits is omitted entirely), which makes it a better target than continuous review.

This change therefore aims the first release at recall and reporting over multi-day ranges, and
demotes window activity from the sole evidence source to one of several. It also keeps the
local-only invariant intact: none of the added sources requires an outbound network path.

## What Changes

- Add artifact-side capture for local Git repository state (commits, branch, changed-file
  statistics) and local AI coding-agent session records (intent, requests, results, state) as
  first-class evidence sources alongside accessibility capture.
- Add calendar context read from the operating-system calendar store so meetings contribute a
  subject, scheduled range, and attendance corroboration instead of an opaque conferencing window.
- Add a deterministic entity-resolution stage that converts window titles, document paths,
  repository state, and session records into structured entities (project, file, host, meeting,
  agent thread) before segmentation, replacing downstream matching on raw title strings.
- Restructure understanding into three explicit stages: deterministic entity resolution,
  deterministic aggregation into segments, and optional narrative summarization that consumes
  aggregated digests rather than raw events.
- Add range digests that aggregate segments across days into project-level work items with evidence
  links, and export a report draft from them.
- Add a week surface to the full history window as the primary review and reporting view; the
  existing day timeline becomes the detail view.
- Preserve the local-only invariant for narrative summarization: it runs on device, or is delegated
  to the user's own agent through MCP. OpenHistory gains no outbound network client for activity
  data, summaries, or diagnostics.
- Scope agent access by default to a recent time window and to segment-level records rather than raw
  events, with widening as an explicit user action.
- Extend privacy controls to the added sources: per-repository opt-in, calendar exclusion, and
  agent-session redaction before persistence.
- Sequence macOS through a complete capture-to-report path before Windows collection work resumes;
  the Windows tasks in `build-open-history-desktop` are unchanged but follow this path.

## Capabilities

### New Capabilities

- `work-artifact-capture`: Consent-scoped observation of local Git repositories and local AI
  coding-agent session records, normalized into canonical evidence with provenance.
- `calendar-context`: Read-only operating-system calendar access, meeting entities, and corroboration
  of scheduled meetings against observed attendance.
- `entity-resolution`: Deterministic extraction of structured entities from captured strings and
  artifact records, with stable identity, confidence, and no inference beyond recorded evidence.
- `reporting-digest`: Multi-day aggregation into project-level work items, week review surface data,
  and exportable report drafts grounded in linked evidence.

### Modified Capabilities

- `summary-generation`: Adds the three-stage boundary, requires digest-level rather than raw-event
  input, and adds delegated summarization through MCP as a disclosed alternative to the on-device
  engine.
- `desktop-timeline`: Adds the week review surface and cross-source evidence presentation.
- `agent-history-access`: Adds default time and granularity scoping, plus digest-oriented read
  operations.
- `privacy-and-retention`: Adds repository, calendar, and agent-session exclusions, and defines
  retention for artifact-derived evidence.

## Impact

- Adds Rust crates for artifact adapters and entity resolution, and extends the domain event model
  with artifact and calendar payload variants. Reuses the existing bounded channel, privacy
  evaluator, and encrypted storage without changing their contracts.
- Introduces parsers for external local formats (agent session logs, Git metadata) that are owned by
  upstream tools and can change without notice. These are isolated so that a format break degrades
  one source rather than the capture path.
- Requires one additional macOS permission (Calendar) requested in the existing setup flow; Git and
  agent-session reading use ordinary file access and need no new system permission.
- Adds a week surface and report-draft export to the full history window, reusing the existing design
  system and component set.
- Adds fixtures for agent session logs, Git repository states, and calendar entries, plus tests that
  assert cross-source corroboration and that excluded repositories and calendars never reach storage.
