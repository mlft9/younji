import { FolderTree, LoaderCircle, RotateCw, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { AuthPromptDialog } from '@/features/connect/AuthPromptDialog';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { HostKeyChangedAlert } from '@/features/connect/HostKeyChangedAlert';
import { HostKeyDialog } from '@/features/connect/HostKeyDialog';
import { QuickConnectForm } from '@/features/connect/QuickConnectForm';
import { SftpSidePanel } from '@/features/sftp/SftpSidePanel';
import { cn } from '@/lib/utils';
import { type ConnectTarget, HOME_TAB_ID, type SessionTab, useSessions } from '@/stores/sessions';
import { useUi } from '@/stores/ui';

import { closeTab, terminalSession } from './TerminalSession';
import { TerminalView } from './TerminalView';

/** Contenu d'un onglet de session : terminal et écran adapté à l'état de la connexion. */
/** Reconnexion : un hôte du carnet se reconnecte directement avec son identité enregistrée. */
function reconnect(tab: SessionTab, target: ConnectTarget | null): void {
  if (target?.hostId) {
    void terminalSession(tab.id).connectHost({ ...target, hostId: target.hostId });
  } else {
    useSessions.getState().update(tab.id, { status: 'form', errorCode: null });
  }
}

export function SessionPanel({ tab, active }: { tab: SessionTab; active: boolean }) {
  const { t } = useTranslation();
  const target = tab.target;
  const fromBook = Boolean(target?.hostId);

  return (
    <div
      role="tabpanel"
      id={`panel-${tab.id}`}
      aria-labelledby={`tab-${tab.id}`}
      hidden={!active}
      className={cn('flex min-h-0 flex-1', !active && 'hidden')}
    >
      <div className="relative min-h-0 min-w-0 flex-1">
        <TerminalView tabId={tab.id} active={active} interactive={tab.status === 'connected'} />

        {tab.status === 'connected' && !tab.sftpPanelOpen && (
          <Button
            size="icon"
            variant="outline"
            className="absolute top-2 right-4 z-10 size-8 opacity-60 hover:opacity-100 focus-visible:opacity-100"
            aria-label={t('sftp.openPanel')}
            title={t('sftp.openPanelShortcut')}
            onClick={() => {
              useSessions.getState().update(tab.id, { sftpPanelOpen: true });
            }}
          >
            <FolderTree aria-hidden="true" />
          </Button>
        )}

        {tab.status === 'form' && fromBook && (
          <div className="absolute inset-0 z-20 flex items-start justify-center overflow-auto bg-background/95 p-8">
            <div className="flex w-full max-w-md flex-col gap-4 rounded-lg border bg-background p-6 shadow-sm">
              <h2 className="text-lg font-semibold">
                {t('session.hostFailedTitle', { host: target?.label ?? target?.host ?? '' })}
              </h2>
              <ErrorMessage code={tab.errorCode ?? 'unknown'} />
              <div className="flex justify-end gap-2">
                <Button
                  variant="outline"
                  onClick={() => {
                    closeTab(tab.id);
                  }}
                >
                  <X aria-hidden="true" />
                  {t('session.closeTab')}
                </Button>
                <Button
                  onClick={() => {
                    reconnect(tab, target);
                  }}
                >
                  <RotateCw aria-hidden="true" />
                  {t('session.retry')}
                </Button>
              </div>
            </div>
          </div>
        )}

        {tab.status === 'form' && !fromBook && (
          <div className="absolute inset-0 z-20 flex items-start justify-center overflow-auto bg-background/95 p-8">
            <QuickConnectForm
              // Nouvelle instance à chaque retour au formulaire : valeurs pré-remplies à jour.
              key={`${tab.errorCode ?? ''}-${target?.host ?? ''}`}
              initial={target}
              errorCode={tab.errorCode}
              onSubmit={(next, secrets) => {
                void terminalSession(tab.id).connect(next, secrets);
              }}
            />
          </div>
        )}

        {(tab.status === 'connecting' || tab.status === 'authenticating') && target && (
          <div
            role="status"
            className="absolute top-3 left-1/2 z-20 flex -translate-x-1/2 items-center gap-2 rounded-md border bg-background px-4 py-2 text-sm shadow-md"
          >
            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            {tab.status === 'connecting'
              ? t('session.connecting', { host: target.host })
              : t('session.authenticating')}
          </div>
        )}

        {tab.status === 'hostKey' && tab.pendingHostKey && target && (
          <HostKeyDialog
            host={target.host}
            port={target.port}
            hostKey={tab.pendingHostKey}
            onDecision={(accept) => void terminalSession(tab.id).decideHostKey(accept)}
          />
        )}

        {tab.status === 'authPrompt' && tab.pendingPrompt && target && (
          <AuthPromptDialog
            // Nouvelle instance à chaque demande : les champs repartent vides.
            key={JSON.stringify(tab.pendingPrompt)}
            address={target.port === 22 ? target.host : `${target.host}:${target.port}`}
            prompt={tab.pendingPrompt}
            errorCode={tab.errorCode}
            onSubmit={(answers) => void terminalSession(tab.id).answerPrompt(answers)}
            onCancel={() => void terminalSession(tab.id).answerPrompt(null)}
          />
        )}

        {tab.status === 'blocked' && tab.hostKeyChange && target && (
          <div className="absolute inset-0 z-20 flex items-start justify-center overflow-auto bg-background/95 p-8">
            <HostKeyChangedAlert
              host={target.host}
              port={target.port}
              change={tab.hostKeyChange}
              onClose={() => {
                closeTab(tab.id);
              }}
              onManage={() => {
                useUi.getState().setActiveSection('keys');
                useSessions.getState().activate(HOME_TAB_ID);
              }}
            />
          </div>
        )}

        {tab.status === 'disconnected' && (
          <div className="absolute inset-x-4 bottom-4 z-20 flex flex-wrap items-center justify-between gap-3 rounded-md border bg-background p-3 shadow-lg">
            <ErrorMessage code={tab.errorCode ?? 'ssh.connection_lost'} />
            <div className="flex gap-2">
              <Button
                size="sm"
                onClick={() => {
                  reconnect(tab, target);
                }}
              >
                <RotateCw aria-hidden="true" />
                {t('session.reconnect')}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  closeTab(tab.id);
                }}
              >
                <X aria-hidden="true" />
                {t('session.closeTab')}
              </Button>
            </div>
          </div>
        )}
      </div>
      {tab.sftpPanelOpen && <SftpSidePanel tab={tab} />}
    </div>
  );
}
