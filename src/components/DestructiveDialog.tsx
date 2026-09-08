import { useEffect, useRef } from 'react';
import type { HistoryDeleteScope } from '../types';

const scopeLabels: Record<HistoryDeleteScope, string> = {
  last_10_minutes: 'the last 10 minutes',
  last_hour: 'the last hour',
  today: 'today',
  all: 'all history',
};

export function DestructiveDialog({
  scope,
  onCancel,
  onConfirm,
}: {
  scope: HistoryDeleteScope;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onCancel}>
      <section
        className="dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="delete-title"
        aria-describedby="delete-description"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <p className="eyebrow eyebrow--danger">Delete history</p>
        <h2 id="delete-title">Delete {scopeLabels[scope]}?</h2>
        <p id="delete-description">
          Raw events, task segments, summaries, search entries, and managed temporary exports in
          this range will be removed. This cannot be undone.
        </p>
        <div className="dialog-actions">
          <button ref={cancelRef} className="secondary-button" type="button" onClick={onCancel}>
            Cancel
          </button>
          <button className="danger-button" type="button" onClick={onConfirm}>
            Delete
          </button>
        </div>
      </section>
    </div>
  );
}
