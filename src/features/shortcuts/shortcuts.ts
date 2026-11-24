// Raccourcis clavier de l'application (APP-04).
//
// Ils utilisent tous Ctrl+Maj (ou Ctrl+Tab) pour ne pas voler au shell distant ses propres
// raccourcis : Ctrl+W, Ctrl+T, Ctrl+C, Ctrl+R ou Ctrl+F ont un sens dans bash, vim ou tmux.
// La correspondance se fait sur `key` (la lettre produite, selon la disposition du clavier),
// pour que Ctrl+Maj+W corresponde bien à la touche W en AZERTY comme en QWERTY.

export const SHORTCUT_ACTIONS = [
  'newTab',
  'closeTab',
  'nextTab',
  'previousTab',
  'moveTabLeft',
  'moveTabRight',
  'search',
  'copy',
  'paste',
  'sftpPanel',
  'splitRight',
  'splitDown',
  'closePane',
  'focusLeft',
  'focusRight',
  'focusUp',
  'focusDown',
  'snippets',
  'broadcast',
  'lock',
] as const;
export type ShortcutAction = (typeof SHORTCUT_ACTIONS)[number];

export type KeyboardEventLike = Pick<
  KeyboardEvent,
  'key' | 'ctrlKey' | 'shiftKey' | 'altKey' | 'metaKey'
>;

interface ShortcutDefinition {
  action: ShortcutAction;
  /** Touche, comparée sans tenir compte de la casse. */
  key: string;
  shift: boolean;
  /** Ctrl+Alt (navigation entre panneaux). Absent : Alt interdit. */
  alt?: boolean;
}

/** Définitions, dans l'ordre d'affichage. Une action peut avoir plusieurs combinaisons. */
export const SHORTCUTS: readonly ShortcutDefinition[] = [
  { action: 'newTab', key: 'T', shift: true },
  { action: 'closeTab', key: 'W', shift: true },
  { action: 'nextTab', key: 'Tab', shift: false },
  { action: 'nextTab', key: 'PageDown', shift: false },
  { action: 'previousTab', key: 'Tab', shift: true },
  { action: 'previousTab', key: 'PageUp', shift: false },
  { action: 'moveTabLeft', key: 'PageUp', shift: true },
  { action: 'moveTabRight', key: 'PageDown', shift: true },
  { action: 'search', key: 'F', shift: true },
  { action: 'copy', key: 'C', shift: true },
  { action: 'paste', key: 'V', shift: true },
  { action: 'sftpPanel', key: 'E', shift: true },
  { action: 'splitRight', key: 'D', shift: true },
  { action: 'splitDown', key: 'O', shift: true },
  { action: 'closePane', key: 'X', shift: true },
  { action: 'focusLeft', key: 'ArrowLeft', shift: false, alt: true },
  { action: 'focusRight', key: 'ArrowRight', shift: false, alt: true },
  { action: 'focusUp', key: 'ArrowUp', shift: false, alt: true },
  { action: 'focusDown', key: 'ArrowDown', shift: false, alt: true },
  { action: 'snippets', key: 'S', shift: true },
  { action: 'broadcast', key: 'I', shift: true },
  { action: 'lock', key: 'L', shift: true },
];

/**
 * Action correspondant à une frappe, ou `null`. Toutes exigent Ctrl, sans Méta ; Alt seulement
 * pour la navigation entre panneaux.
 */
export function matchShortcut(event: KeyboardEventLike): ShortcutAction | null {
  if (!event.ctrlKey || event.metaKey) return null;
  const key = event.key.toLowerCase();
  const found = SHORTCUTS.find(
    (shortcut) =>
      shortcut.key.toLowerCase() === key &&
      shortcut.shift === event.shiftKey &&
      (shortcut.alt ?? false) === event.altKey,
  );
  return found?.action ?? null;
}

/** Touches à afficher pour une définition (les libellés des modificateurs sont traduits). */
export function shortcutKeys(definition: ShortcutDefinition): string[] {
  return [
    'ctrl',
    ...(definition.alt ? ['alt'] : []),
    ...(definition.shift ? ['shift'] : []),
    definition.key,
  ];
}

/** Libellé de la première combinaison d'une action, avec des modificateurs traduits. */
export function formatShortcut(
  action: ShortcutAction,
  translate: (key: 'shortcuts.keys.ctrl' | 'shortcuts.keys.shift' | 'shortcuts.keys.alt') => string,
): string {
  const definition = SHORTCUTS.find((s) => s.action === action);
  if (!definition) return '';
  return shortcutKeys(definition)
    .map((part) =>
      part === 'ctrl' || part === 'shift' || part === 'alt'
        ? translate(`shortcuts.keys.${part}`)
        : part,
    )
    .join('+');
}
