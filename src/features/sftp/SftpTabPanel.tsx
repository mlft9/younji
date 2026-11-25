import { LoaderCircle, RotateCw, X } from 'lucide-react';
import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { closeTab } from '@/features/terminal/TerminalSession';
import type { LocalRootRef } from '@/lib/types';
import { cn } from '@/lib/utils';
import { type SessionTab, useSessions } from '@/stores/sessions';

import { ConflictDialog } from './ConflictDialog';
import { LocalBrowser } from './LocalBrowser';
import { RemoteBrowser } from './RemoteBrowser';
import { TransferList } from './TransferList';
import { usePendingConflict } from './usePendingConflict';
import { useSftpClient } from './useSftpClient';

/**
 * Onglet SFTP double panneau (SFTP-02) : dossier local à gauche, serveur à droite, glisser-
 * déposer entre les deux. Il utilise la connexion de son onglet de terminal (ADR-034) et se
 * ferme avec lui.
 */
export function SftpTabPanel({ tab, active }: { tab: SessionTab; active: boolean }) {
  const { t } = useTranslation();
  const parent = useSessions((s) => s.tabs.find((x) => x.id === tab.parentTabId));
  const connected =
    parent?.status === 'connected' && parent.local === null ? parent.sessionId : null;
  const { sftpId, errorCode, opening, retry } = useSftpClient(connected);
  const [remotePath, setRemotePath] = useState<string | null>(tab.remoteCwd);
  const [root, setRoot] = useState<LocalRootRef | null>(null);
  const [localPath, setLocalPath] = useState('');
  const conflict = usePendingConflict(sftpId);

  const navigateRemote = useCallback((next: string) => {
    setRemotePath(next);
  }, []);

  return (
    <div
      role="tabpanel"
      id={`panel-${tab.id}`}
      aria-labelledby={`tab-${tab.id}`}
      hidden={!active}
      className={cn('relative flex min-h-0 flex-1 flex-col', !active && 'hidden')}
    >
      {connected === null && (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 p-8 text-center">
          <p className="text-sm text-muted-foreground">{t('sftp.parentClosed')}</p>
          <Button
            variant="outline"
            onClick={() => {
              closeTab(tab.id);
            }}
          >
            <X aria-hidden="true" />
            {t('session.closeTab')}
          </Button>
        </div>
      )}
      {connected !== null && opening && (
        <p role="status" className="flex items-center gap-2 p-4 text-sm">
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
          {t('sftp.opening')}
        </p>
      )}
      {connected !== null && errorCode && (
        <div className="flex max-w-md flex-col gap-2 p-4">
          <ErrorMessage code={errorCode} />
          <Button size="sm" variant="outline" onClick={retry}>
            <RotateCw aria-hidden="true" />
            {t('common.retry')}
          </Button>
        </div>
      )}
      {sftpId !== null && (
        <>
          <div className="flex min-h-0 flex-1">
            <section
              aria-label={t('sftp.localPane')}
              className="flex min-w-0 flex-1 flex-col border-r"
            >
              <LocalBrowser
                sftpId={sftpId}
                root={root}
                path={localPath}
                onRootChange={(next) => {
                  setRoot(next);
                  setLocalPath('');
                }}
                onPathChange={setLocalPath}
              />
            </section>
            <section aria-label={t('sftp.remotePane')} className="flex min-w-0 flex-1 flex-col">
              <RemoteBrowser
                sftpId={sftpId}
                path={remotePath}
                onPathChange={navigateRemote}
                localTarget={root ? { rootId: root.id, path: localPath } : null}
              />
            </section>
          </div>
          <TransferList sftpId={sftpId} />
          {conflict && <ConflictDialog key={conflict.job} transfer={conflict} />}
        </>
      )}
    </div>
  );
}
