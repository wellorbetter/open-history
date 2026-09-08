import { EyeOff } from 'lucide-react';
import type { ActivitySegment } from '../types';
import { SourceStack } from './SourceStack';

export function TimelineRow({
  segment,
  onOpen,
}: {
  segment: ActivitySegment;
  onOpen: (segmentId: string) => void;
}) {
  const isPrivate = segment.state === 'private_gap';

  return (
    <li className="timeline-item">
      <span className="timeline-time">{segment.start}</span>
      <span className="timeline-rail" aria-hidden="true">
        <span className={`timeline-dot ${segment.state === 'current' ? 'is-current' : ''}`} />
      </span>
      <button
        className={`timeline-row ${isPrivate ? 'timeline-row--private' : ''}`}
        type="button"
        onClick={() => onOpen(segment.id)}
        aria-label={`${segment.title}, ${segment.durationMinutes} minutes`}
      >
        <span className="timeline-title" title={segment.title}>
          {isPrivate && <EyeOff size={14} aria-hidden="true" />}
          {segment.title}
        </span>
        <span className="timeline-sources">
          <SourceStack sources={segment.sources} max={3} />
        </span>
        <span className="timeline-result">
          {segment.mergeCount ? `${segment.mergeCount} merged` : `${segment.durationMinutes} min`}
        </span>
      </button>
    </li>
  );
}
