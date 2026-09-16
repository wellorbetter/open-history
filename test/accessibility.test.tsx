import { fireEvent, render, screen } from '@testing-library/react';
import { axe } from 'jest-axe';
import { dashboardFixture, weekDigestFixtures } from '../src/fixtures';
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

  it('week surface has no automated axe violations, including its empty and out-of-retention states', async () => {
    const { container } = render(
      <FullHistoryView
        initial={structuredClone(dashboardFixture)}
        weeks={structuredClone(weekDigestFixtures)}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Week' }));
    const withActivity = await axe(container);
    expect(withActivity.violations).toEqual([]);
    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    const outsideRetention = await axe(container);
    expect(outsideRetention.violations).toEqual([]);
  });
});
