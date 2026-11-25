import { Columns2, FolderTree, Radio, Rows2, SquareTerminal, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { useBroadcast } from '@/stores/broadcast';
import { type SessionTab, useSessions } from '@/stores/sessions';
import { useSnippetRun } from '@/stores/snippetRun';

import { splitAndConnect } from './split';
import { closePane } from './TerminalSession';

const BUTTON = 'size-7';

/**
 * Barre d'outils d'un panneau : découpe, snippets, diffusion et SFTP quand il est connecté ;
 * fermeture dès que l'onglet est découpé, **quel que soit l'état** (un panneau ouvert par erreur
 * doit pouvoir être fermé depuis son formulaire de connexion). Placée au-dessus des écrans de
 * connexion (z-30).
 */
export function PaneToolbar({ tab, split }: { tab: SessionTab; split: boolean }) {
  const { t } = useTranslation();
  const connected = tab.status === 'connected';
  if (!connected && !split) return null;
  return (
    <div
      role="toolbar"
      aria-label={t('panes.toolbar')}
      className={cn(
        'absolute top-2 right-4 z-30 flex gap-1',
        connected && 'opacity-60 focus-within:opacity-100 hover:opacity-100',
      )}
    >
      {connected && (
        <>
          <Button
            size="icon"
            variant="outline"
            className={BUTTON}
            aria-label={t('panes.splitRight')}
            title={t('panes.splitRightShortcut')}
            onClick={() => {
              splitAndConnect(tab.id, 'row');
            }}
          >
            <Columns2 aria-hidden="true" />
          </Button>
          <Button
            size="icon"
            variant="outline"
            className={BUTTON}
            aria-label={t('panes.splitDown')}
            title={t('panes.splitDownShortcut')}
            onClick={() => {
              splitAndConnect(tab.id, 'column');
            }}
          >
            <Rows2 aria-hidden="true" />
          </Button>
          {/* Snippets, diffusion et SFTP visent des serveurs : absents d'un terminal local. */}
          {!tab.local && (
            <>
              <Button
                size="icon"
                variant="outline"
                className={BUTTON}
                aria-label={t('snippets.runInPane')}
                title={t('snippets.runInPaneShortcut')}
                onClick={() => {
                  useSnippetRun.getState().openPicker([tab.id]);
                }}
              >
                <SquareTerminal aria-hidden="true" />
              </Button>
              <Button
                size="icon"
                variant="outline"
                className={BUTTON}
                aria-label={t('broadcast.open')}
                title={t('broadcast.openShortcut')}
                onClick={() => {
                  useBroadcast.getState().openDialog();
                }}
              >
                <Radio aria-hidden="true" />
              </Button>
              {!tab.sftpPanelOpen && (
                <Button
                  size="icon"
                  variant="outline"
                  className={BUTTON}
                  aria-label={t('sftp.openPanel')}
                  title={t('sftp.openPanelShortcut')}
                  onClick={() => {
                    useSessions.getState().update(tab.id, { sftpPanelOpen: true });
                  }}
                >
                  <FolderTree aria-hidden="true" />
                </Button>
              )}
            </>
          )}
        </>
      )}
      {split && (
        <Button
          size="icon"
          variant="outline"
          className={BUTTON}
          aria-label={t('panes.close')}
          title={t('panes.closeShortcut')}
          onClick={() => {
            closePane(tab.id);
          }}
        >
          <X aria-hidden="true" />
        </Button>
      )}
    </div>
  );
}
