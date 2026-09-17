import { useEffect, useMemo, useState } from 'react';
import {
  CalendarDays,
  CalendarRange,
  ChevronLeft,
  ChevronRight,
  Clock,
  FileClock,
  Gauge,
  History,
  Palette,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { useDashboard } from '../hooks/useDashboard';
import { bridge } from '../lib/bridge';
import { localDay, shiftDay } from '../lib/day';
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
  // Empty by default, because nothing in this app produces a week digest yet. This used to default
  // to the design fixtures, and since the native window renders `<FullHistoryView />` with no props,
  // every user was shown invented commits, meetings and attention minutes as their own history.
  weeks = [],
}: {
  initial?: DashboardSnapshot;
  weeks?: WeekDigest[];
}) {
  // Undefined means today, and keeps meaning today: a window left open overnight rolls over instead
  // of pinning itself to the day it was opened.
  const [date, setDate] = useState<string>();
  const { snapshot, loading, error, unlocking, retry } = useDashboard(initial, date);

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

  return (
    <LoadedFullHistoryView
      initial={snapshot}
      weeks={weeks}
      onDeleted={retry}
      onSelectDate={setDate}
    />
  );
}

function LoadedFullHistoryView({
  initial,
  weeks,
  onDeleted,
  onSelectDate,
}: {
  initial: DashboardSnapshot;
  weeks: WeekDigest[];
  onDeleted: () => void;
  onSelectDate: (date: string) => void;
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

  // A click on a compact timeline row names a task. Honour it whichever section is open, so the
  // window it raises shows what was clicked rather than what was last looked at.
  useEffect(
    () =>
      bridge.onOpenSegment((segmentId) => {
        setSelectedId(segmentId);
        setSection('history');
      }),
    [],
  );

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
                <IconButton
                  label="Previous day"
                  quiet
                  onClick={() => onSelectDate(shiftDay(initial.selectedDate, -1))}
                >
                  <ChevronLeft size={18} />
                </IconButton>
                <IconButton
                  label="Next day"
                  quiet
                  disabled={initial.isToday}
                  onClick={() => onSelectDate(shiftDay(initial.selectedDate, 1))}
                >
                  <ChevronRight size={18} />
                </IconButton>
                {/* A native date input rather than a button that opens something custom: it is
                    keyboard-navigable, localized, and cannot offer a day that has not happened. */}
                <label className="date-field" title="Choose date">
                  <CalendarDays size={17} aria-hidden="true" />
                  <span className="sr-only">Choose date</span>
                  <input
                    type="date"
                    value={initial.selectedDate}
                    max={localDay(new Date())}
                    onChange={(event) => {
                      if (event.target.value) onSelectDate(event.target.value);
                    }}
                  />
                </label>
              </div>
            </header>
            <label className="search-field">
              <Search size={16} aria-hidden="true" />
              {/* Named for what it does. This filters the day already on screen, in the browser;
                  there is no index behind it and no command that searches other days, so calling it
                  "Search history" sent people looking for last week's phrases here. */}
              <span className="sr-only">Filter this day</span>
              <input
                type="search"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Filter this day's tasks"
              />
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
                  {/* A day with nothing in it and a search that matched nothing are different
                      findings, and only one of them is about the search. */}
                  {query.trim() ? (
                    <>
                      <strong>No matching history</strong>
                      <span>Try a broader task or summary phrase.</span>
                    </>
                  ) : (
                    <>
                      <strong>Nothing was recorded on {dayHeading}</strong>
                      <span>
                        {initial.isToday
                          ? 'Activity appears here as it is observed.'
                          : 'Either collection was off, or this Mac was not in use.'}
                      </span>
                    </>
                  )}
                </div>
              )}
            </div>
          </section>

          <TaskInspector
            segment={selected}
            date={initial.selectedDate}
            isToday={initial.isToday}
            onDelete={() => setDeleteScope('today')}
          />
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
function AgentReading({ segment, date }: { segment: ActivitySegment; date: string }) {
  const [reading, setReading] = useState<Interpretation>();
  const [asking, setAsking] = useState(false);
  const [failed, setFailed] = useState<string>();

  const ask = async () => {
    setAsking(true);
    setFailed(undefined);
    try {
      setReading(await bridge.interpretActivity(segment.id, date));
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

function TaskInspector({
  segment,
  date,
  isToday,
  onDelete,
}: {
  segment?: ActivitySegment;
  date: string;
  isToday: boolean;
  onDelete: () => void;
}) {
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
      <AgentReading key={segment.id} segment={segment} date={date} />

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
          {/* Every revision this app writes is deterministic — nothing edits one, and there is no
              command that could. So it is stated flatly rather than chosen between two labels, one
              of which was unreachable. */}
          <strong>Deterministic revision</strong>
        </div>
        {revision?.createdAt && <span>{new Date(revision.createdAt).toLocaleString('en-US')}</span>}
      </div>

      <footer className="inspector-actions">
        {/* Deletion is expressed as "everything since a moment", so the only day it can remove is
            the current one. Offering it from a past day would delete a different day than the one
            being looked at. */}
        <button
          className="danger-quiet-button"
          type="button"
          onClick={onDelete}
          disabled={!isToday}
          title={isToday ? undefined : "Deleting a past day on its own isn't supported yet"}
        >
          <Trash2 size={15} aria-hidden="true" /> Delete today
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
          description="Nothing is deleted on a schedule yet. What was recorded stays on this device until you delete it."
        >
          {/* This card used to offer an expiry window and claim that raw events aged out after 48
              hours. Nothing swept them: the app never runs a retention pass, so the promise was
              false and the control it was attached to persisted nothing. Saying so is the only
              version of this card that is true today. */}
          <p className="setting-note">
            Never recorded: {snapshot.privacy.excludedApplications.join(', ')}
          </p>
        </SettingsCard>

        {/* An "On-device summaries" card and an "Agent access" card used to sit here, offering
            switches for a local model, a loopback API and an MCP companion. None of the three was
            connected to anything: the checkboxes had no change handler, no command existed to
            persist them, no HTTP listener is built, and the MCP binary only prints a line and
            exits. They are gone rather than disabled, because a disabled switch still promises the
            feature is there and merely turned off. */}

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
          description="Removes the raw events and the task segments built from them."
          danger
        >
          <p className="setting-note">
            {snapshot.storageBytes === undefined
              ? 'Stored on this device.'
              : `Using ${formatBytes(snapshot.storageBytes)} on this device. Deleting is the only
                 thing that removes it.`}
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
