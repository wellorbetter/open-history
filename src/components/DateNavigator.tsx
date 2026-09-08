import { ChevronLeft, ChevronRight, ExternalLink } from 'lucide-react';
import { IconButton } from './IconButton';

export function DateNavigator({
  date,
  isToday,
  onReview,
}: {
  date: string;
  isToday: boolean;
  onReview: () => void;
}) {
  const value = new Date(`${date}T12:00:00`);
  const heading = new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    weekday: 'short',
  }).format(value);

  return (
    <section className="date-nav" aria-label="Timeline date">
      <div>
        <p className="eyebrow">{isToday ? 'Today' : 'Selected day'}</p>
        <h2>{heading}</h2>
      </div>
      <div className="date-actions">
        <IconButton label="Previous day" quiet>
          <ChevronLeft size={18} />
        </IconButton>
        <IconButton label="Next day" quiet disabled={isToday}>
          <ChevronRight size={18} />
        </IconButton>
        <button className="review-button" type="button" onClick={onReview}>
          Review <ExternalLink size={14} aria-hidden="true" />
        </button>
      </div>
    </section>
  );
}
