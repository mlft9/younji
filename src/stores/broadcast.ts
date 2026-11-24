import { create } from 'zustand';

/**
 * Diffusion de la saisie (SSH-08). Jamais persistée : elle repart désactivée à chaque
 * démarrage, et s'arrête au verrouillage, à la fermeture des sessions ou dès qu'il reste moins
 * de deux cibles connectées.
 */
interface BroadcastState {
  /** Panneaux de terminal qui reçoivent la saisie (vide : diffusion inactive). */
  targets: string[];
  /** Dialogue de choix des cibles ouvert. */
  dialogOpen: boolean;
  /** Active la diffusion vers ces panneaux (au moins deux, sinon rien n'est activé). */
  start: (targets: string[]) => void;
  stop: () => void;
  /** Retire une cible (déconnectée ou fermée) ; arrête la diffusion s'il en reste moins de deux. */
  removeTarget: (id: string) => void;
  openDialog: () => void;
  closeDialog: () => void;
}

export const useBroadcast = create<BroadcastState>()((set, get) => ({
  targets: [],
  dialogOpen: false,
  start: (targets) => {
    const unique = [...new Set(targets)];
    set({ targets: unique.length >= 2 ? unique : [], dialogOpen: false });
  },
  stop: () => {
    set({ targets: [] });
  },
  removeTarget: (id) => {
    const { targets } = get();
    if (!targets.includes(id)) return;
    const rest = targets.filter((t) => t !== id);
    set({ targets: rest.length >= 2 ? rest : [] });
  },
  openDialog: () => {
    set({ dialogOpen: true });
  },
  closeDialog: () => {
    set({ dialogOpen: false });
  },
}));

/** Vrai si la diffusion est active. */
export function broadcastActive(state: Pick<BroadcastState, 'targets'>): boolean {
  return state.targets.length >= 2;
}
