import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import { EMPTY_SNAPSHOT, useVault } from '@/stores/vault';

import { ChangePasswordForm } from './ChangePasswordForm';
import { SettingsView } from './SettingsView';

vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return {
    ...actual,
    appInfo: vi.fn().mockResolvedValue({ name: 'Younji', version: '0.1.0', commit: 'a14aa94' }),
    vaultChangePassword: vi.fn(),
    vaultSetAutoLock: vi.fn().mockResolvedValue(undefined),
    vaultSnapshot: vi.fn(),
    settingsSet: vi.fn().mockResolvedValue(undefined),
  };
});

const STRONG = 'cheval agrafe batterie correct 42';

async function fillNew(user: ReturnType<typeof userEvent.setup>, value: string, confirm = value) {
  await user.type(screen.getByLabelText('Nouveau mot de passe'), value);
  await user.type(screen.getByLabelText('Confirmer le mot de passe'), confirm);
}

describe('changement du mot de passe maître (VLT-04)', () => {
  beforeEach(() => {
    vi.mocked(ipc.vaultChangePassword).mockReset();
  });

  it('exige le mot de passe actuel', async () => {
    const user = userEvent.setup();
    render(<ChangePasswordForm />);
    await fillNew(user, STRONG);
    await user.click(screen.getByRole('button', { name: 'Changer le mot de passe' }));
    expect(screen.getByRole('alert')).toHaveTextContent('mot de passe actuel');
    expect(ipc.vaultChangePassword).not.toHaveBeenCalled();
  });

  it('envoie les deux mots de passe une fois, vide les champs et confirme', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.vaultChangePassword).mockResolvedValue(undefined);
    render(<ChangePasswordForm />);
    await user.type(screen.getByLabelText('Mot de passe actuel'), 'ancien mot de passe');
    await fillNew(user, STRONG);
    await waitFor(() => {
      expect(screen.getByTestId('password-strength')).toHaveTextContent('robuste');
    });
    await user.click(screen.getByRole('button', { name: 'Changer le mot de passe' }));
    expect(ipc.vaultChangePassword).toHaveBeenCalledWith('ancien mot de passe', STRONG);
    expect(screen.getByLabelText('Mot de passe actuel')).toHaveValue('');
    expect(screen.getByLabelText('Nouveau mot de passe')).toHaveValue('');
    expect(await screen.findByRole('status')).toHaveTextContent("L'ancien ne fonctionne plus");
  });

  it('affiche le refus générique du cœur pour un mauvais mot de passe actuel', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.vaultChangePassword).mockRejectedValue({
      code: 'vault.wrong_password_or_corrupted',
    });
    render(<ChangePasswordForm />);
    await user.type(screen.getByLabelText('Mot de passe actuel'), 'faux');
    await fillNew(user, STRONG);
    await waitFor(() => {
      expect(screen.getByTestId('password-strength')).toHaveTextContent('robuste');
    });
    await user.click(screen.getByRole('button', { name: 'Changer le mot de passe' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Mot de passe incorrect');
  });
});

describe('délai de verrouillage automatique (VLT-03)', () => {
  it('ne propose que des délais finis et les enregistre dans le coffre', async () => {
    const user = userEvent.setup();
    useVault.setState({ status: 'unlocked', snapshot: EMPTY_SNAPSHOT });
    vi.mocked(ipc.vaultSnapshot).mockResolvedValue({ ...EMPTY_SNAPSHOT, autoLockMinutes: 5 });
    render(<SettingsView />);
    const select = screen.getByLabelText('Verrouillage automatique');
    const values = [...select.querySelectorAll('option')].map((option) => option.value);
    expect(values).toEqual(['1', '5', '10', '15', '30', '60']);
    expect(select).toHaveValue('15');
    await user.selectOptions(select, '5');
    expect(ipc.vaultSetAutoLock).toHaveBeenCalledWith(5);
    await waitFor(() => {
      expect(select).toHaveValue('5');
    });
  });
});
