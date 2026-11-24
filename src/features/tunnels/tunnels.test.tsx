import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import type { HostEntry, SessionEvent, TunnelEntry } from '@/lib/types';
import { useTunnels } from '@/stores/tunnels';
import { EMPTY_SNAPSHOT, useVault } from '@/stores/vault';

import { isValidDestination, isValidPort, listenLabel, openSshFlag } from './describe';
import { TunnelBackground } from './TunnelBackground';
import { TunnelsView } from './TunnelsView';

vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return {
    ...actual,
    tunnelSave: vi.fn().mockResolvedValue('t-new'),
    tunnelDelete: vi.fn().mockResolvedValue(undefined),
    tunnelStart: vi.fn(),
    sshClose: vi.fn().mockResolvedValue(undefined),
    sshHostKeyDecision: vi.fn().mockResolvedValue(undefined),
    sshAuthAnswer: vi.fn().mockResolvedValue(undefined),
    vaultSnapshot: vi.fn(),
  };
});

const HOST: HostEntry = {
  id: 'h-1',
  name: 'VM Debian',
  address: '192.168.1.20',
  port: 22,
  username: 'admin',
  identityId: null,
  groupId: null,
  tags: [],
  notes: '',
};

const WEB: TunnelEntry = {
  id: 't-web',
  name: 'Web de la VM',
  hostId: 'h-1',
  kind: 'local',
  bindScope: 'loopback',
  bindPort: 8080,
  destHost: '127.0.0.1',
  destPort: 8080,
  autoStart: false,
};

const SOCKS: TunnelEntry = {
  ...WEB,
  id: 't-socks',
  name: 'Proxy',
  kind: 'dynamic',
  bindScope: 'allInterfaces',
  bindPort: 1080,
  destHost: '',
  destPort: 0,
  autoStart: true,
};

/** Simule `tunnel_start` : renvoie `sessionId` et garde l'émetteur d'événements. */
function mockStart(sessionId = 7) {
  const emitters: ((event: SessionEvent) => void)[] = [];
  vi.mocked(ipc.tunnelStart).mockImplementation((_id, onEvent) => {
    emitters.push(onEvent);
    return Promise.resolve(sessionId);
  });
  return (event: SessionEvent, index = emitters.length - 1) => {
    act(() => {
      emitters[index]?.(event);
    });
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  useTunnels.getState().reset();
  vi.mocked(ipc.vaultSnapshot).mockImplementation(() =>
    Promise.resolve(useVault.getState().snapshot),
  );
  useVault.setState({ snapshot: { ...EMPTY_SNAPSHOT, hosts: [HOST], tunnels: [WEB] } });
});

describe('description des tunnels', () => {
  it('donne l’équivalent OpenSSH et l’écoute', () => {
    expect(openSshFlag(WEB)).toBe('-L 127.0.0.1:8080:127.0.0.1:8080');
    expect(openSshFlag(SOCKS)).toBe('-D 0.0.0.0:1080');
    expect(openSshFlag({ ...WEB, kind: 'remote', destHost: '::1', destPort: 22 })).toBe(
      '-R 127.0.0.1:8080:[::1]:22',
    );
    expect(listenLabel(SOCKS)).toBe('0.0.0.0:1080');
  });

  it('valide les ports et les destinations comme le cœur', () => {
    expect(isValidPort(1)).toBe(true);
    expect(isValidPort(65535)).toBe(true);
    expect(isValidPort(0)).toBe(false);
    expect(isValidPort(65536)).toBe(false);
    expect(isValidPort(80.5)).toBe(false);
    expect(isValidDestination('intranet.lab')).toBe(true);
    expect(isValidDestination('[::1]')).toBe(true);
    expect(isValidDestination('a b')).toBe(false);
    expect(isValidDestination('x\u001b[31m')).toBe(false);
    expect(isValidDestination('')).toBe(false);
  });
});

