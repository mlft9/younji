import { fireEvent, render, screen } from '@testing-library/react';

import { useSessions } from '@/stores/sessions';

import { splitPane } from './layout';
import { SplitView } from './SplitView';

describe('séparateurs des panneaux (SSH-07)', () => {
  it('affiche chaque panneau et se redimensionne au clavier', () => {
    const root = useSessions.getState().openTab();
    const second = useSessions.getState().splitPane(root, 'row') ?? '';
    const layout = useSessions.getState().layouts[root];
    if (layout?.type !== 'split') throw new Error('disposition attendue');
    const { rerender } = render(
      <SplitView tabId={root} node={layout} renderPane={(id) => <p>panneau {id}</p>} />,
    );
    expect(screen.getByText(`panneau ${root}`)).toBeInTheDocument();
    expect(screen.getByText(`panneau ${second}`)).toBeInTheDocument();

    const separator = screen.getByRole('separator', { name: /Redimensionner/ });
    expect(separator).toHaveAttribute('aria-valuenow', '50');
    fireEvent.keyDown(separator, { key: 'ArrowRight' });
    const resized = useSessions.getState().layouts[root];
    expect(resized?.type === 'split' && resized.ratio).toBeCloseTo(0.55);
    if (!resized) throw new Error('disposition attendue');
    rerender(<SplitView tabId={root} node={resized} renderPane={(id) => <p>panneau {id}</p>} />);
    expect(separator).toHaveAttribute('aria-valuenow', '55');
    fireEvent.keyDown(separator, { key: 'Home' });
    const min = useSessions.getState().layouts[root];
    expect(min?.type === 'split' && min.ratio).toBe(0.1);
  });

  it('affiche une grille de quatre panneaux', () => {
    let layout = splitPane({ type: 'pane', id: 'a' }, 'a', 'c', 'column');
    layout = splitPane(layout, 'a', 'b', 'row');
    layout = splitPane(layout, 'c', 'd', 'row');
    render(<SplitView tabId="t" node={layout} renderPane={(id) => <p>panneau {id}</p>} />);
    expect(screen.getAllByText(/^panneau /)).toHaveLength(4);
    expect(screen.getAllByRole('separator')).toHaveLength(3);
  });
});
