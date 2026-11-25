import { create } from 'zustand';

/** Sections de la barre latérale (APP-01). */
export const SECTIONS = ['hosts', 'local', 'keys', 'snippets', 'sftp', 'settings'] as const;
export type Section = (typeof SECTIONS)[number];

interface UiState {
  activeSection: Section;
  setActiveSection: (section: Section) => void;
}

/** État de navigation de l'interface (non persisté). */
export const useUi = create<UiState>()((set) => ({
  activeSection: 'hosts',
  setActiveSection: (activeSection) => {
    set({ activeSection });
  },
}));
