import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import type { HostEntry, VaultSnapshot } from '@/lib/types';
import { EMPTY_SNAPSHOT, useVault } from '@/stores/vault';

import { effectiveUsername, filterHosts, parseTags } from './filter';
import { HostsView } from './HostsView';
import { openHost } from './openHost';

vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return {
    ...actual,
    hostSave: vi.fn().mockResolvedValue('nouvel-id'),
    rdpLaunch: vi.fn().mockResolvedValue({ existingCredential: false }),
    hostDelete: vi.fn().mockResolvedValue(undefined),
    groupSave: vi.fn().mockResolvedValue('g-new'),
    groupDelete: vi.fn().mockResolvedValue(undefined),
    vaultSnapshot: vi.fn(),
  };
});
vi.mock('./openHost', () => ({ openHost: vi.fn() }));

function host(partial: Partial<HostEntry>): HostEntry {
  return {
    id: 'h',
    name: 'h',
    address: 'h.lab',
    port: 22,
    username: 'root',
    identityId: null,
    groupId: null,
    tags: [],
    notes: '',
    protocol: { kind: 'ssh' },
    ...partial,
  };
}

const SNAPSHOT: VaultSnapshot = {
  ...EMPTY_SNAPSHOT,
  groups: [{ id: 'g-lab', name: 'Lab' }],
  identities: [{ id: 'i-1', name: 'Admin', username: 'admin', hasPassword: true, keyId: null }],
  hosts: [
    host({ id: 'h-web', name: 'Web', address: 'web.lab', groupId: 'g-lab', tags: ['prod'] }),
    host({ id: 'h-db', name: 'Base de données', address: '10.0.0.5', port: 2222 }),
    host({ id: 'h-pve', name: 'Proxmox', address: 'pve.lab', username: '', identityId: 'i-1' }),
  ],
};

describe('recherche et groupes (HST-02)', () => {
  it('filtre par groupe et trie par nom', () => {
    expect(filterHosts(SNAPSHOT.hosts, { kind: 'all' }, '').map((h) => h.name)).toEqual([
      'Base de données',
      'Proxmox',
      'Web',
    ]);
    expect(filterHosts(SNAPSHOT.hosts, { kind: 'ungrouped' }, '')).toHaveLength(2);
    expect(filterHosts(SNAPSHOT.hosts, { kind: 'group', id: 'g-lab' }, '')).toHaveLength(1);
  });

  it('cherche sans tenir compte des accents ni de la casse, sur plusieurs champs', () => {
    const names = (query: string) =>
      filterHosts(SNAPSHOT.hosts, { kind: 'all' }, query).map((h) => h.name);
    expect(names('donnees')).toEqual(['Base de données']);
    expect(names('PROD')).toEqual(['Web']);
    expect(names('10.0.0')).toEqual(['Base de données']);
    expect(names('2222')).toEqual(['Base de données']);
    expect(names('lab web')).toEqual(['Web']);
    expect(names('introuvable')).toEqual([]);
  });

  it('découpe les étiquettes et choisit l’utilisateur', () => {
    expect(parseTags(' prod, , web ,lab')).toEqual(['prod', 'web', 'lab']);
    expect(effectiveUsername('', 'admin')).toBe('admin');
    expect(effectiveUsername('root', 'admin')).toBe('root');
    expect(effectiveUsername('', undefined)).toBe('');
  });
});

