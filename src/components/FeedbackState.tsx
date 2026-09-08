import { AlertTriangle, LoaderCircle, RefreshCw } from 'lucide-react';

export function LoadingState() {
  return (
    <div className="feedback-state" role="status">
      <LoaderCircle className="spin" size={22} aria-hidden="true" />
      <span>Loading local history…</span>
    </div>
  );
}

export function ErrorState({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="feedback-state" role="alert">
      <AlertTriangle size={22} aria-hidden="true" />
      <strong>History is unavailable</strong>
      <span>{message}</span>
      <button className="secondary-button" type="button" onClick={onRetry}>
        <RefreshCw size={15} aria-hidden="true" /> Retry
      </button>
    </div>
  );
}
