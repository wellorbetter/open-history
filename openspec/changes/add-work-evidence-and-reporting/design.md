## Context

`build-open-history-desktop` established the canonical event model, privacy evaluator, encrypted
storage, deterministic segmentation, and a consent-gated macOS accessibility collector. The compact
and full surfaces render from fixtures. See `proposal.md` for motivation and `specs/` for observable
behavior.

The collector observes what is on screen. That is sufficient for reading, writing, browsing, and
meetings, and insufficient for work performed through a coding agent, where one terminal window can
remain foreground for hours while files change underneath it. The evidence for that work already
exists locally in two places the collector never looks at: the agent's own session records and the
Git repository.

Both are ordinary local files, so reading them adds no system permission and no network path. Both
are also owned by upstream tools whose on-disk formats are undocumented and versioned independently,
which makes them a durable maintenance surface rather than a one-time integration.

The reporting target changes the shape of the output. A day timeline answers "when"; a status report
answers "what was worked on, across days, per project". These require different aggregation, and the
second is where a model earns its cost, because a week produces hundreds of segments that must
collapse into a handful of sentences.

## Goals / Non-Goals

**Goals:**

- Make work performed through coding agents visible with the same fidelity as window activity.
- Keep every added source local-file or local-API based, adding no outbound network path.
- Resolve structured entities deterministically so that segmentation, search, and digests match on
  identity rather than on formatted strings.
- Produce a report draft that is grounded in linked evidence and that distinguishes observed work
  from inferred narrative.
- Keep a usable result when any single source is absent, excluded, or broken by an upstream change.
- Reach one complete capture-to-report path on macOS before resuming Windows collection work.

**Non-Goals:**

- Reproduce or depend on any remote agent or provider API to read session history.
- Add a built-in network client that transmits activity data, digests, or summaries.
- Track repositories the user has not opted in, or read repository file contents beyond metadata and
  changed-path statistics.
- Read meeting contents, transcripts, or attendee communication; only calendar metadata is used.
- Infer work that has no recorded evidence in any enabled source.
- Replace the day timeline; it remains the detail view behind the week surface.

## Decisions

### 1. Treat work artifacts as an evidence source, not as enrichment of window events

Git state and agent session records enter through the same adapter contract and bounded channel as
platform events, producing canonical envelopes with their own `SourceIdentity` and provenance. They
are not attached as metadata to a window event.

Window events and artifact events describe different things: one is attention, the other is output.
Correlating them at the entity level (same project, overlapping time) preserves both, and lets a
segment report "2h attention, 4 commits, 1 agent thread" without double-counting elapsed time. If
artifacts were enrichment, a day spent entirely inside one agent session would still be anchored to
an event stream that contains almost nothing.

Alternatives considered:

- **Enrich window events with repository state:** simpler storage, but loses artifact evidence
  entirely whenever the window signal is flat, which is exactly the failing case.
- **Post-hoc join at query time:** avoids new event kinds, but repeats expensive scanning per query
  and cannot represent artifact evidence that has no matching window activity.

### 2. Internalize agent session parsing instead of depending on the `cxs` binary

The parsers for agent session storage are ported into an OpenHistory crate rather than invoked as an
external CLI. `wellorbetter/cxs` already implements this against Codex's rollout JSONL and SQLite
state behind ports (`ThreadCatalog`, `RolloutEvidenceSource`) whose `derive(locator, previous)` shape
already supports incremental reads, so the port is a lift rather than a rewrite.

The two tools want different things from the same bytes. `cxs` answers an interactive question once
and exits; OpenHistory needs a long-lived incremental tail that emits canonical events under the
privacy evaluator. Shelling out would put an unversioned text contract, a second binary to install,
and process spawning on the capture path.

Alternatives considered:

- **Depend on the `cxs` executable:** no duplicated parsing, but couples capture to an external
  binary's presence, version, and output format, and provides no incremental streaming.
- **Extract a shared crate consumed by both projects:** correct long-term, but forces coordinated
  releases across two repositories before either behavior is proven; revisit once the format
  surface is stable.

### 3. Read calendar metadata and corroborate it with observed attendance

Meetings are resolved from the operating-system calendar store (EventKit on macOS), producing subject,
scheduled range, and organizer. Observed conferencing-application activity corroborates attendance
and actual duration.

Conferencing window titles are unreliable: they frequently carry a generic product name rather than
the meeting subject. The calendar has the subject in structured form but describes intent, not
attendance, so a cancelled meeting still appears. Each source covers the other's failure: calendar
supplies the subject, observation supplies whether and how long it actually happened.

Alternatives considered:

- **Parse conferencing window titles only:** no calendar permission, but yields unusable subjects for
  the common case and cannot distinguish a cancelled meeting from an attended one.
- **Integrate a specific conferencing vendor's local data:** richer, but vendor-specific, unstable,
  and a much larger privacy surface than calendar metadata.

### 4. Resolve entities deterministically before segmentation

A dedicated stage converts captured strings and artifact records into typed entities — project, file,
host, meeting, agent thread — each with a stable identifier and a confidence value. Segmentation,
search, digests, and correlation consume entities; only entity resolution reads raw strings.

Continuity scoring currently compares window-title tokens, which conflates two files in one project
with two unrelated windows that share a word. Identity must be computed once, testably, and without a
model: a repository path is a fact, not a judgment. Concentrating string handling in one stage also
confines the parsing rules that will need per-application tuning.

Alternatives considered:

