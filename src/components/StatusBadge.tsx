import { AlertCircle, CirclePause, CirclePlay, ShieldAlert } from 'lucide-react';
import type { CollectionStatus } from '../types';

const labels: Record<CollectionStatus, string> = {
  recording: 'Recording',
  paused: 'Paused',
  permission_needed: 'Permission needed',
  error: 'Adapter error',
};

export function StatusBadge({ status }: { status: CollectionStatus }) {
  const Icon =
    status === 'recording'
      ? CirclePlay
      : status === 'paused'
        ? CirclePause
        : status === 'permission_needed'
          ? ShieldAlert
          : AlertCircle;

  return (
    <span className="status-badge" data-status={status} aria-live="polite">
      <Icon size={14} strokeWidth={2.3} aria-hidden="true" />
      <span>{labels[status]}</span>
    </span>
  );
}
