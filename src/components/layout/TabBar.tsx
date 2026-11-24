import { House, Plus, Radio, X } from 'lucide-react';
import { type KeyboardEvent, type RefObject, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { closeTab } from '@/features/terminal/TerminalSession';
import { cn } from '@/lib/utils';
import { useBroadcast } from '@/stores/broadcast';
import {
  defaultTabTitle,
  HOME_TAB_ID,
  MAX_TAB_TITLE,
  panesOf,
  type SessionTab,
  type TabStatus,
  useSessions,
} from '@/stores/sessions';

const STATUS_COLOR: Record<TabStatus, string> = {
  form: 'bg-muted-foreground/40',
  connecting: 'bg-amber-500',
  authenticating: 'bg-amber-500',
  hostKey: 'bg-amber-500',
  authPrompt: 'bg-amber-500',
  connected: 'bg-emerald-500',
  disconnected: 'bg-red-500',
  blocked: 'bg-red-500',
};

const TAB_CLASS =
  'flex h-8 items-center gap-2 rounded-t-md border border-b-0 px-3 text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50';

/** Déplace le focus vers l'onglet voisin (motif ARIA des onglets, flèches gauche/droite). */
function focusSibling(event: KeyboardEvent<HTMLElement>) {
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
  const tabs = [
    ...(event.currentTarget
      .closest('[role=tablist]')
      ?.querySelectorAll<HTMLElement>('[role=tab]') ?? []),
  ];
  const index = tabs.indexOf(event.currentTarget);
  const next = tabs[(index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length];
  event.preventDefault();
  next?.focus();
}

function SessionTabItem({
  tab,
  active,
  dragging,
  onDragStart,
}: {
  tab: SessionTab;
  active: boolean;
  dragging: RefObject<string | null>;
  onDragStart: (id: string) => void;
}) {
  const { t } = useTranslation();
  const activate = useSessions((s) => s.activate);
  const renameTab = useSessions((s) => s.renameTab);
  const moveTabTo = useSessions((s) => s.moveTabTo);
  const [editing, setEditing] = useState(false);
  // Chaîne plutôt que tableau : valeur stable pour le sélecteur.
  const paneList = useSessions((s) => panesOf(s, tab.id).join('\n'));
  const panes = paneList.split('\n');
  const paneCount = panes.length;
  // Onglet dont au moins un panneau reçoit la diffusion (SSH-08) : signalé dans la barre.
  const targets = useBroadcast((s) => s.targets);
  const broadcasting = targets.length >= 2 && targets.some((id) => panes.includes(id));
  const base = defaultTabTitle(tab) ?? t('tabs.newSession');
  const title =
    tab.kind === 'sftp' && !tab.customTitle ? t('sftp.tabTitle', { title: base }) : base;

  function commit(value: string) {
    renameTab(tab.id, value);
    setEditing(false);
  }

  return (
    <div
      className={cn(
        TAB_CLASS,
        'group max-w-56 pr-1',
        active
          ? 'bg-background'
          : 'border-transparent text-muted-foreground hover:bg-background/60',
      )}
      onPointerDown={(e) => {
        if (e.button === 0 && !editing) onDragStart(tab.id);
      }}
      onPointerEnter={(e) => {
        // Glisser-déposer par pointeur : l'onglet tiré prend la place de celui survolé.
        if (dragging.current && dragging.current !== tab.id && e.buttons === 1) {
          moveTabTo(dragging.current, tab.id);
        }
      }}
      onAuxClick={(e) => {
        if (e.button === 1) closeTab(tab.id);
      }}
    >
      <span
        aria-hidden="true"
        className={cn('size-2 shrink-0 rounded-full', STATUS_COLOR[tab.status])}
      />
      {broadcasting && (
        <Radio
          role="img"
          aria-label={t('broadcast.tabBadge')}
          className="size-3.5 shrink-0 text-red-600"
        />
      )}
      {paneCount > 1 && (
        <span
          className="shrink-0 rounded-sm bg-muted px-1 text-xs text-muted-foreground"
          title={t('panes.count', { count: paneCount })}
        >
          {paneCount}
        </span>
      )}
      {editing ? (
        <input
          // Champ créé à la demande de l'utilisateur (double-clic ou F2) : le focus y va.
          autoFocus
          defaultValue={tab.customTitle ?? defaultTabTitle(tab) ?? ''}
          aria-label={t('tabs.renameLabel')}
          maxLength={MAX_TAB_TITLE}
          className="w-36 rounded-sm bg-transparent px-1 outline-none ring-1 ring-ring"
          onFocus={(e) => {
            e.currentTarget.select();
          }}
          onBlur={(e) => {
            commit(e.currentTarget.value);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit(e.currentTarget.value);
            if (e.key === 'Escape') setEditing(false);
            e.stopPropagation();
          }}
        />
      ) : (
        <button
          type="button"
          role="tab"
          id={`tab-${tab.id}`}
          aria-selected={active}
          aria-controls={`panel-${tab.id}`}
          tabIndex={active ? 0 : -1}
          title={t('tabs.renameHint')}
          className="min-w-0 truncate outline-none"
          onClick={() => {
            activate(tab.id);
          }}
          onDoubleClick={() => {
            setEditing(true);
          }}
          onKeyDown={(e) => {
            if (e.key === 'F2') {
              e.preventDefault();
              setEditing(true);
            } else if (e.key === 'Delete') {
              e.preventDefault();
              closeTab(tab.id);
            } else {
              focusSibling(e);
            }
          }}
        >
          {title}
        </button>
      )}
      <button
        type="button"
        aria-label={t('tabs.close', { title })}
        tabIndex={-1}
        className="rounded-sm p-0.5 opacity-60 outline-none hover:bg-accent hover:opacity-100 focus-visible:opacity-100"
        onPointerDown={(e) => {
          e.stopPropagation();
        }}
        onClick={() => {
          closeTab(tab.id);
        }}
      >
        <X aria-hidden="true" className="size-3.5" />
      </button>
    </div>
  );
}

/** Zone d'onglets : accueil, puis une session par onglet (SSH-03). */
export function TabBar() {
  const { t } = useTranslation();
  const allTabs = useSessions((s) => s.tabs);
  // Les panneaux secondaires d'un onglet découpé n'ont pas d'onglet à eux.
  const tabs = allTabs.filter((tab) => tab.paneOf === null);
  const activeTabId = useSessions((s) => s.activeTabId);
  const activate = useSessions((s) => s.activate);
  const openTab = useSessions((s) => s.openTab);
  const dragging = useRef<string | null>(null);
  const homeActive = activeTabId === HOME_TAB_ID;

  function startDrag(id: string) {
    dragging.current = id;
    const stop = () => {
      dragging.current = null;
      window.removeEventListener('pointerup', stop);
    };
    window.addEventListener('pointerup', stop);
  }

  return (
    <div className="flex h-10 shrink-0 items-end gap-1 border-b bg-sidebar px-2">
      <div
        role="tablist"
        aria-label={t('tabs.label')}
        className="flex min-w-0 items-end gap-1 overflow-x-auto"
      >
        <button
          type="button"
          role="tab"
          id="tab-home"
          aria-selected={homeActive}
          aria-controls="panel-home"
          tabIndex={homeActive ? 0 : -1}
          className={cn(
            TAB_CLASS,
            homeActive
              ? 'bg-background'
              : 'border-transparent text-muted-foreground hover:bg-background/60',
          )}
          onClick={() => {
            activate(HOME_TAB_ID);
          }}
          onKeyDown={focusSibling}
        >
          <House aria-hidden="true" className="size-4" />
          <span>{t('tabs.home')}</span>
        </button>
        {tabs.map((tab) => (
          <SessionTabItem
            key={tab.id}
            tab={tab}
            active={tab.id === activeTabId}
            dragging={dragging}
            onDragStart={startDrag}
          />
        ))}
      </div>
      <button
        type="button"
        aria-label={t('tabs.new')}
        title={t('tabs.new')}
        className="mb-1 rounded-md p-1.5 text-muted-foreground outline-none hover:bg-background/60 hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
        onClick={() => {
          openTab();
        }}
      >
        <Plus aria-hidden="true" className="size-4" />
      </button>
    </div>
  );
}
