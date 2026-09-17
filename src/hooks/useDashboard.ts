import { useCallback, useEffect, useState } from 'react';
import { bridge } from '../lib/bridge';
import type { CollectionStatus, DashboardSnapshot } from '../types';

export interface DashboardState {
  snapshot?: DashboardSnapshot;
  loading: boolean;
  error?: string;
  /** When `snapshot` was last fetched (or provided as a fixture), for display only. */
  updatedAt?: Date;
  /** Encrypted storage has not been opened yet, so nothing has been read rather than nothing exists. */
  unlocking: boolean;
  toggleCollection: () => Promise<void>;
  retry: () => void;
}

/**
 * Reads one local day's projection and keeps it fresh.
 *
 * `date` is `YYYY-MM-DD`; leaving it out asks for today, and keeps asking for today, so a surface
 * left open overnight follows the date rather than freezing on the day it was opened. While a newly
 * requested day is in flight the previous one stays on screen — it is captioned with its own
 * `selectedDate`, so what is shown never disagrees with what it says it is.
 */
export function useDashboard(initial?: DashboardSnapshot, date?: string): DashboardState {
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | undefined>(initial);
  const [loading, setLoading] = useState(!initial);
  const [error, setError] = useState<string>();
  const [updatedAt, setUpdatedAt] = useState<Date | undefined>(initial ? new Date() : undefined);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    if (initial) return;
    let active = true;
    bridge
      .snapshot(date)
      .then((next) => {
        if (active) {
          setSnapshot(next);
          setUpdatedAt(new Date());
          setError(undefined);
        }
      })
      .catch((reason: unknown) => {
        if (active) setError(reason instanceof Error ? reason.message : 'Could not load history');
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [date, initial, reload]);

  // Collection keeps running while a window is hidden, so a snapshot taken when the window first
  // loaded is stale by the time it is looked at again. Refetch whenever this surface becomes
  // visible or focused — which for the tray panel is exactly when it is opened — and keep a slow
  // poll going while it stays on screen.
  useEffect(() => {
    if (initial) return;
    const refresh = () => setReload((value) => value + 1);
    const refreshWhenVisible = () => {
      if (document.visibilityState === 'visible') refresh();
    };
    window.addEventListener('focus', refresh);
    document.addEventListener('visibilitychange', refreshWhenVisible);
    const timer = window.setInterval(refreshWhenVisible, 15_000);
    return () => {
      window.removeEventListener('focus', refresh);
      document.removeEventListener('visibilitychange', refreshWhenVisible);
      window.clearInterval(timer);
    };
  }, [initial]);

  // Storage opens after the window does, and it can be waiting behind a keychain prompt for as
  // long as the user takes to answer it. Poll quickly until it is open, so the day appears as soon
  // as it can be read instead of at the next slow refresh.
  useEffect(() => {
    if (initial || snapshot?.storageReady !== false) return;
    const timer = window.setTimeout(() => setReload((value) => value + 1), 700);
    return () => window.clearTimeout(timer);
  }, [initial, snapshot]);

  const toggleCollection = useCallback(async () => {
    if (!snapshot) return;
    const next: CollectionStatus = snapshot.status === 'recording' ? 'paused' : 'recording';
    const status = await bridge.setCollectionStatus(next);
    setSnapshot((current) => (current ? { ...current, status } : current));
  }, [snapshot]);

  return {
    snapshot,
    loading,
    error,
    updatedAt,
    unlocking: snapshot?.storageReady === false,
    toggleCollection,
    retry: () => {
      setLoading(true);
      setReload((value) => value + 1);
    },
  };
}
