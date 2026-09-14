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
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(screen.getByRole('alertdialog')).toHaveTextContent('Delete today?');
    expect(screen.getByRole('alertdialog')).toHaveTextContent('Raw events');
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  });

  it('keeps optional local processing and agent access disabled by default', () => {
    render(<FullHistoryView initial={structuredClone(dashboardFixture)} />);
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.getByLabelText('Use an on-device model')).not.toBeChecked();
    expect(screen.getByLabelText('Loopback API')).not.toBeChecked();
    expect(screen.getByLabelText('MCP companion')).not.toBeChecked();
  });
});
