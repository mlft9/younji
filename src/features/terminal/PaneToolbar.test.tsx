import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { SessionTab, TabStatus } from '@/stores/sessions';

import { PaneToolbar } from './PaneToolbar';
import { closePane } from './TerminalSession';

vi.mock('./TerminalSession', () => ({ closePane: vi.fn() }));
vi.mock('./split', () => ({ splitAndConnect: vi.fn() }));

function pane(status: TabStatus): SessionTab {
  return {
    id: 'p2',
    kind: 'terminal',
    parentTabId: null,
    paneOf: 'p1',
    customTitle: null,
    target: null,
    local: null,
    status,
    errorCode: null,
    pendingHostKey: null,
    hostKeyChange: null,
    pendingPrompt: null,
    searchOpen: false,
    sessionId: null,
    remoteCwd: null,
    sftpPanelOpen: false,
  };
}

describe('barre d’outils d’un panneau (SSH-07)', () => {
  it.each<TabStatus>(['form', 'connecting', 'hostKey', 'authPrompt', 'blocked', 'disconnected'])(
    'permet de fermer un panneau non connecté (%s)',
    async (status) => {
      const user = userEvent.setup();
      render(<PaneToolbar tab={pane(status)} split />);
      // Seule la fermeture a un sens sans session : pas de découpe, de snippet ni de diffusion.
      expect(screen.queryByRole('button', { name: 'Diviser à droite' })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Envoyer un snippet' })).not.toBeInTheDocument();
      await user.click(screen.getByRole('button', { name: 'Fermer ce panneau' }));
      expect(closePane).toHaveBeenCalledWith('p2');
    },
  );

  it('propose tous les outils sur un panneau connecté', () => {
    render(<PaneToolbar tab={pane('connected')} split />);
    expect(screen.getByRole('button', { name: 'Diviser à droite' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Diffusion de la saisie' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Fermer ce panneau' })).toBeInTheDocument();
  });

  it('limite un terminal local à la découpe : ni snippet, ni diffusion, ni SFTP', () => {
    const local = {
      ...pane('connected'),
      local: { id: 'cmd', name: 'CMD', kind: 'cmd' as const, isDefault: true },
    };
    render(<PaneToolbar tab={local} split={false} />);
    expect(screen.getByRole('button', { name: 'Diviser à droite' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Envoyer un snippet' })).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Diffusion de la saisie' }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /SFTP/ })).not.toBeInTheDocument();
  });

  it('n’affiche rien pour un onglet non découpé et non connecté', () => {
    const { container } = render(<PaneToolbar tab={pane('form')} split={false} />);
    expect(container).toBeEmptyDOMElement();
  });
});
