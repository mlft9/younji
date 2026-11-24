import { cn } from '@/lib/utils';
import { activePaneId, type SessionTab, useSessions } from '@/stores/sessions';

import { SplitView } from './SplitView';
import { TerminalPane } from './TerminalPane';

/**
 * Contenu d'un onglet de terminal : un panneau, ou plusieurs disposés en grille (SSH-07).
 * Chaque panneau garde son terminal monté quand l'onglet est masqué.
 */
export function SessionPanel({ tab, active }: { tab: SessionTab; active: boolean }) {
  const layout = useSessions((s) => s.layouts[tab.id]);
  const focused = useSessions((s) =>
    activePaneId({ activeTabId: tab.id, focusedPane: s.focusedPane }),
  );
  const panes = useSessions((s) => s.tabs);

  function renderPane(paneId: string) {
    const pane = panes.find((p) => p.id === paneId);
    if (!pane) return null;
    return (
      <TerminalPane tab={pane} active={active} focused={paneId === focused} split={!!layout} />
    );
  }

  return (
    <div
      role="tabpanel"
      id={`panel-${tab.id}`}
      aria-labelledby={`tab-${tab.id}`}
      hidden={!active}
      className={cn('flex min-h-0 flex-1', !active && 'hidden')}
    >
      {layout ? (
        <SplitView tabId={tab.id} node={layout} renderPane={renderPane} />
      ) : (
        renderPane(tab.id)
      )}
    </div>
  );
}
