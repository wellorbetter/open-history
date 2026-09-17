import { fireEvent, render, screen } from '@testing-library/react';
import { dashboardFixture } from '../src/fixtures';
import { FullHistoryView } from '../src/routes/FullHistoryView';

describe('FullHistoryView', () => {
  it('filters derived task titles and summaries', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    const search = screen.getByRole('searchbox', { name: 'Filter this day' });
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
    expect(screen.getByRole('alertdialog')).toHaveTextContent('raw events');
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

  /**
   * These three switches had no change handler, no command to persist them, and no feature behind
   * them: no HTTP listener is built and the MCP binary prints a line and exits. They flipped when
   * clicked and meant nothing. A test that asserted them "unchecked by default" passed happily,
   * which is why this one asserts they are gone instead.
   */
  it('offers no switch for a feature that is not built', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.queryByLabelText('Use an on-device model')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Loopback API')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('MCP companion')).not.toBeInTheDocument();
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  });

  /**
   * The card names the applications that are never recorded, and it used to do so from its own
   * hardcoded copy of the list — which had fallen one entry behind the policy the collector actually
   * installs, understating what is protected.
   */
  it('names every application the collector actually excludes', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.getByText(/Never recorded:/)).toHaveTextContent(
      'Never recorded: 1Password, Keychain Access, Passwords',
    );
  });

  /** Deletion touches two tables. It must not promise to remove stores that do not exist. */
  it('does not promise to delete stores the app never writes', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Delete today' }));
    const dialog = screen.getByRole('alertdialog');
    expect(dialog).not.toHaveTextContent(/search entries/i);
    expect(dialog).not.toHaveTextContent(/exports/i);
    expect(dialog).not.toHaveTextContent(/summaries/i);
  });

  /** Nothing exports a task, so the inspector may not offer to. */
  it('offers no per-task export', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    expect(screen.queryByRole('button', { name: 'Export' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'More task actions' })).not.toBeInTheDocument();
  });
});
