import { invoke } from '@tauri-apps/api/core';
import { dashboardFixture } from '../fixtures';
import type {
  CollectionStatus,
  DashboardSnapshot,
  HistoryDeleteScope,
  TrayIconStyle,
  WeekDigest,
} from '../types';

const TRAY_ICON_STYLE_FIXTURE_KEY = 'openhistory-tray-icon-style-fixture';

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const isTauri = () => typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__);

const delay = (milliseconds = 80) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds));

export const bridge = {
  async snapshot(): Promise<DashboardSnapshot> {
    if (isTauri()) return invoke<DashboardSnapshot>('get_dashboard');
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

  async deleteHistory(scope: HistoryDeleteScope): Promise<void> {
    if (isTauri()) await invoke('delete_history', { scope });
    else await delay(80);
  },

  /**
   * Exports a Markdown report draft for the given week.
   *
   * There is no native `export_week_report` command yet — range digests are not wired to the
   * Tauri backend (see `openhistory-digest` and `openhistory-demo`). Until that lands this takes
   * the digest only to keep the call site's intent clear, and is a fixture-parity no-op so the
   * button is exercisable without crashing a real build.
   */
  async exportWeekReport(digest: WeekDigest): Promise<void> {
    void digest;
    await delay(120);
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
};
