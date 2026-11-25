import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import type { LocalProfile } from '@/lib/types';
import { HOME_TAB_ID, useSessions } from '@/stores/sessions';

import { LocalTerminalsView } from './LocalTerminalsView';
import { openLocalTerminal } from './openLocal';

vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return { ...actual, termProfiles: vi.fn() };
});
vi.mock('./openLocal', () => ({ openLocalTerminal: vi.fn() }));

const PROFILES: LocalProfile[] = [
  { id: 'shell:/usr/bin/zsh', name: 'zsh', kind: 'shell', isDefault: true },
  { id: 'wsl:Debian', name: 'Debian', kind: 'wsl', isDefault: false },
];

describe('LocalTerminalsView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSessions.setState({ tabs: [], activeTabId: HOME_TAB_ID });
  });

  it('liste les shells détectés par le cœur et ouvre celui choisi', async () => {
    vi.mocked(ipc.termProfiles).mockResolvedValue(PROFILES);
    render(<LocalTerminalsView />);
    expect(await screen.findByText('zsh')).toBeInTheDocument();
    expect(screen.getByText('Debian')).toBeInTheDocument();
    const buttons = screen.getAllByRole('button', { name: 'Ouvrir' });
    const second = buttons[1];
    expect(second).toBeDefined();
    if (second) await userEvent.click(second);
    expect(openLocalTerminal).toHaveBeenCalledWith(PROFILES[1]);
  });

  it('signale l’absence de shell et l’erreur du cœur', async () => {
    vi.mocked(ipc.termProfiles).mockRejectedValue({ code: 'vault.locked' });
    render(<LocalTerminalsView />);
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Ouvrir' })).not.toBeInTheDocument();
  });
});
