import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { useBroadcast } from '@/stores/broadcast';
import { type SessionTab, useSessions } from '@/stores/sessions';

import { BroadcastBanner } from './BroadcastBanner';
import { BroadcastDialog } from './BroadcastDialog';

function terminal(
  id: string,
  host: string,
  status: SessionTab['status'] = 'connected',
): SessionTab {
  return {
    id,
    kind: 'terminal',
    parentTabId: null,
    paneOf: null,
    customTitle: null,
    target: { host, port: 22, username: 'root', authKind: 'password', keyFile: null },
    local: null,
    status,
    errorCode: null,
    pendingHostKey: null,
    hostKeyChange: null,
    pendingPrompt: null,
    searchOpen: false,
    sessionId: 1,
    remoteCwd: null,
    sftpPanelOpen: false,
  };
}

beforeEach(() => {
  useBroadcast.setState({ targets: [], dialogOpen: true });
  useSessions.setState({
    tabs: [
      terminal('a', 'web.lab'),
      terminal('b', 'db.lab'),
      terminal('c', 'pve.lab'),
      terminal('d', 'off.lab', 'disconnected'),
    ],
    activeTabId: 'a',
    layouts: {},
    focusedPane: {},
    lastPaneId: 'a',
  });
});

describe('activation de la diffusion (SSH-08)', () => {
  it('ne propose que les sessions connectées et exige deux cibles confirmées', async () => {
    const user = userEvent.setup();
    render(<BroadcastDialog />);
    expect(screen.queryByText(/off\.lab/)).not.toBeInTheDocument();
    // Seule la session active est cochée d'office : impossible d'activer en un clic.
    expect(screen.getByRole('checkbox', { name: /web\.lab/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /db\.lab/ })).not.toBeChecked();
    const confirm = screen.getByRole('button', { name: /Diffuser vers 1 session/ });
    expect(confirm).toBeDisabled();

    await user.click(screen.getByRole('checkbox', { name: /db\.lab/ }));
    await user.click(screen.getByRole('checkbox', { name: /pve\.lab/ }));
    expect(screen.getByRole('alert')).toHaveTextContent('3 sessions recevront chaque frappe');
    await user.click(screen.getByRole('button', { name: 'Diffuser vers 3 sessions' }));
    expect(useBroadcast.getState().targets).toEqual(['a', 'b', 'c']);
    expect(useBroadcast.getState().dialogOpen).toBe(false);
  });

  it('annule sans rien activer', async () => {
    const user = userEvent.setup();
    render(<BroadcastDialog />);
    await user.click(screen.getByRole('button', { name: 'Annuler' }));
    expect(useBroadcast.getState()).toMatchObject({ targets: [], dialogOpen: false });
  });

  it('affiche un bandeau impossible à manquer, qui arrête la diffusion en un clic', async () => {
    const user = userEvent.setup();
    useBroadcast.setState({ targets: ['a', 'b', 'c'], dialogOpen: false });
    render(<BroadcastBanner />);
    expect(screen.getByRole('status')).toHaveTextContent('active vers 3 sessions');
    await user.click(screen.getByRole('button', { name: 'Arrêter la diffusion' }));
    expect(useBroadcast.getState().targets).toEqual([]);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });
});
