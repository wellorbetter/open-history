import { render, screen } from '@testing-library/react';
import { SetupView } from '../src/routes/SetupView';

describe('SetupView', () => {
  it('explains capture boundaries before permission setup', () => {
    render(<SetupView />);
    expect(
      screen.getByRole('heading', { name: 'Remember the work, not the surveillance.' }),
    ).toBeVisible();
    expect(screen.getByText('No screenshots')).toBeVisible();
    expect(screen.getByText('No audio')).toBeVisible();
    expect(screen.getByText('No raw keys')).toBeVisible();
    expect(screen.getByRole('button', { name: /continue to permission setup/i })).toBeVisible();
  });
});
