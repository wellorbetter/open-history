export type CollectionStatus = 'recording' | 'paused' | 'permission_needed' | 'error';

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
  sources: ActivitySource[];
  state: 'current' | 'complete' | 'private_gap';
  mergeCount?: number;
  category: string;
  confidence: 'high' | 'medium' | 'low';
  origin: 'native' | 'browser' | 'imported';
  revisions: SummaryRevision[];
}

export interface PrivacySettings {
  rawRetentionHours: 0 | 12 | 24 | 48;
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
}

export type HistoryDeleteScope = 'last_10_minutes' | 'last_hour' | 'today' | 'all';
