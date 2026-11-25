import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import type { SnippetEntry } from '@/lib/types';
import { type SessionTab, useSessions } from '@/stores/sessions';
import { useSnippetRun } from '@/stores/snippetRun';
import { EMPTY_SNAPSHOT, useVault } from '@/stores/vault';

import { SnippetDialogs } from './SnippetDialogs';
import { SnippetsView } from './SnippetsView';

const sendText = vi.fn<(text: string, execute: boolean) => boolean>(() => true);
const focus = vi.fn();

vi.mock('@/features/terminal/TerminalSession', () => ({
  existingTerminalSession: () => ({ sendText, focus }),
}));
vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return {
    ...actual,
    snippetSave: vi.fn().mockResolvedValue('s-new'),
    snippetDelete: vi.fn().mockResolvedValue(undefined),
    vaultSnapshot: vi.fn(),
  };
});

const RESTART: SnippetEntry = {
  id: 's-restart',
  name: 'Redémarrer un service',
  content: 'sudo systemctl restart {{service}}',
  description: 'systemd',
  variables: ['service'],
};

function terminal(id: string, host: string, paneOf: string | null = null): SessionTab {
  return {
    id,
    kind: 'terminal',
    parentTabId: null,
    paneOf,
    customTitle: null,
    target: { host, port: 22, username: 'root', authKind: 'password', keyFile: null },
    local: null,
    status: 'connected',
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
  vi.clearAllMocks();
  useVault.setState({ snapshot: { ...EMPTY_SNAPSHOT, snippets: [RESTART] } });
  vi.mocked(ipc.vaultSnapshot).mockResolvedValue({ ...EMPTY_SNAPSHOT, snippets: [RESTART] });
  useSessions.setState({
    tabs: [terminal('t-web', 'web.lab'), terminal('t-db', 'db.lab'), terminal('t-pve', 'pve.lab')],
    activeTabId: 't-web',
    layouts: {},
    focusedPane: {},
    lastPaneId: 't-web',
  });
  useSnippetRun.setState({ pickerTargets: null, run: null });
});

describe('envoi d’un snippet (SNP-02, SNP-03)', () => {
  it('demande la valeur de {{service}} avant l’envoi et montre le texte final', async () => {
    const user = userEvent.setup();
    useSnippetRun.getState().openRun(RESTART.id, ['t-web']);
    render(<SnippetDialogs />);

    const insert = screen.getByRole('button', { name: /^Insérer$/ });
    expect(insert).toBeDisabled();
    expect(screen.getByText(/Renseignez chaque variable/)).toBeInTheDocument();

    await user.type(screen.getByRole('textbox', { name: 'service' }), 'nginx');
    expect(screen.getByText('sudo systemctl restart nginx')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^Exécuter$/ }));
    expect(sendText).toHaveBeenCalledTimes(1);
    expect(sendText).toHaveBeenCalledWith('sudo systemctl restart nginx', true);
    expect(useSnippetRun.getState().run).toBeNull();
  });

  it('insère sans valider par défaut (Entrée dans le champ)', async () => {
    const user = userEvent.setup();
    useSnippetRun.getState().openRun(RESTART.id, ['t-web']);
    render(<SnippetDialogs />);
    await user.type(screen.getByRole('textbox', { name: 'service' }), 'sshd{Enter}');
    expect(sendText).toHaveBeenCalledWith('sudo systemctl restart sshd', false);
  });

  it('envoie vers plusieurs sessions cochées, et seulement elles', async () => {
    const user = userEvent.setup();
    useSnippetRun.getState().openRun(RESTART.id, ['t-web']);
    render(<SnippetDialogs />);
    await user.click(screen.getByRole('checkbox', { name: /db\.lab/ }));
    await user.type(screen.getByRole('textbox', { name: 'service' }), 'nginx');
    await user.click(screen.getByRole('button', { name: 'Exécuter dans 2 sessions' }));
    expect(sendText).toHaveBeenCalledTimes(2);
  });

  it('refuse une valeur sur plusieurs lignes ou avec des caractères de contrôle', async () => {
    const user = userEvent.setup();
    useSnippetRun.getState().openRun(RESTART.id, ['t-web']);
    render(<SnippetDialogs />);
    const field = screen.getByRole('textbox', { name: 'service' });
    await user.click(field);
    await user.paste('nginx\u001b[201~');
    expect(field).toHaveAttribute('aria-invalid', 'true');
    expect(screen.getByRole('button', { name: /^Exécuter$/ })).toBeDisabled();
    expect(sendText).not.toHaveBeenCalled();
  });

  it('choisit un snippet dans la liste rapide puis prépare l’envoi', async () => {
    const user = userEvent.setup();
    useSnippetRun.getState().openPicker(['t-db']);
    render(<SnippetDialogs />);
    await user.click(screen.getByRole('button', { name: /Redémarrer un service/ }));
    expect(useSnippetRun.getState().run).toEqual({ snippetId: RESTART.id, targets: ['t-db'] });
  });
});

describe('bibliothèque de snippets (SNP-01)', () => {
  it('crée un snippet et affiche les variables détectées', async () => {
    const user = userEvent.setup();
    render(<SnippetsView />);
    await user.click(screen.getByRole('button', { name: 'Nouveau snippet' }));
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByRole('textbox', { name: 'Nom' }), 'Journal');
    // `{{` est une séquence spéciale de user-event : on colle le texte.
    await user.click(within(dialog).getByRole('textbox', { name: 'Commande' }));
    await user.paste('journalctl -u {{unit}} -n {{lines}}');
    expect(within(dialog).getByText(/unit, lines/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    expect(ipc.snippetSave).toHaveBeenCalledWith({
      id: null,
      name: 'Journal',
      content: 'journalctl -u {{unit}} -n {{lines}}',
      description: '',
    });
  });

  it('refuse un contenu avec des caractères de contrôle sans appeler le cœur', async () => {
    const user = userEvent.setup();
    render(<SnippetsView />);
    await user.click(screen.getByRole('button', { name: 'Nouveau snippet' }));
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByRole('textbox', { name: 'Nom' }), 'X');
    await user.click(within(dialog).getByRole('textbox', { name: 'Commande' }));
    await user.paste('echo \u001b[31m');
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    expect(ipc.snippetSave).not.toHaveBeenCalled();
    expect(within(dialog).getByRole('alert')).toHaveTextContent(/Snippet invalide/);
  });

  it('supprime un snippet après confirmation et propose l’envoi vers le dernier panneau', async () => {
    const user = userEvent.setup();
    render(<SnippetsView />);
    await user.click(screen.getByRole('button', { name: 'Envoyer Redémarrer un service' }));
    expect(useSnippetRun.getState().run).toEqual({ snippetId: RESTART.id, targets: ['t-web'] });

    await user.click(screen.getByRole('button', { name: 'Supprimer Redémarrer un service' }));
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }),
    );
    expect(ipc.snippetDelete).toHaveBeenCalledWith(RESTART.id);
  });
});
