import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { dashboardFixture } from '../fixtures';
import type {
  CaptureGranularity,
  CollectionStatus,
  DashboardSnapshot,
  HistoryDeleteScope,
  Interpretation,
  TimelineBucket,
  TrayIconStyle,
} from '../types';

const TRAY_ICON_STYLE_FIXTURE_KEY = 'openhistory-tray-icon-style-fixture';
const CAPTURE_GRANULARITY_FIXTURE_KEY = 'openhistory-capture-granularity-fixture';
const TIMELINE_BUCKET_FIXTURE_KEY = 'openhistory-timeline-bucket-fixture';

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const isTauri = () => typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__);

const delay = (milliseconds = 80) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds));

export const bridge = {
  /**
   * Reads one local day's projection. `date` is `YYYY-MM-DD`; omitted means today.
   *
   * The fixture route answers with today's fixture whatever day is asked for, and says so through
   * `selectedDate` so a surface never captions it with a day it does not hold.
   */
  async snapshot(date?: string): Promise<DashboardSnapshot> {
    if (isTauri()) return invoke<DashboardSnapshot>('get_dashboard', { date });
    await delay();
    return structuredClone(dashboardFixture);
  },

  async setCollectionStatus(status: CollectionStatus): Promise<CollectionStatus> {
    if (isTauri()) return invoke<CollectionStatus>('set_collection_status', { status });
    await delay(40);
    return status;
  },

  async openHistory(segmentId?: string): Promise<void> {
    if (isTauri()) {
      await invoke('open_history_window', { segmentId });
      return;
    }
    const target = new URL(window.location.href);
    target.searchParams.set('surface', 'history');
    if (segmentId) target.searchParams.set('segment', segmentId);
    window.history.pushState({}, '', target);
    window.dispatchEvent(new PopStateEvent('popstate'));
  },

  /**
   * Asks a coding agent installed on this Mac what one timeline row was about.
   *
   * Only ever called from an explicit click: the agent is a CLI the user signed in themselves, and
   * it sends the row's evidence to that agent's vendor. Outside the native app there is no agent to
   * ask, so this rejects rather than inventing an answer the fixtures could not have produced.
   */
  async interpretActivity(segmentId: string, date?: string): Promise<Interpretation> {
    if (isTauri()) return invoke<Interpretation>('interpret_activity', { segmentId, date });
    await delay(200);
    throw new Error('a coding agent can only be asked from the desktop app');
  },

  /** Deletes the given span from encrypted storage, returning how many raw events were removed. */
  async deleteHistory(scope: HistoryDeleteScope): Promise<number> {
    if (isTauri()) return invoke<number>('delete_history', { scope });
    await delay(80);
    return 0;
  },

  /**
   * Subscribes to the task the tray asked the history window to show, returning an unsubscribe.
   *
   * The backend has always emitted this when a compact row is clicked, and nothing listened, so the
   * window came up on whatever happened to be selected before — the one task the user did not ask
   * for. Outside the native app there is no event bus and the surface reads the id from the URL
   * instead, so this subscribes to nothing and says so by returning a no-op.
   */
  onOpenSegment(handler: (segmentId: string) => void): () => void {
    if (!isTauri()) return () => {};
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen<string>('open-segment', (event) => handler(event.payload)).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  },

  /**
   * Reads the menu bar icon style.
   *
   * There is no tray icon outside the native app, so the browser-only fixture route persists the
   * choice in `localStorage` purely so the setting is exercisable there too.
   */
  async getTrayIconStyle(): Promise<TrayIconStyle> {
    if (isTauri()) return invoke<TrayIconStyle>('get_tray_icon_style');
    await delay(20);
    const stored = window.localStorage.getItem(TRAY_ICON_STYLE_FIXTURE_KEY);
    return stored === 'color' ? 'color' : 'monochrome';
  },

  async setTrayIconStyle(style: TrayIconStyle): Promise<TrayIconStyle> {
    if (isTauri()) return invoke<TrayIconStyle>('set_tray_icon_style', { style });
    await delay(40);
    window.localStorage.setItem(TRAY_ICON_STYLE_FIXTURE_KEY, style);
    return style;
  },

  /** Reads how much detail collection records per observation. */
  async getCaptureGranularity(): Promise<CaptureGranularity> {
    if (isTauri()) return invoke<CaptureGranularity>('get_capture_granularity');
    await delay(20);
    const stored = window.localStorage.getItem(CAPTURE_GRANULARITY_FIXTURE_KEY);
    return stored === 'application' || stored === 'semantic' ? stored : 'window';
  },

  async setCaptureGranularity(granularity: CaptureGranularity): Promise<CaptureGranularity> {
    if (isTauri()) return invoke<CaptureGranularity>('set_capture_granularity', { granularity });
    await delay(40);
    window.localStorage.setItem(CAPTURE_GRANULARITY_FIXTURE_KEY, granularity);
    return granularity;
  },

  /** Reads the fixed window the day timeline is grouped into. */
  async getTimelineBucket(): Promise<TimelineBucket> {
    if (isTauri()) return invoke<TimelineBucket>('get_timeline_bucket');
    await delay(20);
    const stored = window.localStorage.getItem(TIMELINE_BUCKET_FIXTURE_KEY);
    return stored === 'five_minutes' || stored === 'thirty_minutes' || stored === 'one_hour'
      ? stored
      : 'ten_minutes';
  },

  async setTimelineBucket(bucket: TimelineBucket): Promise<TimelineBucket> {
    if (isTauri()) return invoke<TimelineBucket>('set_timeline_bucket', { bucket });
    await delay(40);
    window.localStorage.setItem(TIMELINE_BUCKET_FIXTURE_KEY, bucket);
    return bucket;
  },
};
