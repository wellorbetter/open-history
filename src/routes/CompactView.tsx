import { useState } from 'react';
import { History, Pause, Play, RotateCw } from 'lucide-react';
import { bridge } from '../lib/bridge';
import { useDashboard } from '../hooks/useDashboard';
import type { DashboardSnapshot } from '../types';
import { BrandMark } from '../components/BrandMark';
import { CurrentActivityCard } from '../components/CurrentActivityCard';
import { DateNavigator } from '../components/DateNavigator';
import { ErrorState, LoadingState } from '../components/FeedbackState';
import { IconButton } from '../components/IconButton';
import { StatusBadge } from '../components/StatusBadge';
import { TimelineRow } from '../components/TimelineRow';

export function CompactView({ initial }: { initial?: DashboardSnapshot }) {
  // Undefined means today, and keeps meaning today, so a tray panel left open overnight rolls over
  // instead of pinning itself to the day it was opened.
  const [date, setDate] = useState<string>();
  const { snapshot, loading, error, updatedAt, unlocking, actionError, toggleCollection, retry } =
    useDashboard(initial, date);

  // An unlocked-but-unread history must not be drawn as an empty day: the timeline below would
  // read as "nothing happened", when in fact nothing has been looked at yet.
  if (loading || !snapshot || unlocking) {
    return (
      <main className="compact-shell compact-shell--centered">
        {error ? (
          <ErrorState message={error} onRetry={retry} />
        ) : (
          <LoadingState message={unlocking ? 'Unlocking encrypted history…' : undefined} />
        )}
      </main>
    );
  }

  // A permission-needed status is retryable: clicking again re-requests the system permission
  // (showing the trust prompt if it's still missing) rather than leaving the user stuck with no
  // path forward once collection has failed once.
  const canToggle = snapshot.status !== 'error';
  const isRecording = snapshot.status === 'recording';
  const needsPermission = snapshot.status === 'permission_needed';

  return (
    <main className="compact-shell" aria-label="OpenHistory compact timeline">
      <header className="compact-header">
        <div className="brand-lockup">
          <BrandMark />
          <span className="brand-name">OpenHistory</span>
        </div>
        <div className="header-actions">
          <StatusBadge status={snapshot.status} />
          <IconButton
            label={
              needsPermission
                ? 'Grant Accessibility permission'
                : isRecording
                  ? 'Pause collection'
                  : 'Resume collection'
            }
            onClick={() => void toggleCollection()}
            disabled={!canToggle}
          >
            {isRecording ? <Pause size={17} /> : <Play size={17} />}
          </IconButton>
          {/* Labelled for where it goes. It used to say "Open settings" behind a gear, but it only
              ever opens the history window on its timeline — the user had to find Settings from
              there themselves. */}
          <IconButton label="Open full history" onClick={() => void bridge.openHistory()}>
            <History size={17} />
          </IconButton>
        </div>
      </header>

      {actionError && (
        <p className="action-error" role="alert">
          {actionError}
        </p>
      )}

      <CurrentActivityCard
        segment={snapshot.current}
        onOpen={(id) => void bridge.openHistory(id)}
      />

      <DateNavigator
        date={snapshot.selectedDate}
        isToday={snapshot.isToday}
        onReview={() => void bridge.openHistory()}
        onSelectDate={setDate}
      />

      <section
        className="timeline-scroll"
        aria-label="Daily task timeline"
        data-testid="timeline-scroll"
      >
        {snapshot.timeline.length ? (
          <ol className="timeline-list">
            {snapshot.timeline.map((segment) => (
              <TimelineRow
                key={segment.id}
                segment={segment}
                onOpen={(id) => void bridge.openHistory(id)}
              />
            ))}
          </ol>
        ) : (
          <div className="empty-timeline">
            <RotateCw size={20} aria-hidden="true" />
            {/* Now that past days are reachable here, "yet" would be wrong on all of them: a day
                with nothing in it is a finished fact, not a wait. */}
            {snapshot.isToday ? (
              <>
                <strong>No activity yet</strong>
                <span>Your work history appears here as it is observed.</span>
              </>
            ) : (
              <>
                <strong>Nothing was recorded</strong>
                <span>Either collection was off, or this Mac was not in use.</span>
              </>
            )}
          </div>
        )}
      </section>

      <footer className="compact-footer">
        {/* No expiry is claimed, because nothing expires: the app runs no retention pass, so the
            only thing that removes history is deleting it. */}
        <span>Local-only · kept until you delete it</span>
        {updatedAt && (
          <span aria-label={`Last synchronized at ${updatedAt.toLocaleTimeString('en-US')}`}>
            Updated{' '}
            {updatedAt.toLocaleTimeString('en-US', {
              hour: '2-digit',
              minute: '2-digit',
              hour12: false,
            })}
          </span>
        )}
      </footer>
    </main>
  );
}
