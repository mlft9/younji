import {
  FolderTree,
  KeyRound,
  Lock,
  type LucideIcon,
  Server,
  Settings,
  SquareCode,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { formatShortcut } from '@/features/shortcuts/shortcuts';
import { lockNow } from '@/features/vault/lock';
import { cn } from '@/lib/utils';
import { HOME_TAB_ID, useSessions } from '@/stores/sessions';
import { type Section, useUi } from '@/stores/ui';

const ITEMS: readonly { section: Section; icon: LucideIcon }[] = [
  { section: 'hosts', icon: Server },
  { section: 'keys', icon: KeyRound },
  { section: 'snippets', icon: SquareCode },
  { section: 'sftp', icon: FolderTree },
];

function SidebarItem({ section, icon: Icon }: { section: Section; icon: LucideIcon }) {
  const { t } = useTranslation();
  const active = useUi((s) => s.activeSection === section);
  const activateTab = useSessions((s) => s.activate);
  const setActiveSection = useUi((s) => s.setActiveSection);

  return (
    <li>
      <button
        type="button"
        aria-current={active ? 'page' : undefined}
        onClick={() => {
          setActiveSection(section);
          // Les sections s'affichent dans l'onglet d'accueil.
          activateTab(HOME_TAB_ID);
        }}
        className={cn(
          'flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm outline-none transition-colors',
          'hover:bg-sidebar-accent hover:text-sidebar-accent-foreground',
          'focus-visible:ring-[3px] focus-visible:ring-ring/50',
          active && 'bg-sidebar-accent font-medium text-sidebar-accent-foreground',
        )}
      >
        <Icon aria-hidden="true" className="size-4 shrink-0" />
        <span>{t(`nav.${section}`)}</span>
      </button>
    </li>
  );
}

/** Barre latérale par sections, dans l'esprit de Termius (APP-01). */
export function Sidebar() {
  const { t } = useTranslation();

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground">
      <div className="px-4 py-4">
        <p className="text-base font-semibold tracking-tight">{t('app.name')}</p>
        <p className="text-xs text-muted-foreground">{t('app.tagline')}</p>
      </div>
      <nav aria-label={t('nav.label')} className="flex flex-1 flex-col justify-between px-2 pb-3">
        <ul className="flex flex-col gap-1">
          {ITEMS.map((item) => (
            <SidebarItem key={item.section} {...item} />
          ))}
        </ul>
        <ul className="flex flex-col gap-1">
          <SidebarItem section="settings" icon={Settings} />
          <li>
            <button
              type="button"
              onClick={() => void lockNow()}
              title={formatShortcut('lock', t)}
              className={cn(
                'flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm outline-none transition-colors',
                'hover:bg-sidebar-accent hover:text-sidebar-accent-foreground',
                'focus-visible:ring-[3px] focus-visible:ring-ring/50',
              )}
            >
              <Lock aria-hidden="true" className="size-4 shrink-0" />
              <span>{t('vault.lock')}</span>
            </button>
          </li>
        </ul>
      </nav>
    </aside>
  );
}
