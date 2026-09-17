import { fireEvent, render, screen, within } from '@testing-library/react';
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

  it('reports a week with no work items as empty', () => {
    openWeek();
    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    expect(screen.getByText('No activity recorded')).toBeVisible();
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

  it('renders nothing to select when a week has no work items', () => {
    render(
      <WeekPane
        weeks={[{ rangeStart: '2026-09-07', rangeEnd: '2026-09-13', workItems: [] }]}
        onOpenSegment={() => {}}
      />,
    );
    expect(screen.getByText('Select a work item')).toBeVisible();
  });

  /**
   * The native window renders `<FullHistoryView />` with no `weeks`, and this default used to be the
   * design fixtures — so every user was shown invented commits, agent sessions and attention minutes
   * captioned as their own week. Nothing aggregates a week yet, and until something does this pane
   * has to say so.
   */
  it('shows no week digest at all when nothing produced one', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Week' }));

    expect(screen.getByText('Weeks are not aggregated yet')).toBeVisible();
    // None of the fixture's invented numbers, and no week to page through.
    expect(screen.queryByText('95 min observed')).not.toBeInTheDocument();
    expect(screen.queryByText(/2 commits/)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Previous week' })).not.toBeInTheDocument();
  });

  /** There is no export command in the backend, so nothing may offer to export. */
  it('offers no export, because nothing can be exported', () => {
    openWeek();
    expect(screen.queryByRole('button', { name: /export/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/generated text/i)).not.toBeInTheDocument();
  });

  /** No retention pass exists, so no week can be described as having aged out of one. */
  it('never claims a week was deleted by a retention policy', () => {
    openWeek();
    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    fireEvent.click(screen.getByRole('button', { name: 'Previous week' }));
    expect(screen.queryByText(/retention/i)).not.toBeInTheDocument();
  });
});
