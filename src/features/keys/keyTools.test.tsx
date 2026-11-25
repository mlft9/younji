import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import type { HostEntry, KeyEntry, SessionEvent } from '@/lib/types';
import { EMPTY_SNAPSHOT, useVault } from '@/stores/vault';

import { KeyDeployDialog } from './KeyDeployDialog';
import { KeyGenerateDialog } from './KeyGenerateDialog';
import { KeysView } from './KeysView';

vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return {
    ...actual,
    keyGenerate: vi.fn(),
    keyExportPublic: vi.fn(),
    keyDeploy: vi.fn(),
    sshHostKeyDecision: vi.fn().mockResolvedValue(undefined),
    sshAuthAnswer: vi.fn().mockResolvedValue(undefined),
    sshClose: vi.fn().mockResolvedValue(undefined),
    knownHostList: vi.fn().mockResolvedValue([]),
    vaultSnapshot: vi.fn(),
  };
});

const KEY: KeyEntry = {
  id: 'k-1',
  name: 'lab',
  algorithm: 'ssh-ed25519',
  fingerprint: 'SHA256:abc',
  publicKey: 'ssh-ed25519 AAAAC3Nza',
};
const HOST_WITH_IDENTITY: HostEntry = {
  id: 'h-1',
  name: 'debian',
  address: 'srv.lab',
  port: 22,
  username: '',
  identityId: 'i-1',
  groupId: null,
  tags: [],
  notes: '',
  protocol: { kind: 'ssh' },
};
const HOST_WITHOUT_IDENTITY: HostEntry = {
  ...HOST_WITH_IDENTITY,
  id: 'h-2',
  name: 'nu',
  address: 'nu.lab',
  port: 2222,
  username: 'testpwd',
  identityId: null,
};

beforeEach(() => {
  vi.clearAllMocks();
});

describe('génération de clé (KEY-01)', () => {
  it('propose Ed25519 par défaut et envoie seulement le nom et le type', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.keyGenerate).mockResolvedValue(KEY);
    const onGenerated = vi.fn();
    const onClose = vi.fn();
    render(<KeyGenerateDialog onGenerated={onGenerated} onClose={onClose} />);
    expect(screen.getByLabelText('Type de clé')).toHaveValue('ed25519');
    await user.click(screen.getByRole('button', { name: 'Générer' }));
    expect(ipc.keyGenerate).toHaveBeenCalledWith({
      name: 'Clé Ed25519 (recommandé)',
      kind: 'ed25519',
    });
    expect(onGenerated).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
    // Aucun champ de passphrase : la clé ne quitte pas le coffre.
    expect(document.querySelector('input[type=password]')).toBeNull();
  });

  it('génère une clé RSA 4096 nommée, et affiche une erreur du cœur', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.keyGenerate).mockRejectedValue({ code: 'vault.invalid_name' });
    const onClose = vi.fn();
    render(<KeyGenerateDialog onGenerated={vi.fn()} onClose={onClose} />);
    await user.selectOptions(screen.getByLabelText('Type de clé'), 'rsa4096');
    expect(screen.getByText(/quelques secondes/)).toBeInTheDocument();
    await user.type(screen.getByLabelText('Nom'), 'ancien serveur');
    await user.click(screen.getByRole('button', { name: 'Générer' }));
    expect(ipc.keyGenerate).toHaveBeenCalledWith({ name: 'ancien serveur', kind: 'rsa4096' });
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('export de la clé publique (KEY-02)', () => {
  it('passe par la boîte de dialogue du cœur, sans chemin côté interface', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.keyExportPublic).mockResolvedValue(true);
    vi.mocked(ipc.vaultSnapshot).mockResolvedValue({ ...EMPTY_SNAPSHOT, keys: [KEY] });
    useVault.setState({ snapshot: { ...EMPTY_SNAPSHOT, keys: [KEY] } });
    render(<KeysView />);
    const button = screen.getByRole('button', { name: 'Exporter la clé publique de lab' });
    await user.click(button);
    expect(ipc.keyExportPublic).toHaveBeenCalledWith('k-1');
    expect(button).toHaveAttribute('title', 'Clé publique exportée');
  });
});

/** Rend le dialogue de déploiement et capture le flux d'événements du cœur. */
function renderDeploy(hosts: HostEntry[] = [HOST_WITH_IDENTITY, HOST_WITHOUT_IDENTITY]) {
  let onEvent: ((event: SessionEvent) => void) | undefined;
  vi.mocked(ipc.keyDeploy).mockImplementation((_request, listener) => {
    onEvent = listener;
    return Promise.resolve(42);
  });
  const onClose = vi.fn();
  const view = render(
    <KeyDeployDialog keyEntry={KEY} hosts={hosts} onClose={onClose} onManageHostKeys={vi.fn()} />,
  );
  const emit = (event: SessionEvent) => {
    act(() => {
      onEvent?.(event);
    });
  };
  return { emit, onClose, view };
}

