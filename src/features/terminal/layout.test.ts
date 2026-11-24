import {
  clampRatio,
  type LayoutNode,
  neighborPane,
  paneIds,
  paneRects,
  removePane,
  setRatio,
  splitPane,
} from './layout';

/** Grille 2×2 : a | b au-dessus, c | d en dessous. */
function grid(): LayoutNode {
  let layout: LayoutNode = { type: 'pane', id: 'a' };
  layout = splitPane(layout, 'a', 'c', 'column');
  layout = splitPane(layout, 'a', 'b', 'row');
  layout = splitPane(layout, 'c', 'd', 'row');
  return layout;
}

describe('disposition des panneaux (SSH-07)', () => {
  it('découpe en grille et liste les panneaux dans l’ordre de lecture', () => {
    expect(paneIds(grid())).toEqual(['a', 'b', 'c', 'd']);
    const rects = paneRects(grid());
    expect(rects.get('d')).toEqual({ x: 0.5, y: 0.5, width: 0.5, height: 0.5 });
  });

  it('navigue vers le voisin dans chaque direction', () => {
    const layout = grid();
    expect(neighborPane(layout, 'a', 'right')).toBe('b');
    expect(neighborPane(layout, 'a', 'down')).toBe('c');
    expect(neighborPane(layout, 'd', 'left')).toBe('c');
    expect(neighborPane(layout, 'd', 'up')).toBe('b');
    expect(neighborPane(layout, 'a', 'left')).toBeNull();
    expect(neighborPane(layout, 'a', 'up')).toBeNull();
    expect(neighborPane(layout, 'inconnu', 'up')).toBeNull();
  });

  it('préfère le voisin le plus aligné quand plusieurs sont adjacents', () => {
    // a à gauche sur toute la hauteur, b et c empilés à droite.
    let layout: LayoutNode = { type: 'pane', id: 'a' };
    layout = splitPane(layout, 'a', 'b', 'row');
    layout = splitPane(layout, 'b', 'c', 'column');
    expect(neighborPane(layout, 'a', 'right')).toBe('b');
    expect(neighborPane(layout, 'c', 'left')).toBe('a');
  });

  it('retire un panneau : le voisin prend sa place', () => {
    const layout = removePane(grid(), 'b');
    expect(layout && paneIds(layout)).toEqual(['a', 'c', 'd']);
    expect(removePane({ type: 'pane', id: 'a' }, 'a')).toBeNull();
    expect(removePane({ type: 'pane', id: 'a' }, 'z')).toEqual({ type: 'pane', id: 'a' });
  });

  it('borne la part de chaque séparation', () => {
    const layout = grid();
    if (layout.type !== 'split') throw new Error('séparation attendue');
    const resized = setRatio(layout, layout.id, 0.99);
    expect(resized.type === 'split' && resized.ratio).toBe(0.9);
    expect(clampRatio(-3)).toBe(0.1);
    expect(clampRatio(Number.NaN)).toBe(0.5);
  });
});
