import { useEffect, useMemo, useState } from 'react';
import {
  CalendarDays,
  CalendarRange,
  ChevronLeft,
  Clock,
  Download,
  Ellipsis,
  FileClock,
  Filter,
  Gauge,
  History,
  Palette,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { weekDigestFixtures } from '../fixtures';
import { useDashboard } from '../hooks/useDashboard';
import { bridge } from '../lib/bridge';
import { describeObservedDuration, formatObservedDuration } from '../lib/duration';
import type {
  ActivitySegment,
  CaptureGranularity,
  DashboardSnapshot,
  HistoryDeleteScope,
  Interpretation,
  TimelineBucket,
  TrayIconStyle,
  WeekDigest,
} from '../types';
import { BrandMark } from '../components/BrandMark';
import { DestructiveDialog } from '../components/DestructiveDialog';
import { ErrorState, LoadingState } from '../components/FeedbackState';
import { IconButton } from '../components/IconButton';
import { SourceStack } from '../components/SourceStack';
import { StatusBadge } from '../components/StatusBadge';
import { WeekPane } from '../components/WeekPane';

type FullSection = 'history' | 'week' | 'settings';

export function FullHistoryView({
  initial,
  weeks = weekDigestFixtures,
}: {
  initial?: DashboardSnapshot;
  weeks?: WeekDigest[];
}) {
  const { snapshot, loading, error, unlocking, retry } = useDashboard(initial);

  if (loading || !snapshot || unlocking) {
    return (
      <main className="history-shell history-shell--centered" aria-label="OpenHistory full history">
        {error ? (
          <ErrorState message={error} onRetry={retry} />
        ) : (
          <LoadingState message={unlocking ? 'Unlocking encrypted history…' : undefined} />
        )}
      </main>
    );
  }

  return <LoadedFullHistoryView initial={snapshot} weeks={weeks} onDeleted={retry} />;
}

function LoadedFullHistoryView({
  initial,
  weeks,
  onDeleted,
}: {
  initial: DashboardSnapshot;
  weeks: WeekDigest[];
  onDeleted: () => void;
}) {
  const requestedSegment = new URLSearchParams(window.location.search).get('segment');
  const [section, setSection] = useState<FullSection>('history');
  const [query, setQuery] = useState('');
  const [selectedId, setSelectedId] = useState(requestedSegment ?? initial.current?.id);
  const [deleteScope, setDeleteScope] = useState<HistoryDeleteScope>();
  const [deleted, setDeleted] = useState<Set<string>>(() => new Set());
  const dayHeading = useMemo(
    () =>
      new Intl.DateTimeFormat('en-US', { month: 'long', day: 'numeric' }).format(
        new Date(`${initial.selectedDate}T12:00:00`),
      ),
    [initial.selectedDate],
  );

  const segments = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return initial.timeline.filter(
      (segment) =>
        !deleted.has(segment.id) &&
        (!normalized ||
          segment.title.toLocaleLowerCase().includes(normalized) ||
          segment.summary.toLocaleLowerCase().includes(normalized)),
    );
  }, [deleted, initial.timeline, query]);

  const selected = segments.find((segment) => segment.id === selectedId) ?? segments[0];

  const confirmDelete = async () => {
    if (!deleteScope) return;
    await bridge.deleteHistory(deleteScope);
    if (deleteScope === 'all') setDeleted(new Set(initial.timeline.map((segment) => segment.id)));
    else if (selected) setDeleted((current) => new Set(current).add(selected.id));
    setDeleteScope(undefined);
    // Storage is now smaller and the timeline shorter; pull the real numbers rather than
    // leaving the surface showing what was deleted.
    onDeleted();
  };

  const openSegmentFromWeek = (segmentId: string) => {
    setSelectedId(segmentId);
    setSection('history');
  };

  return (
    <main className="history-shell" aria-label="OpenHistory full history">
      <aside className="sidebar">
        <div className="sidebar-brand">
          <BrandMark size={32} />
          <span>OpenHistory</span>
        </div>
        <nav aria-label="Primary navigation">
          <button
            className={section === 'history' ? 'nav-item is-active' : 'nav-item'}
            type="button"
            onClick={() => setSection('history')}
            aria-current={section === 'history' ? 'page' : undefined}
          >
            <History size={17} aria-hidden="true" /> History
          </button>
          <button
            className={section === 'week' ? 'nav-item is-active' : 'nav-item'}
            type="button"
            onClick={() => setSection('week')}
            aria-current={section === 'week' ? 'page' : undefined}
          >
            <CalendarRange size={17} aria-hidden="true" /> Week
          </button>
          <button
            className={section === 'settings' ? 'nav-item is-active' : 'nav-item'}
            type="button"
            onClick={() => setSection('settings')}
            aria-current={section === 'settings' ? 'page' : undefined}
          >
            <Settings size={17} aria-hidden="true" /> Settings
          </button>
        </nav>
        <div className="sidebar-status">
          <StatusBadge status={initial.status} />
          <span>Stored on this device</span>
        </div>
      </aside>

      {section === 'settings' ? (
        <SettingsPanel
          snapshot={initial}
          onDelete={() => setDeleteScope('all')}
          onChanged={onDeleted}
        />
      ) : section === 'week' ? (
        <WeekPane weeks={weeks} onOpenSegment={openSegmentFromWeek} />
      ) : (
        <>
          <section className="history-list-pane" aria-label="History results">
            <header className="history-toolbar">
              <div>
                <p className="eyebrow">{initial.isToday ? 'Today' : 'Selected day'}</p>
                <h1>{dayHeading}</h1>
              </div>
              <div className="toolbar-actions">
                <IconButton label="Previous day" quiet>
                  <ChevronLeft size={18} />
                </IconButton>
                <IconButton label="Choose date" quiet>
                  <CalendarDays size={17} />
                </IconButton>
              </div>
            </header>
            <label className="search-field">
              <Search size={16} aria-hidden="true" />
              <span className="sr-only">Search history</span>
              <input
                type="search"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search tasks and summaries"
              />
              <Filter size={15} aria-hidden="true" />
            </label>
            <div className="history-results" aria-live="polite">
              {segments.map((segment) => (
                <HistoryResult
                  key={segment.id}
                  segment={segment}
                  selected={segment.id === selected?.id}
                  onSelect={setSelectedId}
                />
              ))}
              {!segments.length && (
                <div className="empty-results">
                  <Search size={22} aria-hidden="true" />
                  <strong>No matching history</strong>
                  <span>Try a broader task or summary phrase.</span>
                </div>
              )}
            </div>
          </section>

          <TaskInspector segment={selected} onDelete={() => setDeleteScope('today')} />
        </>
      )}

      {deleteScope && (
        <DestructiveDialog
          scope={deleteScope}
          onCancel={() => setDeleteScope(undefined)}
          onConfirm={() => void confirmDelete()}
        />
      )}
    </main>
  );
}

