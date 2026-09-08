import { dashboardFixture } from '../fixtures';
import type { DashboardSnapshot } from '../types';
import { CompactView } from './CompactView';

function variant(values: Partial<DashboardSnapshot>): DashboardSnapshot {
  return { ...structuredClone(dashboardFixture), ...values };
}

const fixtures = [
  { name: 'Recording', snapshot: variant({ status: 'recording' }) },
  { name: 'Paused', snapshot: variant({ status: 'paused' }) },
  {
    name: 'Permission needed',
    snapshot: variant({ status: 'permission_needed', current: undefined }),
  },
  {
    name: 'Long localized content',
    snapshot: variant({
      current: {
        ...structuredClone(dashboardFixture.current!),
        title: 'Review a deliberately long project title without collapsing source provenance',
      },
    }),
  },
  { name: 'Empty day', snapshot: variant({ current: undefined, timeline: [] }) },
];

export function PreviewGallery() {
  return (
    <main className="preview-gallery">
      <header>
        <p className="eyebrow">Fixture gallery</p>
        <h1>Compact surface states</h1>
        <p>Browser-only fixtures. No platform permission or activity collection is started.</p>
      </header>
      <div className="preview-grid">
        {fixtures.map(({ name, snapshot }) => (
          <section className="preview-frame" key={name}>
            <h2>{name}</h2>
            <CompactView initial={snapshot} />
          </section>
        ))}
      </div>
    </main>
  );
}
