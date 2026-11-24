import { useEffect } from 'react';

import { focusNeighbor, splitAndConnect } from '@/features/terminal/split';
import { closePane, closeTab, existingTerminalSession } from '@/features/terminal/TerminalSession';
import { lockNow } from '@/features/vault/lock';
import { broadcastActive, useBroadcast } from '@/stores/broadcast';
import { activePaneId, useSessions } from '@/stores/sessions';
import { useSnippetRun } from '@/stores/snippetRun';

import { matchShortcut } from './shortcuts';

/** Vrai si le focus est dans un terminal xterm.js. */
function focusInTerminal(): boolean {
  return document.activeElement?.closest('.xterm') != null;
}

/**
 * Empêche le rechargement de la page (F5, Ctrl+R, Ctrl+Maj+R) hors du terminal : il
 * fermerait toutes les sessions. Dans le terminal, xterm.js consomme déjà ces touches
 * (Ctrl+R est la recherche inverse de bash).
 */
function isReloadKey(event: KeyboardEvent): boolean {
  return event.key === 'F5' || (event.ctrlKey && event.key.toLowerCase() === 'r');
}

/** Raccourcis globaux (APP-04), interceptés avant xterm.js. Monté une fois dans AppShell. */
export function useShortcuts() {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (isReloadKey(event) && !focusInTerminal()) {
        event.preventDefault();
        return;
      }
      const action = matchShortcut(event);
      if (!action) return;
      const sessions = useSessions.getState();
      // Onglet affiché, et son panneau actif s'il est découpé (SSH-07).
      const root = sessions.tabs.find((t) => t.id === sessions.activeTabId);
      const paneId = activePaneId(sessions);
      const tab = sessions.tabs.find((t) => t.id === paneId) ?? root;
      const terminal = tab ? existingTerminalSession(tab.id) : undefined;
      const isTerminal = tab?.kind === 'terminal';

      const handled = () => {
        event.preventDefault();
        event.stopPropagation();
      };

      switch (action) {
        case 'newTab':
          handled();
          sessions.openTab();
          break;
        case 'closeTab':
          handled();
          if (root) closeTab(root.id);
          break;
        case 'closePane':
          if (tab && isTerminal) {
            handled();
            closePane(tab.id);
          }
          break;
        case 'splitRight':
        case 'splitDown':
          if (tab && isTerminal) {
            handled();
            splitAndConnect(tab.id, action === 'splitRight' ? 'row' : 'column');
          }
          break;
        case 'focusLeft':
        case 'focusRight':
        case 'focusUp':
        case 'focusDown':
          // Seulement depuis un terminal : dans un champ, Ctrl+Maj+flèches sélectionne des mots.
          if (tab && isTerminal && focusInTerminal()) {
            handled();
            const direction = (
              { focusLeft: 'left', focusRight: 'right', focusUp: 'up', focusDown: 'down' } as const
            )[action];
            focusNeighbor(tab.id, direction);
          }
          break;
        case 'snippets':
          handled();
          useSnippetRun.getState().openPicker(tab && isTerminal ? [tab.id] : []);
          break;
        case 'broadcast': {
          // Arrêt en une frappe ; l'activation passe toujours par le dialogue de confirmation.
          handled();
          const broadcast = useBroadcast.getState();
          if (broadcastActive(broadcast)) broadcast.stop();
          else broadcast.openDialog();
          break;
        }
        case 'nextTab':
          handled();
          sessions.activateRelative(1);
          break;
        case 'previousTab':
          handled();
          sessions.activateRelative(-1);
          break;
        case 'moveTabLeft':
        case 'moveTabRight':
          handled();
          if (root) sessions.moveTab(root.id, action === 'moveTabLeft' ? -1 : 1);
          break;
        case 'search':
          if (tab && terminal && (tab.status === 'connected' || tab.status === 'disconnected')) {
            handled();
            sessions.update(tab.id, { searchOpen: true });
          }
          break;
        case 'copy':
          if (terminal && focusInTerminal()) {
            handled();
            void terminal.copySelection().catch(() => undefined);
          }
          break;
        case 'sftpPanel':
          // Panneau SFTP à côté du terminal (SFTP-01).
          if (tab?.kind === 'terminal') {
            handled();
            sessions.update(tab.id, { sftpPanelOpen: !tab.sftpPanelOpen });
          }
          break;
        case 'lock':
          // Verrouillage (VLT-03, APP-04) : prioritaire, même depuis le terminal.
          handled();
          void lockNow();
          break;
        case 'paste':
          // Pas de preventDefault : le navigateur déclenche le collage natif, intercepté par la
          // garde de collage du terminal. On empêche seulement xterm d'envoyer Ctrl+V au shell.
          if (terminal && focusInTerminal()) event.stopPropagation();
          break;
      }
    }
    window.addEventListener('keydown', onKeyDown, { capture: true });
    return () => {
      window.removeEventListener('keydown', onKeyDown, { capture: true });
    };
  }, []);
}
