## 1. Entity Resolution Foundation

- [x] 1.1 Define the typed entity model for project, file, host, meeting, and agent-thread entities with stable identifiers, confidence, and provenance; verify identical input resolves to byte-equivalent entities across runs and process restarts.
- [x] 1.2 Implement deterministic resolution rules for editor, terminal, browser, and document window metadata; verify a fixture matrix of representative applications resolves expected entities and that unmatched titles are marked unresolved rather than guessed.
- [x] 1.3 Implement identifier derivation from durable properties (repository path, document path, canonical host, calendar entry identifier); verify a changed window-title format and a changed display name both resolve to the unchanged identifier.
- [x] 1.4 Run resolution at the capture boundary immediately after privacy evaluation, emitting entity identifiers on the event while the raw document path is discarded before persistence; verify excluded events produce no entity, that no raw path reaches storage, and that segmentation no longer compares raw title tokens to establish identity. (`canonical_events` now takes a `Resolver` and calls it from `resolve_project_id`, which reads `current.document` once and returns only the resolved entity id string; the macOS collector threads a `Resolver` through `run_collector`. Verified in `openhistory-platform`'s tests; the macOS-only collection path itself is not compiled or tested on this machine — see CI note below.)
- [x] 1.5 Rewire continuity scoring to consume entity identity with a title-token fallback for unresolved events; verify existing segmentation fixtures still produce byte-equivalent projections and that two files in one project now score as continuous. (Continuity scoring already keyed on `project_id` equality before this change, so entity resolution improved it for free once wired in by 1.4; added the title fallback for when neither side resolves a project, with a project match always outscoring it.)
- [ ] 1.6 Implement backfill of entities for previously stored events on first run; verify backfill is resumable, bounded in batch size, and leaves segments usable while it is incomplete.

## 2. Repository Evidence

- [ ] 2.1 Implement per-repository opt-in storage, suggestion of observed-but-unadded repositories, and removal that stops collection; verify an unadded repository referenced by window activity produces no evidence and that removal deletes prior evidence.
- [x] 2.2 Implement metadata-only repository reading for commits, branch, and changed-path statistics; verify no working-tree content, diff hunk, staged content, or remote credential is read, and that a repository with uncommitted secrets yields none of them.
- [x] 2.3 Emit canonical repository events with independent source identity through the existing bounded channel; verify evidence is attributed separately from platform capture and that elapsed time is counted once when both cover the same period.
- [x] 2.4 Apply path-pattern exclusion to changed paths before persistence; verify excluded paths never reach storage, search index, or diagnostics while the commit retains a reduced-coverage indicator. (`apply_path_exclusions` uses substring matching, not glob patterns; revisit if a pattern language is needed.)
- [ ] 2.5 Implement incremental reads anchored to the last observed commit per repository; verify force-push, rebase, branch switch, shallow clone, and detached-head fixtures produce no duplicate or fabricated evidence.

## 3. Agent Session Evidence

- [ ] 3.1 Port the Codex rollout JSONL and SQLite state parsers into a dedicated crate behind a catalog/derive port with incremental position tracking; verify derived intent, latest request, result, and state match golden fixtures captured from known upstream versions. (Deviation: `openhistory-agent-sessions` is a new implementation covering the JSONL rollout format's two known dialects, not a port of `cxs`; SQLite session state is not read. Revisit before relying on this for anything beyond the JSONL path.)
- [x] 3.2 Implement upstream version declaration, unrecognized-layout detection, and per-record skip; verify an unrecognized store emits no events with a degraded status while other sources continue, and that one malformed record does not discard its siblings. (Per-record and per-line recognition is implemented; a store-level degraded status surfaced to the source-visibility UI is not — there is no UI yet.)
- [x] 3.3 Implement append-only incremental derivation and truncation/rewrite recovery; verify a growing session parses only appended content and a rewritten record re-derives without duplicating represented activity.
- [ ] 3.4 Implement credential redaction over session text before persistence; verify automated secret scanning of the database and diagnostics finds no token, key, or authorization header from redaction fixtures.
- [ ] 3.5 Emit canonical agent-session events and correlate them to project entities; verify a session with no corresponding window activity is still represented and marked as having no observed attention data.

## 4. Calendar Context

- [ ] 4.1 Implement macOS calendar permission request within the existing progressive setup step and a single enable-all path across Accessibility and Calendar; verify declined and revoked permission leave every other capability functional.
- [ ] 4.2 Implement read-only metadata reading for subject, range, all-day status, organizer, calendar name, and response; verify no attachment, note, transcript, or attendee detail beyond organizer is read and that no calendar entry is created or modified.
- [ ] 4.3 Implement calendar exclusion and private-entry handling; verify excluded calendars contribute nothing even when overlapping observed conferencing activity, and that private entries appear as busy periods without subjects unless opted in.
- [ ] 4.4 Implement meeting entity resolution and attendance corroboration against observed conferencing activity; verify cancelled, overrunning, and unscheduled-meeting fixtures produce the specified attendance, observed duration, and absent-subject behavior.

## 5. Range Digest

- [x] 5.1 Implement range aggregation of segments into project-level work items with deterministic titles, attention duration, contributing entities, artifact counts, and active days; verify repeated aggregation of an unchanged range is byte-equivalent including ordering. (`openhistory-digest` aggregates a flat `EvidenceItem` list, not `TaskSegment` directly, since segmentation is not yet entity-aware — see task 1.4.)
- [ ] 5.2 Implement evidence linking from work items to segments, commits, sessions, and meetings; verify every work item resolves to openable evidence and that deleted evidence disappears from the next generation without leaving an orphaned copy.
- [ ] 5.3 Implement confidence thresholding for work-item attribution; verify entities below the threshold are excluded from attribution while remaining visible in the day timeline.
- [ ] 5.4 Implement gap representation for excluded, expired, and uncollected periods; verify gaps are never reconstructed, estimated, or silently closed.
- [ ] 5.5 Verify digest generation performs no outbound request and remains available with no summarization engine configured; verify with a network-deny test over a full week fixture.

## 6. Week Review Surface and Report Export

- [ ] 6.1 Build the week surface in the full history window listing work items with duration, sources, and summary slot, reusing existing design tokens and components; verify deterministic ordering, empty-week versus outside-retention states, and week navigation.
- [ ] 6.2 Implement drill-down from work item to evidence to the positioned day timeline; verify selecting a commit opens the day and segment containing it.
- [ ] 6.3 Present attention duration and artifact counts as distinct values; verify an artifact-only work item states that no attention data was observed rather than showing an unexplained zero.
- [ ] 6.4 Implement Markdown report-draft export for the displayed range with work items, durations, and evidence references; verify golden files across time zones, empty ranges, gap periods, and user-edited summaries.
- [ ] 6.5 Mark generated text distinctly from deterministic content in surface and export; verify an export with summarization disabled states that no generated text is included.
- [ ] 6.6 Run accessibility and platform-variant review of the week surface; verify contrast, focus order, keyboard navigation, long titles, many sources, and reduced-transparency rendering.

## 7. Summarization Boundary

- [ ] 7.1 Implement digest-bounded summarizer input with deterministic evidence sampling; verify request size does not scale with raw event count on a high-volume week and that truncation is disclosed in the summary.
- [ ] 7.2 Reject remote provider endpoints and credentials for summarization; verify configuration attempts are refused with an explanation and that no remote transport or credential store exists on the summarization path.
- [ ] 7.3 Implement delegated summarization through the MCP surface with pre-use disclosure, explicit opt-in, and revocation; verify delegation is disabled by default and that no prompt, digest, or evidence leaves the application while it is disabled.
- [ ] 7.4 Extend attribution to record which engine produced a summary and the source digest revision; verify deterministic, on-device, and delegated revisions remain distinguishable and recoverable.
- [ ] 7.5 Extend untrusted-evidence handling to repository and agent-session text in prompt assembly; verify directive text in commit subjects and session messages cannot alter instructions or request tool execution.

## 8. Agent Access Scoping

- [ ] 8.1 Implement default recent-window and segment-level scoping for agent-facing reads; verify an unbounded request returns the default window and states the range covered.
- [ ] 8.2 Refuse raw-event access without a granting approval; verify refusal carries an explanation and that segment-level records remain available.
- [ ] 8.3 Implement per-client scope widening with recorded approval and independent revocation; verify widening affects only the approving client and that revocation takes effect on the next request.
- [ ] 8.4 Expose range digests through the agent surface with bounded size; verify a week digest is sufficient for summarization and that gaps and exclusions are represented without leaking excluded content.
- [ ] 8.5 Apply untrusted-content and provenance markers to digest and evidence responses; verify malicious source fixtures remain data and cannot modify tool schemas or invoke mutations.

## 9. Source Visibility and Retention

- [ ] 9.1 Build the source status surface showing enabled, degraded, and last-evidence state per source; verify a degraded parser is shown with its reason while other sources show as active, and that excluded content never appears in this surface.
- [ ] 9.2 Distinguish "no activity" from "not collecting" for every source; verify the distinction holds for a disabled source, a permission-revoked source, and an idle period.
- [ ] 9.3 Apply exclusion changes retroactively to stored artifact and calendar evidence; verify excluding a repository or calendar stops collection and removes prior evidence through existing deletion controls.
- [ ] 9.4 Implement retention for artifact-derived and calendar-derived evidence consistent with existing raw and derived retention; verify expired raw detail is not reconstructed by digests and that stored digests hold no copy of removed evidence.

## 10. macOS End-to-End Validation

- [ ] 10.1 Run the complete macOS journey from consent through accessibility, repository, agent-session, and calendar capture, entity resolution, segmentation, weekly digest, week surface review, and report export; verify every scenario in this change maps to a passing automated test.
- [ ] 10.2 Validate a representative real week on the primary development machine covering agent-driven development, meetings, and reading-only work; verify the exported draft names every project actually worked on and omits no work item with recorded evidence.
- [ ] 10.3 Measure added CPU, memory, database growth, and digest latency from the new sources against the existing idle targets; verify targets hold or document and approve a measured exception.
- [ ] 10.4 Test degradation paths: unrecognized agent format, revoked calendar permission, removed repository, and corrupted session record; verify each degrades one source visibly without stalling capture or fabricating evidence.
- [ ] 10.5 Confirm Windows collection tasks in `build-open-history-desktop` resume only after 10.1 and 10.2 pass; verify the canonical event model and adapter contract required no Windows-side change during this work.
