import { Pause, Play, RotateCw, Settings2 } from 'lucide-react';
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
  const { snapshot, loading, error, updatedAt, toggleCollection, retry } = useDashboard(initial);

  if (loading || !snapshot) {
    return (
      <main className="compact-shell compact-shell--centered">
        {error ? <ErrorState message={error} onRetry={retry} /> : <LoadingState />}
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
          <IconButton label="Open settings" onClick={() => void bridge.openHistory()}>
            <Settings2 size={17} />
          </IconButton>
        </div>
      </header>

      <CurrentActivityCard
        segment={snapshot.current}
        onOpen={(id) => void bridge.openHistory(id)}
      />

      <DateNavigator
        date={snapshot.selectedDate}
        isToday={snapshot.isToday}
        onReview={() => void bridge.openHistory()}
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
            <strong>No activity yet</strong>
            <span>Your consented work history will appear here.</span>
          </div>
        )}
      </section>

      <footer className="compact-footer">
        <span>Local-only · raw events expire in {snapshot.privacy.rawRetentionHours}h</span>
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
