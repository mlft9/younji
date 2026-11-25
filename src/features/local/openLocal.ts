import { terminalSession } from '@/features/terminal/TerminalSession';
import type { LocalProfile } from '@/lib/types';
import { useSessions } from '@/stores/sessions';

/** Ouvre un onglet de terminal local avec un profil détecté par le cœur. */
export function openLocalTerminal(profile: LocalProfile): void {
  const id = useSessions.getState().openLocalTab(profile);
  void terminalSession(id).openLocal(profile);
}
