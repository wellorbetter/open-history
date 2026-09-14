import { useMemo, useState } from 'react';
import {
  CalendarDays,
  ChevronLeft,
  Download,
  Ellipsis,
  FileClock,
  Filter,
  History,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { dashboardFixture } from '../fixtures';
import { bridge } from '../lib/bridge';
import type { ActivitySegment, DashboardSnapshot, HistoryDeleteScope } from '../types';
import { BrandMark } from '../components/BrandMark';
import { DestructiveDialog } from '../components/DestructiveDialog';
import { IconButton } from '../components/IconButton';
import { SourceStack } from '../components/SourceStack';
import { StatusBadge } from '../components/StatusBadge';

type FullSection = 'history' | 'settings';

export function FullHistoryView({ initial = dashboardFixture }: { initial?: DashboardSnapshot }) {
  const requestedSegment = new URLSearchParams(window.location.search).get('segment');
  const [section, setSection] = useState<FullSection>('history');
  const [query, setQuery] = useState('');
  const [selectedId, setSelectedId] = useState(requestedSegment ?? initial.current?.id);
  const [deleteScope, setDeleteScope] = useState<HistoryDeleteScope>();
  const [deleted, setDeleted] = useState<Set<string>>(() => new Set());

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
        <SettingsPanel snapshot={initial} onDelete={() => setDeleteScope('all')} />
      ) : (
        <>
          <section className="history-list-pane" aria-label="History results">
            <header className="history-toolbar">
              <div>
                <p className="eyebrow">Today</p>
                <h1>September 8</h1>
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
          {segment.durationMinutes} min · {segment.category}
        </small>
      </span>
      <SourceStack sources={segment.sources} max={3} />
    </button>
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
        {segment.start}–{segment.end} · {segment.durationMinutes} minutes
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
            ? new Date(revision.createdAt).toLocaleString()
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
}: {
  snapshot: DashboardSnapshot;
  onDelete: () => void;
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
          icon={<Trash2 size={19} />}
          title="Delete local history"
          description="Remove raw events, summaries, indexes, and managed exports."
          danger
        >
          <button className="danger-button" type="button" onClick={onDelete}>
            Delete all history
          </button>
        </SettingsCard>
      </div>
    </section>
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
