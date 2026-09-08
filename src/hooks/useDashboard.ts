import { useCallback, useEffect, useState } from 'react';
import { bridge } from '../lib/bridge';
import type { CollectionStatus, DashboardSnapshot } from '../types';

export interface DashboardState {
  snapshot?: DashboardSnapshot;
  loading: boolean;
  error?: string;
  toggleCollection: () => Promise<void>;
  retry: () => void;
}

export function useDashboard(initial?: DashboardSnapshot): DashboardState {
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | undefined>(initial);
  const [loading, setLoading] = useState(!initial);
  const [error, setError] = useState<string>();
  const [reload, setReload] = useState(0);

  useEffect(() => {
    if (initial) return;
    let active = true;
    bridge
      .snapshot()
      .then((next) => {
        if (active) {
          setSnapshot(next);
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
  }, [initial, reload]);

  const toggleCollection = useCallback(async () => {
    if (!snapshot || snapshot.status === 'permission_needed') return;
    const next: CollectionStatus = snapshot.status === 'recording' ? 'paused' : 'recording';
    const status = await bridge.setCollectionStatus(next);
    setSnapshot((current) => (current ? { ...current, status } : current));
  }, [snapshot]);

  return {
    snapshot,
    loading,
    error,
    toggleCollection,
    retry: () => {
      setLoading(true);
      setReload((value) => value + 1);
    },
  };
}
