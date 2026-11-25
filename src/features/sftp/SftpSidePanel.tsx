import { Columns2, LoaderCircle, Radar, RotateCw, X } from 'lucide-react';
import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { existingTerminalSession } from '@/features/terminal/TerminalSession';
import { type SessionTab, useSessions } from '@/stores/sessions';

import { ConflictDialog } from './ConflictDialog';
import { RemoteBrowser } from './RemoteBrowser';
import { TransferList } from './TransferList';
import { usePendingConflict } from './usePendingConflict';
import { useSftpClient } from './useSftpClient';

/**
 * Panneau SFTP à côté du terminal (SFTP-01). Il suit le dossier courant du shell annoncé par
 * OSC 7, tant que l'option « suivre » est active.
 */
export function SftpSidePanel({ tab }: { tab: SessionTab }) {
  const { t } = useTranslation();
  // Jamais sur un terminal local (aucune connexion SSH derrière).
  const connected = tab.status === 'connected' && tab.local === null ? tab.sessionId : null;
  const { sftpId, errorCode, opening, retry } = useSftpClient(connected);
  // Navigation manuelle, avec le dossier du shell au moment où elle a eu lieu : quand le shell
  // change de dossier ensuite, le suivi reprend la main (si l'option est active).
  const [manual, setManual] = useState<{ cwd: string | null; path: string } | null>(null);
  const [follow, setFollow] = useState(true);
  const conflict = usePendingConflict(sftpId);
  const cwd = tab.remoteCwd;
  const followsShell = follow && cwd !== null && manual?.cwd !== cwd;
  const path = followsShell ? cwd : (manual?.path ?? cwd);

  const navigate = useCallback(
    (next: string) => {
      setManual({ cwd, path: next });
    },
    [cwd],
  );

  return (
    <aside
      aria-label={t('sftp.panel')}
      className="relative flex w-80 shrink-0 flex-col border-l bg-background xl:w-96"
    >
      <div className="flex items-center gap-1 border-b px-2 py-1">
        <h2 className="flex-1 text-sm font-semibold">{t('nav.sftp')}</h2>
        <Button
          size="icon"
          variant="ghost"
          className="size-7"
          aria-label={t('sftp.openDual')}
          title={t('sftp.openDual')}
          onClick={() => {
            useSessions.getState().openSftpTab(tab.id);
          }}
        >
          <Columns2 aria-hidden="true" />
        </Button>
        <Button
          size="icon"
          variant="ghost"
          className="size-7"
          aria-label={t('sftp.closePanel')}
          title={t('sftp.closePanel')}
          onClick={() => {
            useSessions.getState().update(tab.id, { sftpPanelOpen: false });
          }}
        >
          <X aria-hidden="true" />
        </Button>
      </div>
      <div className="flex flex-wrap items-center gap-2 border-b px-2 py-1 text-xs">
        <label className="flex items-center gap-1.5">
          <input
            type="checkbox"
            checked={follow}
            onChange={(e) => {
              setFollow(e.target.checked);
              // Réactivé : le dossier du shell reprend la main. Désactivé : le dossier affiché
              // est figé.
              setManual(e.target.checked || path === null ? null : { cwd, path });
            }}
            className="size-3.5"
          />
          {t('sftp.follow')}
        </label>
        {tab.remoteCwd === null && (
          <Button
            size="sm"
            variant="ghost"
            className="h-6 px-2 text-xs"
            title={t('sftp.integrationHint')}
            onClick={() => {
              existingTerminalSession(tab.id)?.sendShellIntegration();
            }}
          >
            <Radar aria-hidden="true" />
            {t('sftp.enableFollow')}
          </Button>
        )}
      </div>

      {connected === null && (
        <p className="p-3 text-sm text-muted-foreground">{t('sftp.notConnected')}</p>
      )}
      {connected !== null && opening && (
        <p role="status" className="flex items-center gap-2 p-3 text-sm">
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
          {t('sftp.opening')}
        </p>
      )}
      {connected !== null && errorCode && (
        <div className="flex flex-col gap-2 p-3">
          <ErrorMessage code={errorCode} />
          <Button size="sm" variant="outline" onClick={retry}>
            <RotateCw aria-hidden="true" />
            {t('common.retry')}
          </Button>
        </div>
      )}
      {sftpId !== null && (
        <>
          <RemoteBrowser sftpId={sftpId} path={path} onPathChange={navigate} compact />
          <TransferList sftpId={sftpId} />
          {conflict && <ConflictDialog key={conflict.job} transfer={conflict} />}
        </>
      )}
    </aside>
  );
}
