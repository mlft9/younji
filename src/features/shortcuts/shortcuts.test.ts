import { matchShortcut, SHORTCUT_ACTIONS, SHORTCUTS, shortcutKeys } from './shortcuts';

const key = (k: string, mods: Partial<KeyboardEvent> = {}) => ({
  key: k,
  ctrlKey: true,
  shiftKey: false,
  altKey: false,
  metaKey: false,
  ...mods,
});

describe('raccourcis clavier', () => {
  it('reconnaît les combinaisons Ctrl+Maj, quelle que soit la casse', () => {
    expect(matchShortcut(key('T', { shiftKey: true }))).toBe('newTab');
    expect(matchShortcut(key('t', { shiftKey: true }))).toBe('newTab');
    expect(matchShortcut(key('W', { shiftKey: true }))).toBe('closeTab');
    expect(matchShortcut(key('F', { shiftKey: true }))).toBe('search');
    expect(matchShortcut(key('C', { shiftKey: true }))).toBe('copy');
    expect(matchShortcut(key('V', { shiftKey: true }))).toBe('paste');
  });

  it('gère la navigation entre onglets', () => {
    expect(matchShortcut(key('Tab'))).toBe('nextTab');
    expect(matchShortcut(key('Tab', { shiftKey: true }))).toBe('previousTab');
    expect(matchShortcut(key('PageDown'))).toBe('nextTab');
    expect(matchShortcut(key('PageUp'))).toBe('previousTab');
    expect(matchShortcut(key('PageUp', { shiftKey: true }))).toBe('moveTabLeft');
    expect(matchShortcut(key('PageDown', { shiftKey: true }))).toBe('moveTabRight');
  });

  it('laisse au shell ses raccourcis sans Maj (Ctrl+W, Ctrl+C, Ctrl+R…)', () => {
    for (const k of ['w', 't', 'c', 'v', 'r', 'f']) {
      expect(matchShortcut(key(k)), k).toBeNull();
    }
  });

  it('ignore Alt, Méta et les frappes sans Ctrl', () => {
    expect(matchShortcut(key('T', { shiftKey: true, altKey: true }))).toBeNull();
    expect(matchShortcut(key('T', { shiftKey: true, metaKey: true }))).toBeNull();
    expect(matchShortcut(key('T', { shiftKey: true, ctrlKey: false }))).toBeNull();
  });

  it('documente chaque action', () => {
    for (const action of SHORTCUT_ACTIONS) {
      expect(
        SHORTCUTS.some((s) => s.action === action),
        action,
      ).toBe(true);
    }
    expect(shortcutKeys({ action: 'newTab', key: 'T', shift: true })).toEqual([
      'ctrl',
      'shift',
      'T',
    ]);
  });

  it('gère les panneaux, les snippets et la diffusion (étape 5)', () => {
    expect(matchShortcut(key('D', { shiftKey: true }))).toBe('splitRight');
    expect(matchShortcut(key('O', { shiftKey: true }))).toBe('splitDown');
    expect(matchShortcut(key('X', { shiftKey: true }))).toBe('closePane');
    expect(matchShortcut(key('S', { shiftKey: true }))).toBe('snippets');
    expect(matchShortcut(key('I', { shiftKey: true }))).toBe('broadcast');
    expect(matchShortcut(key('ArrowLeft', { altKey: true }))).toBe('focusLeft');
    expect(matchShortcut(key('ArrowDown', { altKey: true }))).toBe('focusDown');
    // Sans Alt, les flèches restent au shell (Ctrl+flèche : mot précédent).
    expect(matchShortcut(key('ArrowLeft'))).toBeNull();
    // Alt+Maj n'est pas une combinaison de navigation.
    expect(matchShortcut(key('ArrowLeft', { altKey: true, shiftKey: true }))).toBeNull();
    expect(shortcutKeys({ action: 'focusUp', key: 'ArrowUp', shift: false, alt: true })).toEqual([
      'ctrl',
      'alt',
      'ArrowUp',
    ]);
  });
});