describe('store des tunnels', () => {
  const host = { address: HOST.address, port: 22 };

  it('suit la connexion, les compteurs et l’arrêt', async () => {
    const emit = mockStart(7);
    await act(() => useTunnels.getState().start(WEB, host));
    expect(useTunnels.getState().runs[WEB.id]?.status).toBe('connecting');
    emit({ type: 'tunnelReady', port: 8080 });
    emit({ type: 'tunnelStats', active: 2, total: 5 });
    expect(useTunnels.getState().runs[WEB.id]).toMatchObject({
      status: 'running',
      active: 2,
      total: 5,
      sessionId: 7,
    });
    // Un second démarrage ne fait rien.
    await act(() => useTunnels.getState().start(WEB, host));
    expect(ipc.tunnelStart).toHaveBeenCalledTimes(1);

    act(() => {
      useTunnels.getState().stop(WEB.id);
    });
    expect(ipc.sshClose).toHaveBeenCalledWith(7);
    expect(useTunnels.getState().runs[WEB.id]).toBeUndefined();
    // La fin annoncée ensuite par le cœur n'est pas une erreur.
    emit({ type: 'disconnected', code: 'ssh.session_closed' });
    expect(useTunnels.getState().errors[WEB.id]).toBeUndefined();
  });

  it('garde la cause d’un échec et ignore les événements d’un démarrage précédent', async () => {
    const emit = mockStart();
    await act(() => useTunnels.getState().start(WEB, host));
    emit({ type: 'disconnected', code: 'ssh.tunnel_port_in_use' });
    expect(useTunnels.getState().errors[WEB.id]).toBe('ssh.tunnel_port_in_use');

    await act(() => useTunnels.getState().start(WEB, host));
    expect(useTunnels.getState().errors[WEB.id]).toBeUndefined();
    // Événement tardif du premier démarrage : sans effet sur le second.
    emit({ type: 'tunnelReady', port: 8080 }, 0);
    expect(useTunnels.getState().runs[WEB.id]?.status).toBe('connecting');
  });

  it('transmet les décisions de clé d’hôte et les réponses 2FA', async () => {
    const emit = mockStart(9);
    await act(() => useTunnels.getState().start(WEB, host));
    emit({ type: 'hostKeyUnknown', key: { algorithm: 'ssh-ed25519', fingerprint: 'SHA256:x' } });
    act(() => {
      useTunnels.getState().decideHostKey(WEB.id, true);
    });
    expect(ipc.sshHostKeyDecision).toHaveBeenCalledWith(9, true);
    emit({
      type: 'authPrompt',
      round: 1,
      name: '',
      instructions: '',
      prompts: [{ text: 'Code:', echo: false }],
    });
    act(() => {
      useTunnels.getState().answerPrompt(WEB.id, ['123456']);
    });
    expect(ipc.sshAuthAnswer).toHaveBeenCalledWith(9, 1, ['123456']);
    expect(useTunnels.getState().runs[WEB.id]?.pending).toBeNull();
  });

  it('ferme la session si l’arrêt est demandé pendant le démarrage', async () => {
    let resolve: (id: number) => void = () => undefined;
    vi.mocked(ipc.tunnelStart).mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const starting = useTunnels.getState().start(WEB, host);
    useTunnels.getState().stop(WEB.id);
    resolve(11);
    await act(() => starting);
    expect(ipc.sshClose).toHaveBeenCalledWith(11);
    expect(useTunnels.getState().runs[WEB.id]).toBeUndefined();
  });

  it('remonte l’erreur d’un démarrage refusé par le cœur', async () => {
    vi.mocked(ipc.tunnelStart).mockRejectedValue({ code: 'vault.locked' });
    await act(() => useTunnels.getState().start(WEB, host));
    expect(useTunnels.getState().errors[WEB.id]).toBe('vault.locked');
    expect(useTunnels.getState().runs[WEB.id]).toBeUndefined();
  });
});

