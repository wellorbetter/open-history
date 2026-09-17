import { describe, expect, it } from 'vitest';
import { describeObservedDuration, formatObservedDuration } from '../src/lib/duration';

describe('observed duration', () => {
  it('separates an empty window from one holding less than a minute', () => {
    // The regression this guards: both used to render "0 min", so a window that had really been
    // observed was indistinguishable from one that had not.
    expect(formatObservedDuration(0)).toBe('0 min');
    expect(formatObservedDuration(50)).toBe('<1 min');
  });

  it('never rounds an unmeasured minute into existence', () => {
    expect(formatObservedDuration(59)).toBe('<1 min');
    expect(formatObservedDuration(60)).toBe('1 min');
    expect(formatObservedDuration(119)).toBe('1 min');
  });

  it('treats a negative reading as no observation rather than as time spent', () => {
    expect(formatObservedDuration(-30)).toBe('0 min');
    expect(describeObservedDuration(-30)).toBe('no observed time');
  });

  it('spells the same claim out for screen readers', () => {
    expect(describeObservedDuration(0)).toBe('no observed time');
    expect(describeObservedDuration(30)).toBe('less than a minute');
    expect(describeObservedDuration(60)).toBe('1 minute');
    expect(describeObservedDuration(600)).toBe('10 minutes');
  });
});
