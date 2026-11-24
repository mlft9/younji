import { installContextMenuGuard } from './contextMenu';

function rightClick(element: Element): boolean {
  const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
  element.dispatchEvent(event);
  return event.defaultPrevented;
}

describe('menu contextuel natif', () => {
  let remove: () => void;
  beforeEach(() => {
    document.body.innerHTML = `
      <main><p id="text">Hôtes</p><button id="button">Ajouter</button></main>
      <input id="field" /><input id="readonly" readonly /><input id="checkbox" type="checkbox" />
      <textarea id="area"></textarea>
      <div class="xterm"><textarea id="xterm-input"></textarea></div>`;
    remove = installContextMenuGuard();
  });
  afterEach(() => {
    remove();
  });

  const byId = (id: string) => {
    const element = document.getElementById(id);
    if (!element) throw new Error(id);
    return element;
  };

  it('est bloqué sur le reste de l’application (Recharger fermerait les sessions)', () => {
    expect(rightClick(byId('text'))).toBe(true);
    expect(rightClick(byId('button'))).toBe(true);
    expect(rightClick(byId('readonly'))).toBe(true);
    expect(rightClick(byId('checkbox'))).toBe(true);
    expect(rightClick(byId('xterm-input'))).toBe(true);
  });

  it('reste disponible dans les champs de saisie', () => {
    expect(rightClick(byId('field'))).toBe(false);
    expect(rightClick(byId('area'))).toBe(false);
  });
});