describe('déploiement de clé (KEY-03)', () => {
  it('suit la clé d’hôte, le 2FA puis annonce l’ajout', async () => {
    const user = userEvent.setup();
    const { emit } = renderDeploy();
    // Hôte avec identité : aucun mot de passe demandé.
    expect(document.querySelector('input[type=password]')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Déployer' }));
    expect(ipc.keyDeploy).toHaveBeenCalledWith(
      { hostId: 'h-1', keyId: 'k-1', password: null },
      expect.any(Function),
    );
    expect(screen.getByText('Déploiement sur srv.lab…')).toBeInTheDocument();

    emit({ type: 'hostKeyUnknown', key: { algorithm: 'ssh-ed25519', fingerprint: 'SHA256:x' } });
    await user.click(screen.getByRole('button', { name: 'Accepter et se connecter' }));
    expect(ipc.sshHostKeyDecision).toHaveBeenCalledWith(42, true);

    emit({ type: 'authenticating' });
    emit({
      type: 'authPrompt',
      round: 1,
      name: '',
      instructions: '',
      prompts: [{ text: 'Verification code: ', echo: false }],
    });
    await user.type(screen.getByLabelText('Verification code:'), '123456');
    await user.click(screen.getByRole('button', { name: 'Valider' }));
    expect(ipc.sshAuthAnswer).toHaveBeenCalledWith(42, 1, ['123456']);

    emit({ type: 'keyDeployed', added: true });
    emit({ type: 'disconnected', code: 'ssh.session_ended' });
    expect(screen.getByText('Clé ajoutée sur srv.lab.')).toBeInTheDocument();
    expect(screen.getByText(/Clés > Identités/)).toBeInTheDocument();
  });

  it('signale une clé déjà présente', async () => {
    const user = userEvent.setup();
    const { emit } = renderDeploy();
    await user.click(screen.getByRole('button', { name: 'Déployer' }));
    emit({ type: 'keyDeployed', added: false });
    expect(screen.getByText(/déjà présente sur srv\.lab/)).toBeInTheDocument();
  });

  it('demande un mot de passe pour un hôte sans identité, l’envoie une fois puis le vide', async () => {
    const user = userEvent.setup();
    renderDeploy();
    await user.selectOptions(screen.getByLabelText('Hôte'), 'h-2');
    await user.type(screen.getByLabelText('Mot de passe'), 'hunter2');
    await user.click(screen.getByRole('button', { name: 'Déployer' }));
    expect(ipc.keyDeploy).toHaveBeenCalledWith(
      { hostId: 'h-2', keyId: 'k-1', password: 'hunter2' },
      expect.any(Function),
    );
  });

  it('revient au choix avec la cause en cas d’échec', async () => {
    const user = userEvent.setup();
    const { emit } = renderDeploy();
    await user.click(screen.getByRole('button', { name: 'Déployer' }));
    emit({ type: 'disconnected', code: 'ssh.deploy_failed' });
    expect(screen.getByRole('alert')).toHaveTextContent(/serveur Unix/);
    expect(screen.getByRole('button', { name: 'Déployer' })).toBeInTheDocument();
  });

  it('bloque sur une clé d’hôte modifiée', async () => {
    const user = userEvent.setup();
    const { emit } = renderDeploy();
    await user.click(screen.getByRole('button', { name: 'Déployer' }));
    const key = { algorithm: 'ssh-ed25519', fingerprint: 'SHA256:x' };
    emit({ type: 'hostKeyChanged', key, known: [key], hostPattern: 'srv.lab' });
    emit({ type: 'disconnected', code: 'ssh.host_key_changed' });
    expect(screen.getByRole('alert')).toHaveTextContent(/a changé : déploiement refusé/);
  });

  it('abandonne la session si le dialogue est fermé en cours de route', async () => {
    const user = userEvent.setup();
    const { view } = renderDeploy();
    await user.click(screen.getByRole('button', { name: 'Déployer' }));
    view.unmount();
    expect(ipc.sshClose).toHaveBeenCalledWith(42);
  });

  it('indique qu’il faut d’abord un hôte dans le carnet', () => {
    renderDeploy([]);
    expect(screen.getByText(/Aucun hôte dans le carnet/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Déployer' })).toBeDisabled();
  });
});