- **Infer entities inside segmentation:** fewer moving parts, but makes identity implicit and
  untestable in isolation, and re-derives the same strings for every consumer.
- **Use a local model for entity extraction:** tolerates unusual formats, but makes identity
  non-deterministic, which breaks byte-equivalent replay of segment projections.

### 5. Keep narrative summarization local-only; delegate to the user's agent rather than adding a remote client

Narrative summaries are produced by the on-device engine, or by the user's own agent reading digests
through MCP and writing the summary itself. OpenHistory adds no outbound network client and no
provider credential for activity data. When the user chooses the delegated path, the UI discloses
that the connected agent receives digest content and that its transport is outside OpenHistory's
boundary.

Configuring a hosted provider endpoint inside the application would be the shortest path to good
prose, and it would break the invariant the product is built on. It would also be self-defeating in
the target scenario: status reporting is work activity, and the environments where it matters most
are the ones least able to forward that activity to a third-party endpoint. Delegation keeps the
network decision where the user already makes it explicitly, while the deterministic digest remains
usable with no model at all.

Alternatives considered:

- **Built-in configurable remote endpoint:** best prose with least setup, but contradicts the stated
  invariant, adds credential storage, and makes the product unusable under common workplace policy.
- **On-device engine only:** strictest, but leaves quality bounded by what runs locally and ignores
  that the target user already runs a capable agent beside the app.

### 6. Summarize from digests, never from raw events

Any summarizer, on-device or delegated, receives aggregated digests: per work item, a title, time
range, contributing entities, counts, and a bounded evidence sample. Raw event streams are never
supplied.

A week can hold tens of thousands of raw events and a few dozen digest rows. Digest input keeps cost
and latency bounded regardless of how busy the week was, and it narrows what leaves the local
boundary on the delegated path to material the user can inspect beforehand. It also forces the
deterministic stages to carry the accuracy load, which is where accuracy is testable.

### 7. Make the week the primary reporting unit, with the day as detail

The week surface presents project-level work items with duration, evidence counts, and a summary
slot; selecting one opens the existing day timeline filtered to its evidence. Report export operates
on a range, defaulting to the current week.

The reporting trigger is weekly, and a report is organized by project rather than by clock time.
Presenting seven day-timelines side by side would preserve the day as the organizing unit and leave
the user to do the aggregation the product exists to do.

### 8. Isolate upstream format parsers behind versioned adapters with explicit degradation

Each artifact parser declares the upstream versions it recognizes, reports a source-level diagnostic
when it encounters an unrecognized layout, and yields no events rather than guessing. A broken parser
disables its own source and surfaces a status message; capture, storage, and every other source
continue.

Agent session storage is undocumented and will change. The failure mode must be a visibly degraded
source, never fabricated evidence in a status report and never a stalled capture path.

### 9. Sequence macOS to a complete path before resuming Windows collection

Work proceeds through entity resolution, artifact capture, digests, and the week surface on macOS
before the Windows collection tasks in `build-open-history-desktop` resume.

The product cannot be evaluated until one platform produces a real report from real activity; two
partial platforms produce none. macOS is where the target reporting scenario occurs for the primary
user, and the canonical model keeps the Windows adapter contract unchanged in the meantime.

## Risks / Trade-offs

- **Upstream format churn in agent session storage.** Mitigated by version-declaring parsers,
  explicit degradation, fixtures captured per known upstream version, and isolation in a dedicated
  crate. Accepted as recurring maintenance rather than solved.
- **Report overclaiming.** A narrative summary can assert work that the evidence does not support.
  Mitigated by requiring every work item to carry evidence links, by marking model-authored text as
  inferred, and by keeping the deterministic digest visible beside it.
- **Delegated summarization moves data out of the local boundary.** The user's agent has its own
  transport. Mitigated by disclosure at the point of use, digest-only payloads, and an off-by-default
  posture; it is never silently enabled.
- **Repository reading is sensitive.** Commit subjects can contain confidential material. Mitigated
  by per-repository opt-in, metadata-and-statistics-only reading, and the same exclusion evaluation
  applied to every other source before persistence.
- **Calendar permission fatigue.** A third permission prompt during setup. Mitigated by requesting
  all permissions in one progressive setup step with a single enable-all path, and by keeping the
  product functional when calendar access is declined.
- **Correlation errors across sources.** Attributing a commit to the wrong attention window
  misstates duration. Mitigated by reporting attention and artifact counts as separate quantities and
  never summing them into one elapsed figure.

## Migration Plan

This change extends `build-open-history-desktop` rather than replacing it. That change stays open;
its domain, privacy, storage, segmentation, and macOS collection tasks remain the foundation, and its
Windows and browser sections are unchanged but sequenced after this work.

New event payload variants are additive to the existing versioned envelope, so stored events from the
current build remain readable. No migration of captured data is required. Entity resolution is
introduced ahead of segmentation and backfills entities for stored events on first run; until it has,
segments fall back to existing title-token continuity.

The week surface and report export are additions to the full history window; the compact surface and
day timeline are unchanged.

## Open Questions

- Which agent session formats beyond Codex are in scope for the first release, given that each adds
  an independent parser and an independent breakage surface.
- Whether repository opt-in should default to repositories already observed through window activity,
  or require explicit selection for each one.
- How a work item spanning several weeks should be presented in a single-week report: repeated with
  that week's evidence only, or marked as continuing with cumulative duration.
- Whether declined calendar access should fall back to conferencing-window detection for duration
  only, or omit meetings from reports entirely.