describe('section Tunnels', () => {
  it('affiche l’état, les compteurs et un indicateur permanent pour 0.0.0.0', async () => {
    const user = userEvent.setup();
    useVault.setState({ snapshot: { ...EMPTY_SNAPSHOT, hosts: [HOST], tunnels: [WEB, SOCKS] } });
    const emit = mockStart();
    render(<TunnelsView />);
    const [web, socks] = screen.getAllByRole('listitem');
    if (!web || !socks) throw new Error('deux tunnels attendus');
    expect(within(web).queryByText(/Exposé au réseau/)).toBeNull();
    expect(within(socks).getByText(/Exposé au réseau/)).toBeInTheDocument();
    expect(within(web).getByText('Arrêté')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Démarrer Web de la VM' }));
    expect(ipc.tunnelStart).toHaveBeenCalledWith('t-web', expect.any(Function));
    emit({ type: 'tunnelReady', port: 8080 });
    emit({ type: 'tunnelStats', active: 1, total: 3 });
    expect(
      within(web).getByText(/Actif sur 127\.0\.0\.1:8080 — 1 connexion\(s\) en cours, 3/),
    ).toBeInTheDocument();
    // Modification impossible tant que le tunnel tourne.
    expect(screen.getByRole('button', { name: 'Modifier Web de la VM' })).toBeDisabled();

    await user.click(screen.getByRole('button', { name: 'Arrêter Web de la VM' }));
    expect(ipc.sshClose).toHaveBeenCalled();
    expect(within(web).getByText('Arrêté')).toBeInTheDocument();
  });

  it('crée un tunnel sur 127.0.0.1 par défaut', async () => {
    const user = userEvent.setup();
    useVault.setState({ snapshot: { ...EMPTY_SNAPSHOT, hosts: [HOST] } });
    render(<TunnelsView />);
    await user.click(screen.getByRole('button', { name: 'Nouveau tunnel' }));
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByLabelText('Nom'), 'Web');
    await user.type(within(dialog).getByLabelText("Port d'écoute"), '8080');
    await user.type(within(dialog).getByLabelText('Port de destination'), '80');
    expect(within(dialog).getByText('-L 127.0.0.1:8080:127.0.0.1:80')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    expect(ipc.tunnelSave).toHaveBeenCalledWith({
      id: null,
      name: 'Web',
      hostId: 'h-1',
      kind: 'local',
      bindScope: 'loopback',
      bindPort: 8080,
      destHost: '127.0.0.1',
      destPort: 80,
      autoStart: false,
      exposureConfirmed: false,
    });
  });

  it('exige une confirmation explicite pour écouter sur 0.0.0.0', async () => {
    const user = userEvent.setup();
    useVault.setState({ snapshot: { ...EMPTY_SNAPSHOT, hosts: [HOST] } });
    render(<TunnelsView />);
    await user.click(screen.getByRole('button', { name: 'Nouveau tunnel' }));
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByLabelText('Nom'), 'Proxy');
    await user.click(within(dialog).getByLabelText(/Dynamique SOCKS5/));
    // Pas de destination pour -D.
    expect(within(dialog).queryByLabelText('Port de destination')).toBeNull();
    await user.type(within(dialog).getByLabelText("Port d'écoute"), '1080');
    await user.selectOptions(within(dialog).getByLabelText('Écoute sur ce PC'), 'allInterfaces');
    expect(within(dialog).getByText(/rebondir sur le réseau du serveur/)).toBeInTheDocument();
    const save = within(dialog).getByRole('button', { name: 'Enregistrer' });
    expect(save).toBeDisabled();
    await user.click(within(dialog).getByLabelText("Je comprends et j'expose ce port au réseau"));
    expect(save).toBeEnabled();
    await user.click(save);
    expect(ipc.tunnelSave).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: 'dynamic',
        bindScope: 'allInterfaces',
        destHost: '',
        destPort: 0,
        exposureConfirmed: true,
      }),
    );
  });

  it('refuse un port invalide avant tout appel au cœur', async () => {
    const user = userEvent.setup();
    render(<TunnelsView />);
    await user.click(screen.getByRole('button', { name: 'Nouveau tunnel' }));
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByLabelText('Nom'), 'Web');
    await user.type(within(dialog).getByLabelText("Port d'écoute"), '70000');
    await user.type(within(dialog).getByLabelText('Port de destination'), '80');
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    expect(ipc.tunnelSave).not.toHaveBeenCalled();
    expect(within(dialog).getByRole('alert')).toHaveTextContent(/Tunnel invalide/);
  });

  it('arrête un tunnel avant de le supprimer', async () => {
    const user = userEvent.setup();
    mockStart(4);
    render(<TunnelsView />);
    await user.click(screen.getByRole('button', { name: 'Démarrer Web de la VM' }));
    await user.click(screen.getByRole('button', { name: 'Supprimer Web de la VM' }));
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }),
    );
    expect(ipc.sshClose).toHaveBeenCalledWith(4);
    expect(ipc.tunnelDelete).toHaveBeenCalledWith('t-web');
  });
});

describe('arrière-plan des tunnels', () => {
  it('démarre une seule fois les tunnels en démarrage automatique', () => {
    mockStart();
    useVault.setState({ snapshot: EMPTY_SNAPSHOT });
    const { rerender } = render(<TunnelBackground />);
    expect(ipc.tunnelStart).not.toHaveBeenCalled();
    act(() => {
      useVault.setState({ snapshot: { ...EMPTY_SNAPSHOT, hosts: [HOST], tunnels: [WEB, SOCKS] } });
    });
    expect(ipc.tunnelStart).toHaveBeenCalledTimes(1);
    expect(ipc.tunnelStart).toHaveBeenCalledWith('t-socks', expect.any(Function));
    act(() => {
      useVault.setState({ snapshot: { ...EMPTY_SNAPSHOT, hosts: [HOST], tunnels: [SOCKS] } });
    });
    rerender(<TunnelBackground />);
    expect(ipc.tunnelStart).toHaveBeenCalledTimes(1);
  });

  it('affiche la clé d’hôte d’un tunnel quelle que soit la section', async () => {
    const user = userEvent.setup();
    const emit = mockStart(3);
    render(<TunnelBackground />);
    await act(() => useTunnels.getState().start(WEB, { address: HOST.address, port: 22 }));
    emit({ type: 'hostKeyUnknown', key: { algorithm: 'ssh-ed25519', fingerprint: 'SHA256:abc' } });
    expect(screen.getByText('SHA256:abc')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Refuser' }));
    expect(ipc.sshHostKeyDecision).toHaveBeenCalledWith(3, false);
    expect(screen.queryByText('SHA256:abc')).toBeNull();
  });
});
