import { fireEvent, render, screen } from '@testing-library/react';
import { dashboardFixture } from '../src/fixtures';
import { FullHistoryView } from '../src/routes/FullHistoryView';

describe('FullHistoryView', () => {
  it('filters derived task titles and summaries', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    const search = screen.getByRole('searchbox', { name: 'Search history' });
    fireEvent.change(search, { target: { value: 'architecture' } });
    expect(
      screen.getByRole('button', { name: /active intelligence architecture review/i }),
    ).toBeVisible();
    expect(screen.queryByRole('button', { name: /adapter verification/i })).not.toBeInTheDocument();
  });

  it('opens a scoped destructive confirmation and cancels safely', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Delete today' }));
    expect(screen.getByRole('alertdialog')).toHaveTextContent('Delete today?');
    expect(screen.getByRole('alertdialog')).toHaveTextContent('Raw events');
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  });

  // The day controls have to be real controls, and they have to refuse the one move that would ask
  // for a day that has not happened yet.
  it('offers day navigation that cannot run past the day being shown', () => {
    const snapshot = structuredClone(dashboardFixture);
    render(<FullHistoryView initial={snapshot} />);
    expect(screen.getByRole('button', { name: 'Previous day' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Next day' })).toBeDisabled();
    expect(screen.getByLabelText('Choose date')).toHaveValue(snapshot.selectedDate);
  });

  it('lets the day being shown be left behind once it is not today', () => {
    const snapshot = { ...structuredClone(dashboardFixture), isToday: false };
    render(<FullHistoryView initial={snapshot} />);
    expect(screen.getByRole('button', { name: 'Next day' })).toBeEnabled();
    // Deletion is expressed as "everything since a moment", so a past day cannot be removed on its
    // own. Leaving the button live would delete today from under a day that is not today.
    expect(screen.getByRole('button', { name: 'Delete today' })).toBeDisabled();
  });

  it('keeps optional local processing and agent access disabled by default', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.getByLabelText('Use an on-device model')).not.toBeChecked();
    expect(screen.getByLabelText('Loopback API')).not.toBeChecked();
    expect(screen.getByLabelText('MCP companion')).not.toBeChecked();
  });
});
