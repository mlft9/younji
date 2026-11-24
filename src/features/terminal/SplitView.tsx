import { type KeyboardEvent, type PointerEvent, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { cn } from '@/lib/utils';
import { useSessions } from '@/stores/sessions';

import type { LayoutNode } from './layout';

/** Pas d'un déplacement de séparateur au clavier. */
const KEYBOARD_STEP = 0.05;

interface SplitViewProps {
  tabId: string;
  node: LayoutNode;
  renderPane: (paneId: string) => React.ReactNode;
}

/**
 * Affiche récursivement la disposition d'un onglet (SSH-07). Les séparateurs se déplacent à la
 * souris (glisser) ou au clavier (flèches, une fois le séparateur sélectionné avec Tab).
 */
export function SplitView({ tabId, node, renderPane }: SplitViewProps) {
  if (node.type === 'pane') return <>{renderPane(node.id)}</>;
  const row = node.direction === 'row';
  const percent = `${(node.ratio * 100).toFixed(3)}%`;
  return (
    <div className={cn('flex h-full min-h-0 w-full min-w-0', row ? 'flex-row' : 'flex-col')}>
      <div
        className="min-h-0 min-w-0 overflow-hidden"
        // Valeur dynamique (part choisie par l'utilisateur) : style en ligne autorisé.
        style={{ flex: `0 0 calc(${percent} - 2px)` }}
      >
        <SplitView tabId={tabId} node={node.first} renderPane={renderPane} />
      </div>
      <Splitter tabId={tabId} splitId={node.id} row={row} ratio={node.ratio} />
      <div className="min-h-0 min-w-0 flex-1 overflow-hidden">
        <SplitView tabId={tabId} node={node.second} renderPane={renderPane} />
      </div>
    </div>
  );
}

function Splitter({
  tabId,
  splitId,
  row,
  ratio,
}: {
  tabId: string;
  splitId: string;
  row: boolean;
  ratio: number;
}) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const resize = useSessions((s) => s.resizeSplit);

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    const parent = ref.current?.parentElement;
    if (event.button !== 0 || !parent) return;
    event.preventDefault();
    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);
    const bounds = parent.getBoundingClientRect();
    const move = (e: globalThis.PointerEvent) => {
      const size = row ? bounds.width : bounds.height;
      if (size <= 0) return;
      const offset = row ? e.clientX - bounds.left : e.clientY - bounds.top;
      resize(tabId, splitId, offset / size);
    };
    const up = () => {
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', up);
      target.removeEventListener('pointercancel', up);
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', up);
    target.addEventListener('pointercancel', up);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const decrease = row ? 'ArrowLeft' : 'ArrowUp';
    const increase = row ? 'ArrowRight' : 'ArrowDown';
    if (event.key === decrease) resize(tabId, splitId, ratio - KEYBOARD_STEP);
    else if (event.key === increase) resize(tabId, splitId, ratio + KEYBOARD_STEP);
    else if (event.key === 'Home') resize(tabId, splitId, 0);
    else if (event.key === 'End') resize(tabId, splitId, 1);
    else return;
    event.preventDefault();
    event.stopPropagation();
  }

  return (
    <div
      ref={ref}
      role="separator"
      tabIndex={0}
      aria-orientation={row ? 'vertical' : 'horizontal'}
      aria-label={t('panes.resize')}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(ratio * 100)}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      className={cn(
        'shrink-0 bg-border outline-none hover:bg-primary/60 focus-visible:bg-primary',
        row ? 'w-1 cursor-col-resize' : 'h-1 cursor-row-resize',
      )}
    />
  );
}
