import { useState } from 'react';
import {
  Bot,
  CalendarX,
  ChevronLeft,
  ChevronRight,
  FileClock,
  GitCommit,
  ShieldCheck,
  Video,
} from 'lucide-react';
import type { WeekDigest, WorkEvidence, WorkItem } from '../types';
import { IconButton } from './IconButton';

// Pinned to 'en-US' throughout this file: the surrounding UI copy is English-only,
// so following the runtime's default locale would mix English labels with a
// differently formatted (or non-English) date on a non-English system.
function weekRangeLabel(rangeStart: string, rangeEnd: string) {
  const format = (value: string) =>
    new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric' }).format(
      new Date(`${value}T12:00:00`),
    );
  return `${format(rangeStart)} – ${format(rangeEnd)}`;
}

export function WeekPane({
  weeks,
  onOpenSegment,
}: {
  weeks: WeekDigest[];
  onOpenSegment: (segmentId: string) => void;
}) {
  const [weekIndex, setWeekIndex] = useState(0);
  const digest = weeks[weekIndex];
  const workItems = digest?.workItems ?? [];
  const [selectedId, setSelectedId] = useState<string | undefined>(workItems[0]?.projectId);
  const selected = workItems.find((item) => item.projectId === selectedId) ?? workItems[0];

  const selectWeek = (nextIndex: number) => {
    setWeekIndex(nextIndex);
    setSelectedId(weeks[nextIndex].workItems?.[0]?.projectId);
  };

  // Nothing in this app aggregates a week yet, so with no digest to show there is nothing to page
  // through and no range to caption. Saying that is the only honest thing this pane can do; the
  // alternative it replaced was rendering the design fixtures as if they were the user's own week.
  if (!digest) {
    return (
      <section className="history-list-pane" aria-label="Week results">
        <header className="history-toolbar">
          <div>
            <p className="eyebrow">Week</p>
            <h1>No weekly digest</h1>
          </div>
        </header>
        <div className="history-results">
          <div className="empty-results">
            <CalendarX size={22} aria-hidden="true" />
            <strong>Weeks are not aggregated yet</strong>
            <span>
              Your day timeline is recorded and kept. Rolling it up into a week — commits, agent
              sessions and meetings per project — is not built, so there is nothing to show here.
            </span>
          </div>
        </div>
      </section>
    );
  }

  return (
    <>
      <section className="history-list-pane" aria-label="Week results">
        <header className="history-toolbar">
          <div>
            <p className="eyebrow">Week</p>
            <h1>{weekRangeLabel(digest.rangeStart, digest.rangeEnd)}</h1>
          </div>
          <div className="toolbar-actions">
            <IconButton
              label="Previous week"
              quiet
              disabled={weekIndex >= weeks.length - 1}
              onClick={() => selectWeek(weekIndex + 1)}
            >
              <ChevronLeft size={18} />
            </IconButton>
            <IconButton
              label="Next week"
              quiet
              disabled={weekIndex <= 0}
              onClick={() => selectWeek(weekIndex - 1)}
            >
              <ChevronRight size={18} />
            </IconButton>
          </div>
        </header>

        <div className="history-results" aria-live="polite">
          {workItems.length ? (
            workItems.map((item) => (
              <WorkItemCard
                key={item.projectId}
                item={item}
                selected={item.projectId === selected?.projectId}
                onSelect={setSelectedId}
              />
            ))
          ) : (
            <div className="empty-results">
              <FileClock size={22} aria-hidden="true" />
              <strong>No activity recorded</strong>
              <span>Nothing was observed for this week.</span>
            </div>
          )}
        </div>
      </section>

      <WorkItemInspector item={selected} onOpenEvidence={onOpenSegment} />
    </>
  );
}

