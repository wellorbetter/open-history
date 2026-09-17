import { render, screen } from '@testing-library/react';
import { SetupView } from '../src/routes/SetupView';

describe('SetupView', () => {
  it('explains capture boundaries', () => {
    render(<SetupView />);
    expect(
      screen.getByRole('heading', { name: 'Remember the work, not the surveillance.' }),
    ).toBeVisible();
    expect(screen.getByText('No screenshots')).toBeVisible();
    expect(screen.getByText('No audio')).toBeVisible();
    expect(screen.getByText('No raw keys')).toBeVisible();
  });

  /**
   * There is no permission-setup surface to continue to — the app knows four surfaces and none of
   * them is one, and permission is only ever requested as a side effect of starting collection. The
   * button had no handler, so it promised a next step that did not exist.
   */
  it('offers no button that leads nowhere', () => {
    render(<SetupView />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  /**
   * Asking an agent to read a row back sends that row's window titles to the agent's vendor. The
   * history surface discloses this; an unqualified "no cloud processing" promise here contradicted
   * it.
   */
  it('does not promise that nothing ever leaves the machine', () => {
    render(<SetupView />);
    expect(
      screen.queryByText(/no account, cloud processing, or telemetry/i),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/unless you ask an agent to read a row back/i)).toBeVisible();
  });

  /**
   * Password-manager exclusion is real and enforced. Private-browser exclusion is not: no browser
   * adapter exists and every caller reports `private_context: false`, so the branch can never fire.
   */
  it('claims only the exclusion that is enforced', () => {
    render(<SetupView />);
    expect(screen.getByText(/password managers are excluded/i)).toBeVisible();
    expect(screen.queryByText(/private browser/i)).not.toBeInTheDocument();
  });
});
