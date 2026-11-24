// Disposition des panneaux d'un onglet de terminal (SSH-07).
//
// Un onglet contient un arbre binaire : chaque feuille est un panneau (une session), chaque
// nœud une séparation en deux, côte à côte (`row`) ou l'une au-dessus de l'autre (`column`),
// avec la part (`ratio`) laissée au premier enfant. Fonctions pures, testées sans React.

/** Nombre maximal de panneaux par onglet (chaque panneau ouvre sa propre connexion). */
export const MAX_PANES = 8;
/** Part minimale d'un côté d'une séparation. */
export const MIN_RATIO = 0.1;

export type SplitDirection = 'row' | 'column';

export type LayoutNode =
  | { type: 'pane'; id: string }
  | {
      type: 'split';
      id: string;
      direction: SplitDirection;
      ratio: number;
      first: LayoutNode;
      second: LayoutNode;
    };

export type NavigationDirection = 'left' | 'right' | 'up' | 'down';

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

let splitCounter = 0;
function newSplitId(): string {
  splitCounter += 1;
  return `split-${splitCounter.toString(36)}`;
}

export function clampRatio(ratio: number): number {
  if (!Number.isFinite(ratio)) return 0.5;
  return Math.min(1 - MIN_RATIO, Math.max(MIN_RATIO, ratio));
}

/** Panneaux, dans l'ordre de lecture (de gauche à droite, de haut en bas). */
export function paneIds(node: LayoutNode): string[] {
  return node.type === 'pane' ? [node.id] : [...paneIds(node.first), ...paneIds(node.second)];
}

/** Coupe le panneau `paneId` en deux ; le nouveau panneau va à droite (`row`) ou en bas. */
export function splitPane(
  node: LayoutNode,
  paneId: string,
  newPaneId: string,
  direction: SplitDirection,
): LayoutNode {
  if (node.type === 'pane') {
    if (node.id !== paneId) return node;
    return {
      type: 'split',
      id: newSplitId(),
      direction,
      ratio: 0.5,
      first: node,
      second: { type: 'pane', id: newPaneId },
    };
  }
  return {
    ...node,
    first: splitPane(node.first, paneId, newPaneId, direction),
    second: splitPane(node.second, paneId, newPaneId, direction),
  };
}

/** Retire un panneau : son voisin prend toute la place. `null` si c'était le dernier. */
export function removePane(node: LayoutNode, paneId: string): LayoutNode | null {
  if (node.type === 'pane') return node.id === paneId ? null : node;
  const first = removePane(node.first, paneId);
  const second = removePane(node.second, paneId);
  if (!first) return second;
  if (!second) return first;
  return { ...node, first, second };
}

/** Change la part d'une séparation (bornée). */
export function setRatio(node: LayoutNode, splitId: string, ratio: number): LayoutNode {
  if (node.type === 'pane') return node;
  if (node.id === splitId) return { ...node, ratio: clampRatio(ratio) };
  return {
    ...node,
    first: setRatio(node.first, splitId, ratio),
    second: setRatio(node.second, splitId, ratio),
  };
}

/** Position relative (0–1) de chaque panneau. */
export function paneRects(
  node: LayoutNode,
  rect: Rect = { x: 0, y: 0, width: 1, height: 1 },
): Map<string, Rect> {
  if (node.type === 'pane') return new Map([[node.id, rect]]);
  const firstRect =
    node.direction === 'row'
      ? { ...rect, width: rect.width * node.ratio }
      : { ...rect, height: rect.height * node.ratio };
  const secondRect =
    node.direction === 'row'
      ? { ...rect, x: rect.x + firstRect.width, width: rect.width - firstRect.width }
      : { ...rect, y: rect.y + firstRect.height, height: rect.height - firstRect.height };
  return new Map([...paneRects(node.first, firstRect), ...paneRects(node.second, secondRect)]);
}

const EPSILON = 1e-9;

/**
 * Panneau voisin dans une direction (navigation au clavier) : parmi les panneaux situés de ce
 * côté et qui se chevauchent sur l'autre axe, le plus proche, puis le plus aligné. `null` s'il
 * n'y en a pas.
 */
export function neighborPane(
  node: LayoutNode,
  paneId: string,
  direction: NavigationDirection,
): string | null {
  const rects = paneRects(node);
  const from = rects.get(paneId);
  if (!from) return null;
  const horizontal = direction === 'left' || direction === 'right';
  let best: { id: string; distance: number; offset: number } | null = null;
  for (const [id, rect] of rects) {
    if (id === paneId) continue;
    let distance: number;
    if (direction === 'right') distance = rect.x - (from.x + from.width);
    else if (direction === 'left') distance = from.x - (rect.x + rect.width);
    else if (direction === 'down') distance = rect.y - (from.y + from.height);
    else distance = from.y - (rect.y + rect.height);
    if (distance < -EPSILON) continue;
    const overlap = horizontal
      ? Math.min(from.y + from.height, rect.y + rect.height) - Math.max(from.y, rect.y)
      : Math.min(from.x + from.width, rect.x + rect.width) - Math.max(from.x, rect.x);
    if (overlap <= EPSILON) continue;
    const offset = horizontal ? Math.abs(rect.y - from.y) : Math.abs(rect.x - from.x);
    if (
      !best ||
      distance < best.distance - EPSILON ||
      (Math.abs(distance - best.distance) <= EPSILON && offset < best.offset)
    ) {
      best = { id, distance, offset };
    }
  }
  return best?.id ?? null;
}
