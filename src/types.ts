export type CollectionStatus = 'recording' | 'paused' | 'permission_needed' | 'error';

export type TrayIconStyle = 'monochrome' | 'color';

export type CaptureGranularity = 'application' | 'window' | 'semantic';

export type TimelineBucket = 'five_minutes' | 'ten_minutes' | 'thirty_minutes' | 'one_hour';

export type SourceKind = 'terminal' | 'editor' | 'browser' | 'agent' | 'document' | 'system';

export interface ActivitySource {
  id: string;
  name: string;
  kind: SourceKind;
  color: string;
}

export interface SummaryRevision {
  id: string;
  author: 'deterministic' | 'ai' | 'user';
  engine?: string;
  createdAt: string;
  title: string;
  summary: string;
}

export interface ActivitySegment {
  id: string;
  title: string;
  summary: string;
  start: string;
  end: string;
  durationMinutes: number;
  /** Observed seconds before minute truncation, so sub-minute activity is distinguishable from none. */
  observedSeconds: number;
  sources: ActivitySource[];
  state: 'current' | 'complete' | 'private_gap';
  mergeCount?: number;
  category: string;
  confidence: 'high' | 'medium' | 'low';
  origin: 'native' | 'browser' | 'imported';
  revisions: SummaryRevision[];
}

export interface PrivacySettings {
  excludedApplications: string[];
  localAiEnabled: boolean;
  localApiEnabled: boolean;
  mcpEnabled: boolean;
}

export interface DashboardSnapshot {
  status: CollectionStatus;
  selectedDate: string;
  isToday: boolean;
  current?: ActivitySegment;
  timeline: ActivitySegment[];
  privacy: PrivacySettings;
  /** Bytes the encrypted history occupies on disk, including its write-ahead log. */
  storageBytes?: number;
  /**
   * Whether encrypted storage was open when this snapshot was taken. `false` means nothing has
   * been read yet, which is not the same claim as a day with nothing in it.
   */
  storageReady?: boolean;
}

/** What a coding agent on this machine made of one window's evidence. */
export interface Interpretation {
  /** The command that answered, so the reader knows who is talking. */
  agent: string;
  title: string;
  summary: string;
}

export type HistoryDeleteScope = 'last_10_minutes' | 'last_hour' | 'today' | 'all';

export type WorkEvidenceKind =
  | { type: 'attention'; minutes: number }
  | { type: 'commit'; commitId: string; subject?: string }
  | { type: 'agentSession'; threadId: string; summary?: string }
  | { type: 'meeting'; subject?: string; minutes: number };

export interface WorkEvidence {
  occurredAt: string;
  kind: WorkEvidenceKind;
  /** Day timeline segment this evidence can be opened against, when one was observed. */
  segmentId?: string;
}

export interface WorkItem {
  projectId: string;
  title: string;
  /** Observed attention minutes. Kept separate from artifact counts; never summed with them. */
  attentionMinutes: number;
  commitCount: number;
  agentSessionCount: number;
  meetingCount: number;
  activeDays: string[];
  evidence: WorkEvidence[];
}

export interface WeekDigest {
  rangeStart: string;
  rangeEnd: string;
  /** Absent when the range has no retained data at all, as opposed to zero activity. */
  workItems?: WorkItem[];
  /** True when a summary in this digest was written by a model rather than deterministically. */
  hasGeneratedText: boolean;
}