describe('carnet d’hôtes', () => {
  beforeEach(() => {
    useVault.setState({ status: 'unlocked', snapshot: SNAPSHOT });
    vi.mocked(ipc.vaultSnapshot).mockResolvedValue(SNAPSHOT);
    vi.mocked(openHost).mockClear();
    vi.mocked(ipc.hostSave).mockClear();
  });

  it('affiche les hôtes avec l’utilisateur de leur identité', () => {
    render(<HostsView />);
    const list = screen.getByRole('list', { name: 'Hôtes' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(3);
    expect(within(list).getByText('admin@pve.lab')).toBeInTheDocument();
    expect(within(list).getByText('root@10.0.0.5:2222')).toBeInTheDocument();
  });

  it('se connecte au double-clic et à la touche Entrée', async () => {
    const user = userEvent.setup();
    render(<HostsView />);
    await user.dblClick(screen.getByText('Proxmox'));
    expect(openHost).toHaveBeenCalledWith(SNAPSHOT.hosts[2], SNAPSHOT.identities);
    screen.getByText('Web').closest('button')?.focus();
    await user.keyboard('{Enter}');
    expect(openHost).toHaveBeenLastCalledWith(SNAPSHOT.hosts[0], SNAPSHOT.identities);
  });

  it('filtre par la recherche et par groupe', async () => {
    const user = userEvent.setup();
    render(<HostsView />);
    await user.type(screen.getByRole('searchbox', { name: 'Rechercher un hôte' }), 'prox');
    expect(screen.getByText('Proxmox')).toBeInTheDocument();
    expect(screen.queryByText('Web')).not.toBeInTheDocument();
    await user.clear(screen.getByRole('searchbox'));
    await user.click(screen.getByRole('button', { name: /^Lab/ }));
    expect(screen.queryByText('Proxmox')).not.toBeInTheDocument();
    expect(screen.getByText('Web')).toBeInTheDocument();
  });

  it('crée un hôte dans le groupe affiché', async () => {
    const user = userEvent.setup();
    render(<HostsView />);
    await user.click(screen.getByRole('button', { name: /^Lab/ }));
    await user.click(screen.getByRole('button', { name: 'Nouvel hôte' }));
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByLabelText('Adresse'), 'nas.lab');
    await user.selectOptions(within(dialog).getByLabelText('Identité'), 'i-1');
    await user.type(within(dialog).getByLabelText('Étiquettes'), 'stockage, lab');
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    expect(ipc.hostSave).toHaveBeenCalledWith({
      id: null,
      name: 'nas.lab',
      address: 'nas.lab',
      port: 22,
      username: '',
      identityId: 'i-1',
      groupId: 'g-lab',
      tags: ['stockage', 'lab'],
      notes: '',
      protocol: { kind: 'ssh' },
    });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('exige un utilisateur quand aucune identité n’est choisie', async () => {
    const user = userEvent.setup();
    render(<HostsView />);
    await user.click(screen.getByRole('button', { name: 'Nouvel hôte' }));
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByLabelText('Adresse'), 'nas.lab');
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    expect(within(dialog).getByRole('alert')).toHaveTextContent('utilisateur');
    expect(ipc.hostSave).not.toHaveBeenCalled();
  });

  it('demande confirmation avant de supprimer un hôte', async () => {
    const user = userEvent.setup();
    render(<HostsView />);
    await user.click(screen.getByRole('button', { name: 'Supprimer Web' }));
    const dialog = screen.getByRole('alertdialog');
    expect(dialog).toHaveTextContent('« Web »');
    await user.click(within(dialog).getByRole('button', { name: 'Annuler' }));
    expect(ipc.hostDelete).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Supprimer Web' }));
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }),
    );
    expect(ipc.hostDelete).toHaveBeenCalledWith('h-web');
  });

  it('affiche une erreur du cœur', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.groupDelete).mockRejectedValueOnce({ code: 'vault.entry_not_found' });
    render(<HostsView />);
    await user.click(screen.getByRole('button', { name: /^Lab/ }));
    await user.click(screen.getByRole('button', { name: 'Supprimer' }));
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }),
    );
    expect(await screen.findByRole('alert')).toHaveTextContent("Cet élément n'existe plus.");
  });

  it('crée un hôte RDP avec domaine et résolution', async () => {
    const user = userEvent.setup();
    render(<HostsView />);
    await user.click(screen.getByRole('button', { name: 'Nouvel hôte' }));
    const dialog = screen.getByRole('dialog');
    await user.selectOptions(within(dialog).getByLabelText('Protocole'), 'rdp');
    expect(within(dialog).getByLabelText('Port')).toHaveValue('3389');
    await user.type(within(dialog).getByLabelText('Adresse'), 'win11.lab');
    await user.selectOptions(within(dialog).getByLabelText('Identité'), 'i-1');
    await user.type(within(dialog).getByLabelText('Domaine'), 'LAB');
    await user.click(within(dialog).getByLabelText('Plein écran'));
    await user.clear(within(dialog).getByLabelText('Largeur (px)'));
    await user.type(within(dialog).getByLabelText('Largeur (px)'), '1280');
    await user.clear(within(dialog).getByLabelText('Hauteur (px)'));
    await user.type(within(dialog).getByLabelText('Hauteur (px)'), '800');
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    expect(ipc.hostSave).toHaveBeenCalledWith(
      expect.objectContaining({
        address: 'win11.lab',
        port: 3389,
        protocol: {
          kind: 'rdp',
          domain: 'LAB',
          resolution: { mode: 'fixed', width: 1280, height: 800 },
        },
      }),
    );
  });

  it('refuse un domaine RDP qui pourrait injecter une option', async () => {
    const user = userEvent.setup();
    render(<HostsView />);
    await user.click(screen.getByRole('button', { name: 'Nouvel hôte' }));
    const dialog = screen.getByRole('dialog');
    await user.selectOptions(within(dialog).getByLabelText('Protocole'), 'rdp');
    await user.type(within(dialog).getByLabelText('Adresse'), 'win11.lab');
    await user.selectOptions(within(dialog).getByLabelText('Identité'), 'i-1');
    await user.type(within(dialog).getByLabelText('Domaine'), '/p:x');
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    expect(ipc.hostSave).not.toHaveBeenCalled();
    expect(within(dialog).getByRole('alert')).toBeInTheDocument();
  });

  it('ouvre un hôte RDP dans le client du système, sans onglet de terminal', async () => {
    const user = userEvent.setup();
    const rdpHost: HostEntry = host({
      id: 'h-rdp',
      name: 'Poste Windows',
      address: 'win11.lab',
      port: 3389,
      identityId: 'i-1',
      protocol: { kind: 'rdp', domain: '', resolution: { mode: 'fullscreen' } },
    });
    useVault.setState({
      status: 'unlocked',
      snapshot: { ...SNAPSHOT, hosts: [...SNAPSHOT.hosts, rdpHost] },
    });
    render(<HostsView />);
    await user.dblClick(screen.getByText('Poste Windows'));
    expect(ipc.rdpLaunch).toHaveBeenCalledWith('h-rdp');
    expect(openHost).not.toHaveBeenCalled();
    expect(await screen.findByRole('status')).toHaveTextContent('Poste Windows');
  });
});
