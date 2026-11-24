import { create } from 'zustand';

import type { EntryId } from '@/lib/types';

/** Dialogues d'envoi de snippet (SNP-02, SNP-03). État d'interface, jamais persisté. */
interface SnippetRunState {
  /** Choix d'un snippet à envoyer vers ces panneaux (`null` : fermé). */
  pickerTargets: string[] | null;
  /** Envoi en préparation : snippet choisi et cibles proposées. */
  run: { snippetId: EntryId; targets: string[] } | null;
  openPicker: (targets: string[]) => void;
  openRun: (snippetId: EntryId, targets: string[]) => void;
  close: () => void;
}

export const useSnippetRun = create<SnippetRunState>()((set) => ({
  pickerTargets: null,
  run: null,
  openPicker: (targets) => {
    set({ pickerTargets: targets, run: null });
  },
  openRun: (snippetId, targets) => {
    set({ pickerTargets: null, run: { snippetId, targets } });
  },
  close: () => {
    set({ pickerTargets: null, run: null });
  },
}));
