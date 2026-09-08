import { Bot, Braces, FileText, Globe2, Monitor, TerminalSquare } from 'lucide-react';
import type { ActivitySource, SourceKind } from '../types';

const sourceIcons: Record<SourceKind, typeof TerminalSquare> = {
  terminal: TerminalSquare,
  editor: Braces,
  browser: Globe2,
  agent: Bot,
  document: FileText,
  system: Monitor,
};

export function SourceStack({ sources, max = 3 }: { sources: ActivitySource[]; max?: number }) {
  const visible = sources.slice(0, max);
  const hidden = Math.max(0, sources.length - max);
  const label = sources.length
    ? `Sources: ${sources.map((source) => source.name).join(', ')}`
    : 'No source details retained';

  return (
    <span className="source-stack" role="img" aria-label={label} title={label}>
      {visible.map((source) => {
        const Icon = sourceIcons[source.kind];
        return (
          <span
            className="source-icon"
            key={source.id}
            style={{ '--source-color': source.color } as React.CSSProperties}
            aria-hidden="true"
          >
            <Icon size={14} strokeWidth={2.1} />
          </span>
        );
      })}
      {hidden > 0 && (
        <span className="source-overflow" aria-hidden="true">
          +{hidden}
        </span>
      )}
    </span>
  );
}
