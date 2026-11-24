import { create } from 'zustand';

import { toAppError, vaultSnapshot, vaultStatus } from '@/lib/ipc';
import type { LockReason, VaultSnapshot, VaultStatus } from '@/lib/types';

/** Contenu vide, utilisé tant que le coffre est verrouillé. */
export const EMPTY_SNAPSHOT: VaultSnapshot = {
  hosts: [],
  groups: [],
  identities: [],
  keys: [],
  snippets: [],
  tunnels: [],
  autoLockMinutes: 15,
};

interface VaultState {
  /** `null` : état pas encore connu (premier appel au cœur en cours ou impossible). */
  status: VaultStatus | null;
  /** Code d'erreur si l'état n'a pas pu être lu. */
  statusError: string | null;
  /** Cause du dernier verrouillage, pour l'expliquer à l'écran de déverrouillage. */
  lastLockReason: LockReason | null;
  /** Contenu affichable (aucun secret : le cœur ne renvoie que des DTO sans secret). */
  snapshot: VaultSnapshot;
  setStatus: (status: VaultStatus, reason?: LockReason | null) => void;
  /** Relit l'état du coffre. */
  refreshStatus: () => Promise<void>;
  /** Relit le contenu du coffre après une modification. */
  refresh: () => Promise<void>;
}

export const useVault = create<VaultState>()((set, get) => ({
  status: null,
  statusError: null,
  lastLockReason: null,
  snapshot: EMPTY_SNAPSHOT,

  setStatus: (status, reason = null) => {
    set({
      status,
      statusError: null,
      lastLockReason: status === 'unlocked' ? null : reason,
      // Le contenu déchiffré ne reste pas affiché une fois le coffre verrouillé.
      ...(status === 'unlocked' ? {} : { snapshot: EMPTY_SNAPSHOT }),
    });
  },

  refreshStatus: async () => {
    try {
      const status = await vaultStatus();
      get().setStatus(status, get().lastLockReason);
      if (status === 'unlocked') await get().refresh();
    } catch (error) {
      set({ status: null, statusError: toAppError(error).code });
    }
  },

  refresh: async () => {
    try {
      set({ snapshot: await vaultSnapshot() });
    } catch (error) {
      if (toAppError(error).code === 'vault.locked') get().setStatus('locked');
    }
  },
}));
