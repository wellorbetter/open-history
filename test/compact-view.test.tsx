import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { dashboardFixture } from '../src/fixtures';
import { CompactView } from '../src/routes/CompactView';

describe('CompactView', () => {
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

  it('keeps permission errors actionable without presenting a resume action', () => {
    const snapshot = structuredClone(dashboardFixture);
    snapshot.status = 'permission_needed';
    render(<CompactView initial={snapshot} />);
    expect(screen.getByText('Permission needed')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Resume collection' })).toBeDisabled();
  });
});
