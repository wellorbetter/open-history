import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { dashboardFixture } from '../src/fixtures';
import { bridge } from '../src/lib/bridge';
import { CompactView } from '../src/routes/CompactView';

describe('CompactView', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('shows current activity and a scannable timeline', () => {
    render(<CompactView initial={structuredClone(dashboardFixture)} />);
    expect(screen.getByText('OpenHistory')).toBeVisible();
    expect(screen.getByText('Recording')).toBeVisible();
    expect(screen.getAllByText('Harness Evidence Gate and release checks').length).toBeGreaterThan(
      0,
    );
    expect(screen.getByRole('button', { name: /private activity, 10 minutes/i })).toBeVisible();
  });

  it('pauses and resumes collection from one accessible control', async () => {
    render(<CompactView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Pause collection' }));
    await waitFor(() => expect(screen.getByText('Paused')).toBeVisible());
    fireEvent.click(screen.getByRole('button', { name: 'Resume collection' }));
    await waitFor(() => expect(screen.getByText('Recording')).toBeVisible());
  });

  it('recovers from a permission-needed status by retrying the same control', async () => {
    const snapshot = structuredClone(dashboardFixture);
    snapshot.status = 'permission_needed';
    render(<CompactView initial={snapshot} />);
    expect(screen.getByText('Permission needed')).toBeVisible();
    const retry = screen.getByRole('button', { name: 'Grant Accessibility permission' });
    expect(retry).toBeEnabled();
    fireEvent.click(retry);
    await waitFor(() => expect(screen.getByText('Recording')).toBeVisible());
  });

  /**
   * The backend really does refuse this — when Accessibility permission has been revoked, and while
   * encrypted storage is still unlocking. The rejection used to be unhandled and the promise
   * discarded, so the button simply did nothing and the panel went on saying Recording.
   */
  it('says why collection did not change when the backend refuses', async () => {
    vi.spyOn(bridge, 'setCollectionStatus').mockRejectedValue(
      new Error('encrypted storage is still unlocking'),
    );
    render(<CompactView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Pause collection' }));

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('encrypted storage is still unlocking'),
    );
    // And it must not have claimed the change happened.
    expect(screen.getByText('Recording')).toBeVisible();
  });

  /**
   * These two buttons had no handler at all, and this surface held no selected day, although the
   * backend has always accepted one. Asserting the day actually requested is what makes this a test
   * of navigation rather than of two rendered chevrons.
   */
  it('asks the backend for the previous day when paged back', async () => {
    const snapshot = structuredClone(dashboardFixture);
    const read = vi.spyOn(bridge, 'snapshot').mockResolvedValue(snapshot);
    render(<CompactView />);
    await waitFor(() => expect(screen.getByLabelText('Timeline date')).toBeVisible());
    expect(read).toHaveBeenLastCalledWith(undefined);

    fireEvent.click(screen.getByRole('button', { name: 'Previous day' }));
    await waitFor(() => expect(read).toHaveBeenLastCalledWith('2026-09-07'));
  });

  /** Today is the last day there is, so the forward control has to refuse it. */
  it('cannot be paged past today', () => {
    render(<CompactView initial={structuredClone(dashboardFixture)} />);
    expect(screen.getByRole('button', { name: 'Next day' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Previous day' })).toBeEnabled();
  });
});
