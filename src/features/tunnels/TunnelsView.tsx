import {
  ArrowLeftRight,
  LoaderCircle,
  Pencil,
  Play,
  Plus,
  Square,
  Trash2,
  TriangleAlert,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '@/components/ConfirmDialog';
import { Button } from '@/components/ui/button';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { toAppError, tunnelDelete } from '@/lib/ipc';
import type { TunnelEntry } from '@/lib/types';
import { cn } from '@/lib/utils';
import { useTunnels } from '@/stores/tunnels';
import { useVault } from '@/stores/vault';

import { destinationLabel, listenLabel, openSshFlag } from './describe';
import { startTunnel } from './start';
import { TunnelForm } from './TunnelForm';

type Dialog =
  { kind: 'edit'; tunnel: TunnelEntry | null } | { kind: 'delete'; tunnel: TunnelEntry };

/** Gestionnaire de tunnels (TUN-02) : liste, état, démarrage/arrêt, création, modification. */
export function TunnelsView() {
  const { t } = useTranslation();
  const tunnels = useVault((s) => s.snapshot.tunnels);
  const hosts = useVault((s) => s.snapshot.hosts);
  const refresh = useVault((s) => s.refresh);
  const runs = useTunnels((s) => s.runs);
  const errors = useTunnels((s) => s.errors);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function remove(tunnel: TunnelEntry) {
    setDialog(null);
    setError(null);
    useTunnels.getState().stop(tunnel.id);
    try {
      await tunnelDelete(tunnel.id);
    } catch (failure) {
      setError(toAppError(failure).code);
    }
    await refresh();
  }

  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-4 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">{t('nav.tunnels')}</h1>
        <Button
          disabled={hosts.length === 0}
          onClick={() => {
            setDialog({ kind: 'edit', tunnel: null });
          }}
        >
          <Plus aria-hidden="true" />
          {t('tunnels.new')}
        </Button>
      </div>
      <p className="text-sm text-muted-foreground">{t('tunnels.intro')}</p>
      {error && <ErrorMessage code={error} />}
      {hosts.length === 0 && <p className="text-sm">{t('tunnels.needHost')}</p>}
      {tunnels.length === 0 ? (
        <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed p-8 text-center">
          <ArrowLeftRight aria-hidden="true" className="size-8 text-muted-foreground" />
          <p className="text-sm text-muted-foreground">{t('tunnels.empty')}</p>
        </div>
      ) : (
        <ul className="flex flex-col gap-2" aria-label={t('tunnels.listLabel')}>
          {tunnels.map((tunnel) => {
            const host = hosts.find((h) => h.id === tunnel.hostId);
            const run = runs[tunnel.id];
            const exposed = tunnel.bindScope === 'allInterfaces';
            const lastError = errors[tunnel.id];
            return (
              <li
                key={tunnel.id}
                className={cn(
                  'flex flex-col gap-2 rounded-lg border p-3',
                  exposed && 'border-amber-500/70',
                )}
              >
                <div className="flex items-start gap-3">
                  <div className="min-w-0 flex-1">
                    <p className="flex flex-wrap items-center gap-2 font-medium">
                      {tunnel.name}
                      <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                        {t(`tunnels.kindShort.${tunnel.kind}`)}
                      </span>
                      {exposed && (
                        <span className="inline-flex items-center gap-1 rounded bg-amber-500/15 px-1.5 py-0.5 text-xs font-semibold text-amber-700 dark:text-amber-400">
                          <TriangleAlert aria-hidden="true" className="size-3.5" />
                          {t('tunnels.exposedBadge')}
                        </span>
                      )}
                      {tunnel.autoStart && (
                        <span className="rounded bg-muted px-1.5 py-0.5 text-xs">
                          {t('tunnels.autoStartBadge')}
                        </span>
                      )}
                    </p>
                    <p className="text-sm text-muted-foreground">
                      {t(`tunnels.summary.${tunnel.kind}`, {
                        listen: listenLabel(tunnel),
                        dest: destinationLabel(tunnel),
                        host: host?.name ?? '?',
                      })}
                    </p>
                    <p className="font-mono text-xs text-muted-foreground">{openSshFlag(tunnel)}</p>
                  </div>
                  <div className="flex shrink-0 gap-1">
                    {run ? (
                      <Button
                        size="sm"
                        variant="outline"
                        aria-label={t('tunnels.stop', { name: tunnel.name })}
                        onClick={() => {
                          useTunnels.getState().stop(tunnel.id);
                        }}
                      >
                        <Square aria-hidden="true" />
                        {t('tunnels.stopShort')}
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        aria-label={t('tunnels.start', { name: tunnel.name })}
                        disabled={!host}
                        onClick={() => {
                          startTunnel(tunnel, hosts);
                        }}
                      >
                        <Play aria-hidden="true" />
                        {t('tunnels.startShort')}
                      </Button>
                    )}
                    <Button
                      size="icon"
                      variant="ghost"
                      className="size-8"
                      disabled={run !== undefined}
                      aria-label={t('tunnels.edit', { name: tunnel.name })}
                      title={run ? t('tunnels.stopToEdit') : t('common.edit')}
                      onClick={() => {
                        setDialog({ kind: 'edit', tunnel });
                      }}
                    >
                      <Pencil aria-hidden="true" />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      className="size-8"
                      aria-label={t('tunnels.delete', { name: tunnel.name })}
                      title={t('common.delete')}
                      onClick={() => {
                        setDialog({ kind: 'delete', tunnel });
                      }}
                    >
                      <Trash2 aria-hidden="true" />
                    </Button>
                  </div>
                </div>
                <p role="status" className="flex items-center gap-2 text-sm">
                  {run?.status === 'connecting' && (
                    <>
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      {t('tunnels.status.connecting')}
                    </>
                  )}
                  {run?.status === 'running' && (
                    <>
                      <span aria-hidden="true" className="size-2 rounded-full bg-emerald-500" />
                      {t('tunnels.status.running', {
                        listen: listenLabel(tunnel),
                        active: run.active,
                        total: run.total,
                      })}
                    </>
                  )}
                  {!run && (
                    <span className="text-muted-foreground">{t('tunnels.status.stopped')}</span>
                  )}
                </p>
                {lastError && !run && <ErrorMessage code={lastError} />}
              </li>
            );
          })}
        </ul>
      )}
      {dialog?.kind === 'edit' && (
        <TunnelForm
          tunnel={dialog.tunnel}
          hosts={hosts}
          onSaved={() => void refresh()}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === 'delete' && (
        <ConfirmDialog
          title={t('tunnels.deleteTitle')}
          message={t('tunnels.deleteMessage', { name: dialog.tunnel.name })}
          confirmLabel={t('common.delete')}
          destructive
          onConfirm={() => void remove(dialog.tunnel)}
          onCancel={() => {
            setDialog(null);
          }}
        />
      )}
    </div>
  );
}
