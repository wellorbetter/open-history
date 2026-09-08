import { render } from '@testing-library/react';
import { axe } from 'jest-axe';
import { dashboardFixture } from '../src/fixtures';
import { CompactView } from '../src/routes/CompactView';
import { FullHistoryView } from '../src/routes/FullHistoryView';
import { SetupView } from '../src/routes/SetupView';

describe('accessibility contracts', () => {
  it.each([
    ['compact', <CompactView initial={structuredClone(dashboardFixture)} />],
    ['full history', <FullHistoryView initial={structuredClone(dashboardFixture)} />],
    ['setup', <SetupView />],
  ])('%s surface has no automated axe violations', async (_name, view) => {
    const { container } = render(view);
    const result = await axe(container);
    expect(result.violations).toEqual([]);
  });
});