function HistoryResult({
  segment,
  selected,
  onSelect,
}: {
  segment: ActivitySegment;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  return (
    <button
      className={`history-result ${selected ? 'is-selected' : ''}`}
      type="button"
      onClick={() => onSelect(segment.id)}
      aria-pressed={selected}
    >
      <span className="history-result-time">
        {segment.start}
        <span aria-hidden="true" />
      </span>
      <span className="history-result-copy">
        <strong title={segment.title}>{segment.title}</strong>
        <span title={segment.summary}>{segment.summary}</span>
        <small>
          {formatObservedDuration(segment.observedSeconds)} · {segment.category}
        </small>
      </span>
      <SourceStack sources={segment.sources} max={3} />
    </button>
  );
}

// Asks a coding agent on this machine what a window was about, on an explicit click.
function AgentReading({ segment }: { segment: ActivitySegment }) {
  const [reading, setReading] = useState<Interpretation>();
  const [asking, setAsking] = useState(false);
  const [failed, setFailed] = useState<string>();

  const ask = async () => {
    setAsking(true);
    setFailed(undefined);
    try {
      setReading(await bridge.interpretActivity(segment.id));
    } catch (reason) {
      setFailed(reason instanceof Error ? reason.message : 'the agent did not answer');
    } finally {
      setAsking(false);
    }
  };

  return (
    <div className="inspector-section">
      <div className="section-heading">
        <h3>What this was about</h3>
        {reading && <span className="local-label">{reading.agent}</span>}
      </div>
      {reading ? (
        <>
          <p>
            <strong>{reading.title}</strong>
          </p>
          <p>{reading.summary}</p>
        </>
      ) : (
        <p className="muted-note">
          Everything above is what was observed. A coding agent already signed in on this Mac can
          read it back to you — which means these window titles are sent to that agent&apos;s
          provider, and only when you ask.
        </p>
      )}
      {failed && <p className="muted-note">Not shown: {failed}.</p>}
      <button
        className="secondary-button"
        type="button"
        onClick={() => void ask()}
        disabled={asking}
      >
        <Sparkles size={15} aria-hidden="true" />{' '}
        {asking ? 'Asking…' : reading ? 'Ask again' : 'Ask the agent on this Mac'}
      </button>
    </div>
  );
}

function TaskInspector({ segment, onDelete }: { segment?: ActivitySegment; onDelete: () => void }) {
  if (!segment) {
    return (
      <section className="inspector inspector--empty">
        <FileClock size={26} aria-hidden="true" />
        <strong>Select a task</strong>
        <span>Its grounded summary and provenance will appear here.</span>
      </section>
    );
  }

  const revision = segment.revisions.at(-1);

  return (
    <section className="inspector" aria-label={`Details for ${segment.title}`}>
      <header className="inspector-header">
        <span className="category-pill">{segment.category}</span>
        <div>
          <IconButton label="More task actions" quiet>
            <Ellipsis size={18} />
          </IconButton>
        </div>
      </header>
      <h2>{segment.title}</h2>
      <p className="inspector-time">
        {segment.start}–{segment.end} · {describeObservedDuration(segment.observedSeconds)}
      </p>

      <div className="inspector-section">
        <div className="section-heading">
          <h3>Grounded summary</h3>
          <span className="local-label">
            <ShieldCheck size={14} aria-hidden="true" /> Local
          </span>
        </div>
        <p>{segment.summary}</p>
      </div>

      {/* Keyed by the row, so selecting another one starts clean: a reading belongs to the stretch
          it was asked about, and carrying it over would caption one day's work with another's. */}
      <AgentReading key={segment.id} segment={segment} />

      <div className="inspector-section">
        <h3>Sources</h3>
        <ul className="source-list">
          {segment.sources.map((source) => (
            <li key={source.id}>
              <SourceStack sources={[source]} max={1} />
              <span>{source.name}</span>
            </li>
          ))}
          {!segment.sources.length && <li>No identifying source data was retained.</li>}
        </ul>
      </div>

      <div className="inspector-section revision-card">
        <div>
          <Sparkles size={15} aria-hidden="true" />
          <strong>
            {revision?.author === 'deterministic' ? 'Deterministic revision' : 'Edited revision'}
          </strong>
        </div>
        <span>
          {revision?.createdAt
            ? new Date(revision.createdAt).toLocaleString('en-US')
            : 'Available offline'}
        </span>
      </div>

      <footer className="inspector-actions">
        <button className="secondary-button" type="button">
          <Download size={15} aria-hidden="true" /> Export
        </button>
        <button className="danger-quiet-button" type="button" onClick={onDelete}>
          <Trash2 size={15} aria-hidden="true" /> Delete
        </button>
      </footer>
    </section>
  );
}

function SettingsPanel({
  snapshot,
  onDelete,
  onChanged,
}: {
  snapshot: DashboardSnapshot;
  onDelete: () => void;
  onChanged: () => void;
}) {
  return (
    <section className="settings-pane">
      <header>
        <p className="eyebrow">Device preferences</p>
        <h1>Settings</h1>
        <p>Collection and local access stay separate, visible, and reversible.</p>
      </header>

      <div className="settings-grid">
        <SettingsCard
          icon={<ShieldCheck size={19} />}
          title="Privacy and retention"
          description={`Raw semantic events expire after ${snapshot.privacy.rawRetentionHours} hours.`}
        >
          <label className="field-label" htmlFor="retention">
            Raw event retention
          </label>
          <select id="retention" defaultValue={snapshot.privacy.rawRetentionHours}>
            <option value="0">Do not retain after summary</option>
            <option value="12">12 hours</option>
            <option value="24">24 hours</option>
            <option value="48">48 hours</option>
          </select>
          <p className="setting-note">
            Excluded by default: {snapshot.privacy.excludedApplications.join(', ')}
          </p>
        </SettingsCard>

        <SettingsCard
          icon={<Sparkles size={19} />}
          title="On-device summaries"
          description="Deterministic summaries always work offline. Local model enrichment is optional."
        >
          <ToggleRow label="Use an on-device model" checked={snapshot.privacy.localAiEnabled} />
        </SettingsCard>

        <SettingsCard
          icon={<FileClock size={19} />}
          title="Agent access"
          description="Approve read-only local clients independently from collection."
        >
          <ToggleRow label="Loopback API" checked={snapshot.privacy.localApiEnabled} />
          <ToggleRow label="MCP companion" checked={snapshot.privacy.mcpEnabled} />
        </SettingsCard>

        <SettingsCard
          icon={<Clock size={19} />}
          title="Timeline grouping"
          description="Activity is grouped into fixed windows so the day reads as blocks, not fragments."
        >
          <TimelineBucketField onChanged={onChanged} />
        </SettingsCard>

        <SettingsCard
          icon={<Gauge size={19} />}
          title="Capture detail"
          description="Less detail records strictly less, and writes less to disk."
        >
          <CaptureGranularityField />
        </SettingsCard>

        <SettingsCard
          icon={<Palette size={19} />}
          title="Menu bar icon"
          description="Choose how the icon in the menu bar is rendered."
        >
          <TrayIconStyleField />
        </SettingsCard>

        <SettingsCard
          icon={<Trash2 size={19} />}
          title="Delete local history"
          description="Remove raw events, summaries, indexes, and managed exports."
          danger
        >
          <p className="setting-note">
            {snapshot.storageBytes === undefined
              ? 'Stored on this device.'
              : `Using ${formatBytes(snapshot.storageBytes)} on this device. Expired raw events are
                 cleared automatically after ${snapshot.privacy.rawRetentionHours} hours.`}
          </p>
          <button className="danger-button" type="button" onClick={onDelete}>
            Delete all history
          </button>
        </SettingsCard>
      </div>
    </section>
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

function TimelineBucketField({ onChanged }: { onChanged: () => void }) {
  const [bucket, setBucket] = useState<TimelineBucket>();

  useEffect(() => {
    let cancelled = false;
    void bridge.getTimelineBucket().then((loaded) => {
      if (!cancelled) setBucket(loaded);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const onChange = (next: TimelineBucket) => {
    setBucket(next);
    // The timeline is grouped server-side, so it has to be refetched to regroup.
    void bridge.setTimelineBucket(next).then(onChanged);
  };

  return (
    <>
      <label className="field-label" htmlFor="timeline-bucket">
        Window length
      </label>
      <select
        id="timeline-bucket"
        value={bucket ?? 'ten_minutes'}
        onChange={(event) => onChange(event.target.value as TimelineBucket)}
      >
        <option value="five_minutes">5 minutes</option>
        <option value="ten_minutes">10 minutes</option>
        <option value="thirty_minutes">30 minutes</option>
        <option value="one_hour">1 hour</option>
      </select>
      <p className="setting-note">
        Durations stay measured: a window shows the time actually observed in it, not its length.
      </p>
    </>
  );
}

function CaptureGranularityField() {
  const [granularity, setGranularity] = useState<CaptureGranularity>();

  useEffect(() => {
    let cancelled = false;
    void bridge.getCaptureGranularity().then((loaded) => {
      if (!cancelled) setGranularity(loaded);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const onChange = (next: CaptureGranularity) => {
    setGranularity(next);
    void bridge.setCaptureGranularity(next);
  };

  return (
    <>
      <label className="field-label" htmlFor="capture-granularity">
        Recorded detail
      </label>
      <select
        id="capture-granularity"
        value={granularity ?? 'window'}
        onChange={(event) => onChange(event.target.value as CaptureGranularity)}
      >
        <option value="application">Application only</option>
        <option value="window">Application and window</option>
        <option value="semantic">Add accessibility roles</option>
      </select>
      <p className="setting-note">
        Application only records which app was in front, and nothing about what was open in it.
      </p>
    </>
  );
}

function TrayIconStyleField() {
  const [style, setStyle] = useState<TrayIconStyle>();

  useEffect(() => {
    let cancelled = false;
    void bridge.getTrayIconStyle().then((loaded) => {
      if (!cancelled) setStyle(loaded);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const onChange = (next: TrayIconStyle) => {
    setStyle(next);
    void bridge.setTrayIconStyle(next);
  };

  return (
    <>
      <label className="field-label" htmlFor="tray-icon-style">
        Icon style
      </label>
      <select
        id="tray-icon-style"
        value={style ?? 'monochrome'}
        onChange={(event) => onChange(event.target.value as TrayIconStyle)}
      >
        <option value="monochrome">System (black &amp; white)</option>
        <option value="color">Brand green</option>
      </select>
      <p className="setting-note">
        System matches native macOS menu bar icons and follows light/dark mode automatically.
      </p>
    </>
  );
}

function SettingsCard({
  icon,
  title,
  description,
  children,
  danger,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  children: React.ReactNode;
  danger?: boolean;
}) {
  return (
    <section className={`settings-card ${danger ? 'settings-card--danger' : ''}`}>
      <div className="settings-card-heading">
        <span aria-hidden="true">{icon}</span>
        <div>
          <h2>{title}</h2>
          <p>{description}</p>
        </div>
      </div>
      <div className="settings-card-body">{children}</div>
    </section>
  );
}

function ToggleRow({ label, checked }: { label: string; checked: boolean }) {
  return (
    <label className="toggle-row">
      <span>{label}</span>
      <input type="checkbox" defaultChecked={checked} />
    </label>
  );
}
