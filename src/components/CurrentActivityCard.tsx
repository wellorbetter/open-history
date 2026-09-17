import { ArrowRight } from 'lucide-react';
import type { ActivitySegment } from '../types';
import { formatObservedDuration } from '../lib/duration';
import { SourceStack } from './SourceStack';

export function CurrentActivityCard({
  segment,
  onOpen,
}: {
  segment?: ActivitySegment;
  onOpen: (segmentId?: string) => void;
}) {
  if (!segment) {
    return (
      <section className="current-card current-card--empty" aria-label="Current activity">
        <p className="eyebrow">Now</p>
        <h2>No active task</h2>
        <p>OpenHistory is ready when activity resumes.</p>
      </section>
    );
  }

  return (
    <button className="current-card" type="button" onClick={() => onOpen(segment.id)}>
      <span className="current-copy">
        <span className="eyebrow">Now</span>
        <span className="current-title" title={segment.title}>
          {segment.title}
        </span>
        <span className="current-meta">
          {segment.start}–{segment.end} · {formatObservedDuration(segment.observedSeconds)}
        </span>
      </span>
      <span className="current-actions">
        <SourceStack sources={segment.sources} max={3} />
        <span className="source-count">{segment.sources.length} sources</span>
      </span>
      <span className="current-open" aria-hidden="true">
        <ArrowRight size={16} />
      </span>
    </button>
  );
}
