import {
  Folder,
  FolderPlus,
  Monitor,
  Pencil,
  Play,
  Plus,
  Search,
  Server,
  SquareTerminal,
  Trash2,
} from 'lucide-react';
import { Fragment, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '@/components/ConfirmDialog';
import { NameDialog } from '@/components/NameDialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { groupDelete, groupSave, hostDelete, rdpLaunch, toAppError } from '@/lib/ipc';
import type { GroupEntry, HostEntry } from '@/lib/types';
import { cn } from '@/lib/utils';
import { useSessions } from '@/stores/sessions';
import { useVault } from '@/stores/vault';

import { effectiveUsername, filterHosts, type GroupFilter } from './filter';
import { HostForm } from './HostForm';
import { openHost } from './openHost';

type Dialog =
  | { kind: 'host'; host: HostEntry | null }
  | { kind: 'group'; group: GroupEntry | null }
  | { kind: 'deleteHost'; host: HostEntry }
  | { kind: 'deleteGroup'; group: GroupEntry };

function sameFilter(a: GroupFilter, b: GroupFilter): boolean {
  return a.kind === b.kind && (a.kind !== 'group' || (b.kind === 'group' && a.id === b.id));
}

/** Carnet d'hôtes (HST-01, HST-02) : groupes, recherche, création, double-clic pour se connecter. */
export function HostsView() {
  const { t } = useTranslation();
  const snapshot = useVault((s) => s.snapshot);
  const refresh = useVault((s) => s.refresh);
  const [filter, setFilter] = useState<GroupFilter>({ kind: 'all' });
  const [query, setQuery] = useState('');
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Message d'information après l'ouverture d'un hôte RDP. */
  const [notice, setNotice] = useState<string | null>(null);

  const visible = filterHosts(snapshot.hosts, filter, query);
  const selectedGroup =
    filter.kind === 'group' ? snapshot.groups.find((g) => g.id === filter.id) : undefined;
  const groups = [...snapshot.groups].sort((a, b) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }),
  );

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
    } catch (failure) {
      setError(toAppError(failure).code);
    } finally {
      await refresh();
    }
  }

  function connect(host: HostEntry) {
    if (host.protocol.kind === 'rdp') {
      void launchRdp(host);
    } else {
      openHost(host, snapshot.identities);
    }
  }

  /** Hôte RDP : le cœur lance le client du système avec l'identité du coffre (RDP-02). */
  async function launchRdp(host: HostEntry) {
    setError(null);
    setNotice(null);
    try {
      const result = await rdpLaunch(host.id);
      setNotice(
        t(result.existingCredential ? 'hosts.rdpExistingCredential' : 'hosts.rdpLaunched', {
          name: host.name,
        }),
      );
    } catch (failure) {
      setError(toAppError(failure).code);
    }
  }

  const groupButton = (target: GroupFilter, label: string, count: number) => (
    <li>
      <button
        type="button"
        aria-current={sameFilter(filter, target) ? 'true' : undefined}
        onClick={() => {
          setFilter(target);
        }}
        className={cn(
          'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none',
          'hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50',
          sameFilter(filter, target) && 'bg-accent font-medium',
        )}
      >
        <Folder aria-hidden="true" className="size-4 shrink-0 opacity-70" />
        <span className="flex-1 truncate">{label}</span>
        <span className="text-xs text-muted-foreground">{count}</span>
      </button>
    </li>
  );

  return (
    <div className="flex h-full min-h-0">
      <nav
        aria-label={t('hosts.groups')}
        className="flex w-52 shrink-0 flex-col gap-2 border-r p-3"
      >
        <div className="flex items-center justify-between">
          <h2 className="text-xs font-semibold text-muted-foreground uppercase">
            {t('hosts.groups')}
          </h2>
          <Button
            size="icon"
            variant="ghost"
            className="size-7"
            aria-label={t('hosts.newGroup')}
            title={t('hosts.newGroup')}
            onClick={() => {
              setDialog({ kind: 'group', group: null });
            }}
          >
            <FolderPlus aria-hidden="true" />
          </Button>
        </div>
        <ul className="flex flex-col gap-0.5 overflow-auto">
          {groupButton({ kind: 'all' }, t('hosts.allHosts'), snapshot.hosts.length)}
          {groupButton(
            { kind: 'ungrouped' },
            t('hosts.ungrouped'),
            snapshot.hosts.filter((h) => h.groupId === null).length,
          )}
          {groups.map((group) => (
            <Fragment key={group.id}>
              {groupButton(
                { kind: 'group', id: group.id },
                group.name,
                snapshot.hosts.filter((h) => h.groupId === group.id).length,
              )}
            </Fragment>
          ))}
        </ul>
        {selectedGroup && (
          <div className="mt-auto flex gap-1">
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setDialog({ kind: 'group', group: selectedGroup });
              }}
            >
              <Pencil aria-hidden="true" />
              {t('common.rename')}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setDialog({ kind: 'deleteGroup', group: selectedGroup });
              }}
            >
              <Trash2 aria-hidden="true" />
              {t('common.delete')}
            </Button>
          </div>
        )}
      </nav>

      <section aria-labelledby="hosts-title" className="flex min-w-0 flex-1 flex-col gap-4 p-6">
        <div className="flex flex-wrap items-center gap-3">
          <h1 id="hosts-title" className="text-xl font-semibold">
            {t('nav.hosts')}
          </h1>
          <div className="relative ml-auto w-64">
            <Search
              aria-hidden="true"
              className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 opacity-60"
            />
            <Input
              type="search"
              aria-label={t('hosts.search')}
              placeholder={t('hosts.search')}
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
              }}
              className="pl-8"
              maxLength={256}
            />
          </div>
          <Button
            variant="outline"
            onClick={() => {
              useSessions.getState().openTab();
            }}
          >
            <SquareTerminal aria-hidden="true" />
            {t('hosts.quickConnect')}
          </Button>
          <Button
            onClick={() => {
              setDialog({ kind: 'host', host: null });
            }}
          >
            <Plus aria-hidden="true" />
            {t('hosts.newHost')}
          </Button>
        </div>

        {error && <ErrorMessage code={error} />}
        {notice && (
          <p role="status" className="rounded-md border bg-muted/50 px-3 py-2 text-sm">
            {notice}
          </p>
        )}

        {visible.length === 0 ? (
          <p className="py-12 text-center text-sm text-muted-foreground">
            {snapshot.hosts.length === 0 ? t('hosts.empty') : t('hosts.noMatch')}
          </p>
        ) : (
          <ul aria-label={t('hosts.list')} className="flex flex-col gap-1 overflow-auto">
            {visible.map((host) => {
              const identity = snapshot.identities.find((i) => i.id === host.identityId);
              const user = effectiveUsername(host.username, identity?.username);
              return (
                <li
                  key={host.id}
                  className="group flex items-center gap-2 rounded-md border px-3 py-2 hover:bg-accent/50"
                >
                  <button
                    type="button"
                    className="flex min-w-0 flex-1 items-center gap-3 rounded text-left outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                    title={t('hosts.doubleClickHint')}
                    onDoubleClick={() => {
                      connect(host);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') {
                        event.preventDefault();
                        connect(host);
                      }
                    }}
                  >
                    {host.protocol.kind === 'rdp' ? (
                      <Monitor aria-hidden="true" className="size-4 shrink-0 opacity-70" />
                    ) : (
                      <Server aria-hidden="true" className="size-4 shrink-0 opacity-70" />
                    )}
                    <span className="flex min-w-0 flex-col">
                      <span className="truncate text-sm font-medium">{host.name}</span>
                      <span className="truncate font-mono text-xs text-muted-foreground">
                        {user ? `${user}@` : ''}
                        {host.address}
                        {host.port === (host.protocol.kind === 'rdp' ? 3389 : 22)
                          ? ''
                          : `:${String(host.port)}`}
                        {host.protocol.kind === 'rdp' && (
                          <span className="ml-2 rounded bg-muted px-1 font-sans">
                            {t('hosts.rdpBadge')}
                          </span>
                        )}
                      </span>
                    </span>
                    {host.tags.length > 0 && (
                      <span className="ml-auto flex shrink-0 gap-1">
                        {host.tags.map((tag) => (
                          <span
                            key={tag}
                            className="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground"
                          >
                            {tag}
                          </span>
                        ))}
                      </span>
                    )}
                  </button>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="size-8"
                    aria-label={t('hosts.connectTo', { name: host.name })}
                    title={t(host.protocol.kind === 'rdp' ? 'hosts.openRdp' : 'hosts.connect')}
                    onClick={() => {
                      connect(host);
                    }}
                  >
                    <Play aria-hidden="true" />
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="size-8"
                    aria-label={t('hosts.editHost', { name: host.name })}
                    title={t('common.edit')}
                    onClick={() => {
                      setDialog({ kind: 'host', host });
                    }}
                  >
                    <Pencil aria-hidden="true" />
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="size-8"
                    aria-label={t('hosts.deleteHost', { name: host.name })}
                    title={t('common.delete')}
                    onClick={() => {
                      setDialog({ kind: 'deleteHost', host });
                    }}
                  >
                    <Trash2 aria-hidden="true" />
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      {dialog?.kind === 'host' && (
        <HostForm
          host={dialog.host}
          defaultGroupId={filter.kind === 'group' ? filter.id : null}
          groups={groups}
          identities={snapshot.identities}
          onSaved={() => void refresh()}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === 'group' && (
        <NameDialog
          title={dialog.group ? t('hosts.renameGroup') : t('hosts.newGroup')}
          label={t('hosts.groupName')}
          initial={dialog.group?.name ?? ''}
          onSave={async (name) => {
            await groupSave({ id: dialog.group?.id ?? null, name });
            await refresh();
          }}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === 'deleteHost' && (
        <ConfirmDialog
          title={t('hosts.deleteHostTitle')}
          message={t('hosts.deleteHostMessage', { name: dialog.host.name })}
          confirmLabel={t('common.delete')}
          destructive
          onCancel={() => {
            setDialog(null);
          }}
          onConfirm={() => {
            const target = dialog.host;
            setDialog(null);
            void run(() => hostDelete(target.id));
          }}
        />
      )}
      {dialog?.kind === 'deleteGroup' && (
        <ConfirmDialog
          title={t('hosts.deleteGroupTitle')}
          message={t('hosts.deleteGroupMessage', { name: dialog.group.name })}
          confirmLabel={t('common.delete')}
          destructive
          onCancel={() => {
            setDialog(null);
          }}
          onConfirm={() => {
            const target = dialog.group;
            setDialog(null);
            setFilter({ kind: 'all' });
            void run(() => groupDelete(target.id));
          }}
        />
      )}
    </div>
  );
}
