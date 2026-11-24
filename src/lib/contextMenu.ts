// Menu contextuel natif de la WebView (« Précédent », « Suivant », « Arrêt », « Recharger »).
//
// Il n'a aucun sens dans une application de bureau, ses entrées sont souvent grisées, et
// « Recharger » fermerait toutes les sessions et verrouillerait le coffre. Il est donc bloqué
// partout, sauf dans les champs de saisie (couper, copier, coller restent utiles). Le terminal a
// son propre menu (TerminalContextMenu).

/** Vrai si la cible est un champ de saisie modifiable, hors du terminal xterm.js. */
function isEditableField(target: EventTarget | null): boolean {
  if (!(target instanceof Element) || target.closest('.xterm')) return false;
  if (target instanceof HTMLTextAreaElement) return !target.readOnly && !target.disabled;
  if (target instanceof HTMLInputElement) {
    const textual = ['text', 'search', 'url', 'email', 'tel', 'number', 'password', ''];
    return textual.includes(target.type) && !target.readOnly && !target.disabled;
  }
  return target.closest('[contenteditable="true"]') !== null;
}

/** Bloque le menu contextuel natif hors des champs de saisie. Renvoie de quoi le retirer. */
export function installContextMenuGuard(target: Window = window): () => void {
  const block = (event: MouseEvent) => {
    if (!isEditableField(event.target)) event.preventDefault();
  };
  target.addEventListener('contextmenu', block);
  return () => {
    target.removeEventListener('contextmenu', block);
  };
}
