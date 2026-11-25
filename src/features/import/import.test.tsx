import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { BackupSection } from '@/features/backup/BackupSection';
import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import type { ConfigImportPreview, KnownHostsImportPreview } from '@/lib/types';
import { EMPTY_SNAPSHOT, useVault } from '@/stores/vault';

import { KnownHostsImportDialog } from './KnownHostsImportDialog';
import { SshConfigImportDialog } from './SshConfigImportDialog';

vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return {
    ...actual,
    importSshConfigApply: vi.fn(),
    importKnownHostsApply: vi.fn(),
    backupExport: vi.fn(),
    backupImport: vi.fn(),
    vaultSnapshot: vi.fn(),
  };
});

const CONFIG: ConfigImportPreview = {
  previewId: 7,
  hosts: [
    {
      index: 0,
      alias: 'pve',
      address: 'pve.lab',
      port: 22,
      username: 'root',
      warnings: ['identity_file'],
      identityFiles: ['~/.ssh/id_lab'],
      alreadyInVault: false,
    },
    {
      index: 1,
      alias: 'web',
      address: '203.0.113.10',
      port: 2222,
      username: 'deploy',
      warnings: ['proxy_ignored'],
      identityFiles: [],
      alreadyInVault: true,
    },
  ],
  skipped: [{ entry: 'Host *.lab', reason: 'wildcard' }],
};

describe('import de ~/.ssh/config (IMP-01)', () => {
  beforeEach(() => {
    vi.mocked(ipc.importSshConfigApply).mockReset();
  });

  it('montre les avertissements et les entrées ignorées avec leur raison', () => {
    render(<SshConfigImportDialog preview={CONFIG} onImported={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByText(/Clé à importer à part.*~\/\.ssh\/id_lab/)).toBeInTheDocument();
    expect(screen.getByText(/ProxyJump\/ProxyCommand ignoré/)).toBeInTheDocument();
    expect(screen.getByText('Host *.lab')).toBeInTheDocument();
    expect(screen.getByText(/motif à joker/)).toBeInTheDocument();
  });

  it('décoche les hôtes déjà présents et n’envoie que les numéros cochés', async () => {
    const user = userEvent.setup();
    const onImported = vi.fn();
    vi.mocked(ipc.importSshConfigApply).mockResolvedValue(1);
    render(<SshConfigImportDialog preview={CONFIG} onImported={onImported} onClose={vi.fn()} />);
    const list = screen.getByRole('list', { name: 'Hôtes trouvés' });
    const boxes = within(list).getAllByRole('checkbox');
    expect(boxes[0]).toBeChecked();
    expect(boxes[1]).not.toBeChecked();
    await user.click(screen.getByRole('button', { name: 'Importer 1 hôte' }));
    expect(ipc.importSshConfigApply).toHaveBeenCalledWith(7, [0]);
    expect(onImported).toHaveBeenCalledWith(1);
  });

  it('affiche un aperçu périmé', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.importSshConfigApply).mockRejectedValue({ code: 'import.stale' });
    render(<SshConfigImportDialog preview={CONFIG} onImported={vi.fn()} onClose={vi.fn()} />);
    await user.click(screen.getByRole('button', { name: 'Importer 1 hôte' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('aperçu');
  });
});

const KNOWN: KnownHostsImportPreview = {
  previewId: 3,
  addedCount: 1,
  added: [{ hosts: 'neuf.lab', algorithm: 'ssh-ed25519' }],
  duplicates: 2,
  skippedCount: 1,
  skipped: [{ line: 4, hosts: 'srv.lab', reason: 'conflict' }],
};

describe('import de known_hosts (IMP-02)', () => {
  it('signale les conflits et applique l’aperçu par son identifiant', async () => {
    const user = userEvent.setup();
    const onImported = vi.fn();
    vi.mocked(ipc.importKnownHostsApply).mockResolvedValue(undefined);
    render(<KnownHostsImportDialog preview={KNOWN} onImported={onImported} onClose={vi.fn()} />);
    expect(screen.getByText(/hôte déjà connu avec une autre clé/)).toBeInTheDocument();
    expect(screen.getByText(/1 clé\(s\) à ajouter, 2 déjà connue\(s\)/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Ajouter 1 clé' }));
    expect(ipc.importKnownHostsApply).toHaveBeenCalledWith(3);
    expect(onImported).toHaveBeenCalled();
  });
});

const STRONG = 'cheval agrafe batterie correct 42';

/** Deux champs portent le même libellé : export (premier) et import (second). */
function exportField(): HTMLElement {
  const field = screen.getAllByLabelText('Phrase de passe de la sauvegarde')[0];
  if (!field) throw new Error('champ absent');
  return field;
}

function importField(): HTMLElement {
  const field = screen.getAllByLabelText('Phrase de passe de la sauvegarde')[1];
  if (!field) throw new Error('champ absent');
  return field;
}

describe('sauvegarde chiffrée (VLT-06)', () => {
  beforeEach(() => {
    vi.mocked(ipc.backupExport).mockReset();
    vi.mocked(ipc.backupImport).mockReset();
    vi.mocked(ipc.vaultSnapshot).mockResolvedValue(EMPTY_SNAPSHOT);
    useVault.setState({ status: 'unlocked', snapshot: EMPTY_SNAPSHOT });
  });

  it('exige une confirmation identique avant l’export', async () => {
    const user = userEvent.setup();
    render(<BackupSection />);
    await user.type(exportField(), STRONG);
    await user.type(screen.getByLabelText('Confirmer la phrase de passe'), `${STRONG}x`);
    await waitFor(() => {
      expect(screen.getByTestId('password-strength')).toHaveTextContent('robuste');
    });
    await user.click(screen.getByRole('button', { name: 'Exporter…' }));
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(ipc.backupExport).not.toHaveBeenCalled();
  });

  it('envoie la phrase de passe une fois et vide les champs', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.backupExport).mockResolvedValue(true);
    render(<BackupSection />);
    const field = exportField();
    await user.type(field, STRONG);
    await user.type(screen.getByLabelText('Confirmer la phrase de passe'), STRONG);
    await waitFor(() => {
      expect(screen.getByTestId('password-strength')).toHaveTextContent('robuste');
    });
    await user.click(screen.getByRole('button', { name: 'Exporter…' }));
    expect(ipc.backupExport).toHaveBeenCalledWith(STRONG);
    expect(field).toHaveValue('');
    expect(await screen.findByRole('status')).toHaveTextContent('Sauvegarde exportée');
  });

  it('demande confirmation avant de remplacer le coffre', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.backupImport).mockResolvedValue({
      hosts: 3,
      groups: 1,
      identities: 1,
      keys: 0,
      snippets: 0,
      knownHosts: 2,
      skipped: 0,
    });
    render(<BackupSection />);
    await user.type(importField(), 'phrase');
    await user.click(screen.getByLabelText(/Remplacer/));
    await user.click(screen.getByRole('button', { name: 'Choisir le fichier et importer…' }));
    expect(ipc.backupImport).not.toHaveBeenCalled();
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Remplacer' }),
    );
    expect(ipc.backupImport).toHaveBeenCalledWith('phrase', 'replace');
    expect(await screen.findByRole('status')).toHaveTextContent('3 hôte(s)');
  });

  it('fusionne par défaut, sans confirmation', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.backupImport).mockResolvedValue(null);
    render(<BackupSection />);
    await user.type(importField(), 'phrase');
    await user.click(screen.getByRole('button', { name: 'Choisir le fichier et importer…' }));
    expect(ipc.backupImport).toHaveBeenCalledWith('phrase', 'merge');
  });
});
