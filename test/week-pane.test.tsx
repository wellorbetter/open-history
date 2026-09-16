import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { dashboardFixture, weekDigestFixtures } from '../src/fixtures';
import { FullHistoryView } from '../src/routes/FullHistoryView';
import { WeekPane } from '../src/components/WeekPane';

function openWeek(weeks = weekDigestFixtures) {
  render(
    <FullHistoryView initial={structuredClone(dashboardFixture)} weeks={structuredClone(weeks)} />,
  );
  fireEvent.click(screen.getByRole('button', { name: 'Week' }));
}

describe('WeekPane', () => {
  it('lists work items in deterministic order and separates attention from artifact counts', () => {
    openWeek();
    const results = screen.getByLabelText('Week results');
    const cards = within(results).getAllByRole('button', { name: /commit/ });
    expect(cards.map((card) => card.textContent)).toEqual([
      expect.stringContaining('open-history'),
      expect.stringContaining('timetrace'),
      expect.stringContaining('Reading: docs.rs'),
    ]);

    // A project with agent-session artifacts but no observed attention states that explicitly,
    // rather than showing an unexplained zero.
    expect(within(results).getByText('No attention data was observed')).toBeVisible();
    expect(within(results).getByText('95 min observed')).toBeVisible();
  });

  it('distinguishes an empty week from a week outside the retention window', () => {
    openWeek();
    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    expect(screen.getByText('No activity recorded')).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    expect(screen.getByText('Outside the retention window')).toBeVisible();
    expect(screen.queryByText('No activity recorded')).not.toBeInTheDocument();
  });

  it('disables navigation at the ends of the available range', () => {
    openWeek();
    expect(screen.getByRole('button', { name: 'Next week' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    expect(screen.getByRole('button', { name: 'Previous week' })).toBeDisabled();
  });

  it('opens the day timeline positioned on evidence selected from a work item', () => {
    openWeek();
    fireEvent.click(screen.getByRole('button', { name: /open-history/ }));
    fireEvent.click(screen.getByRole('button', { name: /observed activity \(95 min\)/ }));

    // Selecting evidence with a segment link switches to the day timeline and opens that segment.
    expect(screen.getByRole('button', { name: 'History' })).toHaveAttribute('aria-current', 'page');
    expect(
      screen.getByRole('heading', { name: 'Harness Evidence Gate and release checks' }),
    ).toBeVisible();
  });

  it('disables export for an empty or out-of-retention week and enables it otherwise', () => {
    openWeek();
    expect(screen.getByRole('button', { name: /export report draft/i })).not.toBeDisabled();

    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    expect(screen.getByRole('button', { name: /export report draft/i })).toBeDisabled();

    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    expect(screen.getByRole('button', { name: /export report draft/i })).toBeDisabled();
  });

  it('confirms export completed and states when no generated text is included', async () => {
    openWeek();
    expect(screen.getByText('No generated text included.')).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: /export report draft/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /exported/i })).toBeVisible());
  });

  it('renders nothing to select when a week has no work items', () => {
    render(
      <WeekPane
        weeks={[
          {
            rangeStart: '2026-09-07',
            rangeEnd: '2026-09-13',
            hasGeneratedText: false,
            workItems: [],
          },
        ]}
        onOpenSegment={() => {}}
      />,
    );
    expect(screen.getByText('Select a work item')).toBeVisible();
  });
});
