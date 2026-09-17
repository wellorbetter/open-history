/**
 * Renders observed attention without overstating or erasing it.
 *
 * Minute truncation alone reports a genuinely empty window and a window holding fifty seconds of
 * observed work identically, as "0 min". Since the number is a claim about evidence, the two cases
 * have to read differently: nothing observed stays "0 min", anything shorter than a minute says so
 * explicitly rather than rounding up to a minute that was never measured.
 */
export function formatObservedDuration(observedSeconds: number): string {
  if (observedSeconds <= 0) return '0 min';
  if (observedSeconds < 60) return '<1 min';
  return `${Math.floor(observedSeconds / 60)} min`;
}

/** The same claim spelled out for screen readers and the inspector header. */
export function describeObservedDuration(observedSeconds: number): string {
  if (observedSeconds <= 0) return 'no observed time';
  if (observedSeconds < 60) return 'less than a minute';
  const minutes = Math.floor(observedSeconds / 60);
  return `${minutes} ${minutes === 1 ? 'minute' : 'minutes'}`;
}