function WorkItemCard({
  item,
  selected,
  onSelect,
}: {
  item: WorkItem;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  return (
    <button
      className={`history-result work-item-result ${selected ? 'is-selected' : ''}`}
      type="button"
      onClick={() => onSelect(item.projectId)}
      aria-pressed={selected}
    >
      <span className="history-result-copy">
        <strong title={item.title}>{item.title}</strong>
        <span>{attentionLabel(item)}</span>
        <small>
          {item.commitCount} commit{item.commitCount === 1 ? '' : 's'} · {item.agentSessionCount}{' '}
          agent session{item.agentSessionCount === 1 ? '' : 's'} · {item.meetingCount} meeting
          {item.meetingCount === 1 ? '' : 's'}
        </small>
      </span>
    </button>
  );
}

function attentionLabel(item: WorkItem) {
  if (item.attentionMinutes === 0) return 'No attention data was observed';
  return `${item.attentionMinutes} min observed`;
}

function WorkItemInspector({
  item,
  onOpenEvidence,
}: {
  item?: WorkItem;
  onOpenEvidence: (segmentId: string) => void;
}) {
  if (!item) {
    return (
      <section className="inspector inspector--empty">
        <FileClock size={26} aria-hidden="true" />
        <strong>Select a work item</strong>
        <span>Its contributing evidence will appear here.</span>
      </section>
    );
  }

  return (
    <section className="inspector" aria-label={`Details for ${item.title}`}>
      <h2>{item.title}</h2>
      <p className="inspector-time">
        Active {item.activeDays.length} day(s): {item.activeDays.join(', ')}
      </p>

      <div className="inspector-section">
        <div className="section-heading">
          <h3>Observed work</h3>
          <span className="local-label">
            <ShieldCheck size={14} aria-hidden="true" /> Local
          </span>
        </div>
        <p>{attentionLabel(item)}</p>
      </div>

      <div className="inspector-section">
        <h3>Evidence</h3>
        <ul className="source-list">
          {item.evidence.map((evidence, index) => (
            <EvidenceRow
              // Evidence has no stable id in this fixture-driven surface; index is fine since the
              // list is not reordered by user action.
              key={index}
              evidence={evidence}
              onOpen={onOpenEvidence}
            />
          ))}
          {!item.evidence.length && <li>No evidence recorded.</li>}
        </ul>
      </div>
    </section>
  );
}

function EvidenceRow({
  evidence,
  onOpen,
}: {
  evidence: WorkEvidence;
  onOpen: (segmentId: string) => void;
}) {
  const when = new Date(evidence.occurredAt).toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
  const { icon, label } = describeEvidence(evidence);

  const content = (
    <>
      <span aria-hidden="true">{icon}</span>
      <span>
        {when} — {label}
      </span>
    </>
  );

  if (evidence.segmentId) {
    return (
      <li>
        <button className="evidence-link" type="button" onClick={() => onOpen(evidence.segmentId!)}>
          {content}
        </button>
      </li>
    );
  }
  return <li className="evidence-row">{content}</li>;
}

function describeEvidence(evidence: WorkEvidence): { icon: React.ReactNode; label: string } {
  switch (evidence.kind.type) {
    case 'attention':
      return {
        icon: <ShieldCheck size={14} />,
        label: `observed activity (${evidence.kind.minutes} min)`,
      };
    case 'commit':
      return {
        icon: <GitCommit size={14} />,
        label: evidence.kind.subject
          ? `commit ${evidence.kind.commitId.slice(0, 12)}: ${evidence.kind.subject}`
          : `commit ${evidence.kind.commitId.slice(0, 12)}`,
      };
    case 'agentSession':
      return {
        icon: <Bot size={14} />,
        label: evidence.kind.summary ? `agent session: ${evidence.kind.summary}` : 'agent session',
      };
    case 'meeting':
      return {
        icon: <Video size={14} />,
        label: evidence.kind.subject
          ? `meeting "${evidence.kind.subject}" (${evidence.kind.minutes} min)`
          : `meeting (${evidence.kind.minutes} min)`,
      };
    default:
      return { icon: null, label: '' };
  }
}
