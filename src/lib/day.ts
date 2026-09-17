/**
 * Local calendar days as the `YYYY-MM-DD` strings the Rust side also speaks.
 *
 * Both surfaces page through days, and both have to agree with the backend about which day a string
 * names, so the arithmetic lives here once rather than being written twice.
 */

/** A `Date` as the local `YYYY-MM-DD` it falls on. */
export function localDay(at: Date) {
  return [
    at.getFullYear(),
    String(at.getMonth() + 1).padStart(2, '0'),
    String(at.getDate()).padStart(2, '0'),
  ].join('-');
}

/**
 * Moves a `YYYY-MM-DD` day by whole days.
 *
 * Arithmetic happens at midday so that a daylight-saving change, which shortens or lengthens one
 * local day, cannot make "the day before" land back on the same date.
 */
export function shiftDay(date: string, days: number) {
  const at = new Date(`${date}T12:00:00`);
  at.setDate(at.getDate() + days);
  return localDay(at);
}
