import type { ActivitySegment, ActivitySource, DashboardSnapshot, WeekDigest } from './types';

const terminal: ActivitySource = {
  id: 'terminal',
  name: 'Terminal',
  kind: 'terminal',
  color: '#252a2d',
};

const editor: ActivitySource = {
  id: 'editor',
  name: 'Visual Studio Code',
  kind: 'editor',
  color: '#5d8fe7',
};

const agent: ActivitySource = {
  id: 'agent',
  name: 'Codex',
  kind: 'agent',
  color: '#eff6f2',
};

const browser: ActivitySource = {
  id: 'browser',
  name: 'Chrome',
  kind: 'browser',
  color: '#e9edf3',
};

const documentSource: ActivitySource = {
  id: 'document',
  name: 'Notes',
  kind: 'document',
  color: '#f5d46b',
};

const revision = (id: string, title: string, summary: string) => [
  {
    id: `${id}-r1`,
    author: 'deterministic' as const,
    createdAt: '2026-09-08T16:50:00+08:00',
    title,
    summary,
  },
];

export const timelineFixture: ActivitySegment[] = [
  {
    id: 'harness-evidence',
    title: 'Harness Evidence Gate and release checks',
    summary: 'Validated the implementation boundary and collected build evidence.',
    start: '16:50',
    end: '17:10',
    durationMinutes: 20,
    observedSeconds: 1200,
    sources: [terminal, editor, agent, browser, documentSource],
    state: 'current',
    mergeCount: 2,
    category: 'Development',
    confidence: 'high',
    origin: 'native',
    revisions: revision('harness-evidence', 'Harness Evidence Gate', 'Validated release evidence.'),
  },
  {
    id: 'adapter-verification',
    title: 'Adapter verification and integration notes',
    summary: 'Compared platform adapter outputs and documented reduced-quality fallbacks.',
    start: '16:40',
    end: '16:50',
    durationMinutes: 10,
    observedSeconds: 600,
    sources: [terminal, editor, agent],
    state: 'complete',
    category: 'Development',
    confidence: 'high',
    origin: 'native',
    revisions: revision('adapter-verification', 'Adapter verification', 'Compared adapter output.'),
  },
  {
    id: 'release-planning',
    title: 'Proactive release planning and acceptance map',
    summary: 'Mapped acceptance scenarios to automated and platform-specific checks.',
    start: '16:20',
    end: '16:40',
    durationMinutes: 20,
    observedSeconds: 1200,
    sources: [terminal, editor, agent, documentSource],
    state: 'complete',
    mergeCount: 2,
    category: 'Planning',
    confidence: 'medium',
    origin: 'native',
    revisions: revision(
      'release-planning',
      'Proactive release planning',
      'Mapped acceptance checks.',
    ),
  },
  {
    id: 'architecture-review',
    title: 'Active intelligence architecture review',
    summary: 'Refined the local-only data flow and read-only agent boundary.',
    start: '16:10',
    end: '16:20',
    durationMinutes: 10,
    observedSeconds: 600,
    sources: [terminal, editor, agent],
    state: 'complete',
    category: 'Architecture',
    confidence: 'high',
    origin: 'native',
    revisions: revision('architecture-review', 'Architecture review', 'Refined local data flow.'),
  },
  {
    id: 'private-gap',
    title: 'Private activity',
    summary: 'Content was excluded before persistence.',
    start: '16:00',
    end: '16:10',
    durationMinutes: 10,
    observedSeconds: 600,
    sources: [],
    state: 'private_gap',
    category: 'Private',
    confidence: 'low',
    origin: 'native',
    revisions: revision('private-gap', 'Private activity', 'Excluded before persistence.'),
  },
  {
    id: 'mcp-collaboration',
    title: 'MCP collaboration and provenance boundaries',
    summary: 'Defined bounded queries and explicit untrusted-content markers.',
    start: '15:50',
    end: '16:00',
    durationMinutes: 10,
    observedSeconds: 600,
    sources: [terminal, editor, agent, browser],
    state: 'complete',
    category: 'Agent access',
    confidence: 'high',
    origin: 'browser',
    revisions: revision('mcp-collaboration', 'MCP collaboration', 'Defined bounded agent queries.'),
  },
];

/**
 * Three weeks demonstrating the states the week surface must distinguish: a week with mixed
 * evidence (attention, commits, an agent session, and a project with artifacts but no observed
 * attention), a week inside the retention window with nothing recorded, and a week the retention
 * boundary has already passed.
 */
export const weekDigestFixtures: WeekDigest[] = [
  {
    rangeStart: '2026-09-14',
    rangeEnd: '2026-09-20',
    hasGeneratedText: false,
    workItems: [
      {
        projectId: 'open-history',
        title: 'open-history',
        attentionMinutes: 95,
        commitCount: 3,
        agentSessionCount: 1,
        meetingCount: 0,
        activeDays: ['2026-09-14', '2026-09-15'],
        evidence: [
          {
            occurredAt: '2026-09-14T09:30:00+08:00',
            kind: { type: 'attention', minutes: 95 },
            segmentId: 'harness-evidence',
          },
          {
            occurredAt: '2026-09-14T11:20:00+08:00',
            kind: {
              type: 'commit',
              commitId: 'be279e13eb48',
              subject: 'feat(entities): add deterministic resolver',
            },
          },
          {
            occurredAt: '2026-09-15T07:32:00+08:00',
            kind: {
              type: 'agentSession',
              threadId: '01JD3K7M4QWERTY',
              summary: 'Added openhistory-digest with range aggregation and Markdown export.',
            },
          },
          {
            occurredAt: '2026-09-15T15:05:00+08:00',
            kind: {
              type: 'commit',
              commitId: '2fa8284855a2',
              subject: 'feat(digest): aggregate evidence into work items',
            },
          },
        ],
      },
      {
        projectId: 'timetrace',
        title: 'timetrace',
        attentionMinutes: 0,
        commitCount: 0,
        agentSessionCount: 2,
        meetingCount: 0,
        activeDays: ['2026-09-16'],
        evidence: [
          {
            occurredAt: '2026-09-16T10:00:00+08:00',
            kind: {
              type: 'agentSession',
              threadId: '01JD4Q9YKSAMPLE',
              summary: 'Fixed idle-detection threshold on sleep/wake.',
            },
          },
        ],
      },
      {
        projectId: 'reading',
        title: 'Reading: docs.rs',
        attentionMinutes: 20,
        commitCount: 0,
        agentSessionCount: 0,
        meetingCount: 0,
        activeDays: ['2026-09-16'],
        evidence: [
          { occurredAt: '2026-09-16T14:00:00+08:00', kind: { type: 'attention', minutes: 20 } },
        ],
      },
    ],
  },
  {
    rangeStart: '2026-09-07',
    rangeEnd: '2026-09-13',
    hasGeneratedText: false,
    workItems: [],
  },
  {
    rangeStart: '2026-08-17',
    rangeEnd: '2026-08-23',
    hasGeneratedText: false,
  },
];

export const dashboardFixture: DashboardSnapshot = {
  status: 'recording',
  selectedDate: '2026-09-08',
  isToday: true,
  current: timelineFixture[0],
  timeline: timelineFixture,
  privacy: {
    rawRetentionHours: 48,
    excludedApplications: ['1Password', 'Keychain Access'],
    localAiEnabled: false,
    localApiEnabled: false,
    mcpEnabled: false,
  },
};
