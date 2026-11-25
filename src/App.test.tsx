import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { App } from './App';
import { i18n } from './i18n';
import * as ipc from './lib/ipc';
import type * as IpcModule from './lib/ipc';
import { useSettings } from './stores/settings';
import { useUi } from './stores/ui';
import { EMPTY_SNAPSHOT, useVault } from './stores/vault';

vi.mock('./lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return {
    ...actual,
    appInfo: vi.fn(),
    vaultStatus: vi.fn(),
    vaultSnapshot: vi.fn(),
    vaultWatch: vi.fn().mockResolvedValue(undefined),
    vaultTouch: vi.fn().mockResolvedValue(undefined),
    vaultLock: vi.fn().mockResolvedValue(undefined),
    settingsSet: vi.fn().mockResolvedValue(undefined),
    knownHostList: vi.fn().mockResolvedValue([]),
  };
});

/** Affiche l'application avec un coffre déverrouillé (la porte du coffre est testée à part). */
async function setup() {
  vi.mocked(ipc.appInfo).mockResolvedValue({ name: 'Younji', version: '0.1.0', commit: 'a14aa94' });
  vi.mocked(ipc.vaultStatus).mockResolvedValue('unlocked');
  vi.mocked(ipc.vaultSnapshot).mockResolvedValue(EMPTY_SNAPSHOT);
  const result = { user: userEvent.setup(), ...render(<App />) };
  await screen.findByRole('tablist');
  return result;
}

describe('App', () => {
  beforeEach(async () => {
    useSettings.setState({ theme: 'system', language: 'fr' });
    useUi.setState({ activeSection: 'hosts' });
    useVault.setState({ status: null, statusError: null, snapshot: EMPTY_SNAPSHOT });
    await i18n.changeLanguage('fr');
  });

  it('affiche la barre latérale avec toutes les sections (APP-01)', async () => {
    await setup();
    const nav = screen.getByRole('navigation', { name: 'Navigation principale' });
    for (const label of ['Hôtes', 'Clés', 'Snippets', 'SFTP', 'Paramètres']) {
      expect(within(nav).getByRole('button', { name: label })).toBeInTheDocument();
    }
    expect(screen.getByRole('tab', { name: 'Accueil' })).toHaveAttribute('aria-selected', 'true');
  });

  it('change de section au clic', async () => {
    const { user } = await setup();
    await user.click(screen.getByRole('button', { name: 'Snippets' }));
    expect(screen.getByRole('heading', { name: 'Snippets' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Snippets' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  it('bascule la langue à chaud (APP-02)', async () => {
    const { user } = await setup();
    await user.click(screen.getByRole('button', { name: 'Paramètres' }));
    await user.selectOptions(screen.getByLabelText('Langue'), 'en');
    expect(await screen.findByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe('en');

    await user.selectOptions(screen.getByLabelText('Language'), 'fr');
    expect(await screen.findByRole('heading', { name: 'Paramètres' })).toBeInTheDocument();
  });

  it('force le thème puis revient au système (APP-03)', async () => {
    const { user } = await setup();
    await user.click(screen.getByRole('button', { name: 'Paramètres' }));
    const select = screen.getByLabelText('Thème');

    await user.selectOptions(select, 'dark');
    expect(document.documentElement).toHaveClass('dark');

    await user.selectOptions(select, 'light');
    expect(document.documentElement).toHaveClass('light');
    expect(document.documentElement).not.toHaveClass('dark');

    await user.selectOptions(select, 'system');
    // jsdom n'implémente pas matchMedia : le système est considéré comme clair.
    expect(document.documentElement).toHaveClass('light');
  });

  it('affiche la version renvoyée par le backend', async () => {
    const { user } = await setup();
    await user.click(screen.getByRole('button', { name: 'Paramètres' }));
    expect(await screen.findByTestId('app-version')).toHaveTextContent('0.1.0 (a14aa94)');
  });

  it('reste utilisable si le backend est indisponible', async () => {
    vi.mocked(ipc.appInfo).mockRejectedValue({ code: 'unknown' });
    vi.mocked(ipc.vaultStatus).mockResolvedValue('unlocked');
    vi.mocked(ipc.vaultSnapshot).mockResolvedValue(EMPTY_SNAPSHOT);
    render(<App />);
    await screen.findByRole('tablist');
    await act(async () => {
      useUi.setState({ activeSection: 'settings' });
      await Promise.resolve();
    });
    expect(screen.getByTestId('app-version')).toHaveTextContent('indisponible');
  });

  // La fermeture des onglets et la destruction des terminaux sont vérifiées dans
  // TerminalSession.test.ts (closeAllTabs).
  it('verrouille au raccourci Ctrl+Maj+L et affiche l’écran de déverrouillage', async () => {
    const { user } = await setup();
    await user.keyboard('{Control>}{Shift>}L{/Shift}{/Control}');
    expect(ipc.vaultLock).toHaveBeenCalled();
    expect(await screen.findByRole('heading', { name: 'Coffre verrouillé' })).toBeInTheDocument();
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });
});
