import { useSessions } from '@/stores/sessions';

import type { NavigationDirection, SplitDirection } from './layout';
import { neighborPane } from './layout';
import { terminalSession } from './TerminalSession';

/**
 * Découpe un panneau (SSH-07). Le nouveau panneau reprend la cible du panneau d'origine : un
 * hôte du carnet s'y connecte directement (identité du coffre, aucun secret dans l'interface) ;
 * une connexion rapide pré-remplit le formulaire (le mot de passe est redemandé).
 * Renvoie l'identifiant du nouveau panneau, ou `null` si la limite est atteinte.
 */
export function splitAndConnect(paneId: string, direction: SplitDirection): string | null {
  const sessions = useSessions.getState();
  const source = sessions.tabs.find((t) => t.id === paneId);
  const id = sessions.splitPane(paneId, direction);
  if (!id || !source?.target) return id;
  const target = source.target;
  if (target.hostId) {
    void terminalSession(id).connectHost({ ...target, hostId: target.hostId });
  } else {
    useSessions.getState().update(id, { target });
  }
  return id;
}

/** Donne le focus au panneau voisin dans une direction. Renvoie `false` s'il n'y en a pas. */
export function focusNeighbor(paneId: string, direction: NavigationDirection): boolean {
  const sessions = useSessions.getState();
  const pane = sessions.tabs.find((t) => t.id === paneId);
  if (!pane) return false;
  const layout = sessions.layouts[pane.paneOf ?? pane.id];
  if (!layout) return false;
  const next = neighborPane(layout, paneId, direction);
  if (!next) return false;
  sessions.focusPane(next);
  return true;
}
