import { MAX_PANES } from '@/features/terminal/layout';

import {
  activePaneId,
  defaultTabTitle,
  HOME_TAB_ID,
  MAX_TAB_TITLE,
  panesOf,
  useSessions,
} from './sessions';

const store = () => useSessions.getState();
const ids = () => store().tabs.map((t) => t.id);
const firstTab = () => {
  const tab = store().tabs[0];
  if (!tab) throw new Error('aucun onglet');
  return tab;
};

describe('store des onglets de session', () => {
  beforeEach(() => {
    useSessions.setState({ tabs: [], activeTabId: HOME_TAB_ID });
  });

  it('ouvre un onglet de connexion et l’active', () => {
    const id = store().openTab();
    expect(store().activeTabId).toBe(id);
    expect(store().tabs[0]).toMatchObject({ id, status: 'form', target: null });
  });

  it('active le voisin à la fermeture de l’onglet actif', () => {
    const a = store().openTab();
    const b = store().openTab();
    const c = store().openTab();
    store().activate(b);
    store().removeTab(b);
    expect(store().activeTabId).toBe(c);
    store().removeTab(c);
    expect(store().activeTabId).toBe(a);
    store().removeTab(a);
    expect(store().activeTabId).toBe(HOME_TAB_ID);
  });

  it('parcourt les onglets en boucle, accueil compris', () => {
    const a = store().openTab();
    const b = store().openTab();
    store().activateRelative(1);
    expect(store().activeTabId).toBe(HOME_TAB_ID);
    store().activateRelative(1);
    expect(store().activeTabId).toBe(a);
    store().activateRelative(-1);
    store().activateRelative(-1);
    expect(store().activeTabId).toBe(b);
  });

  it('renomme, rétablit le titre par défaut et borne la longueur', () => {
    const id = store().openTab();
    store().update(id, {
      target: { host: 'srv', port: 22, username: 'root', authKind: 'password', keyFile: null },
    });
    expect(defaultTabTitle(firstTab())).toBe('root@srv');
    store().renameTab(id, '  Prod  ');
    expect(defaultTabTitle(firstTab())).toBe('Prod');
    store().renameTab(id, '   ');
    expect(defaultTabTitle(firstTab())).toBe('root@srv');
    store().renameTab(id, 'x'.repeat(200));
    expect(firstTab().customTitle).toHaveLength(MAX_TAB_TITLE);
  });

  it('affiche une adresse IPv6 entre crochets', () => {
    const id = store().openTab();
    store().update(id, {
      target: { host: '::1', port: 22, username: 'u', authKind: 'password', keyFile: null },
    });
    expect(defaultTabTitle(firstTab())).toBe('u@[::1]');
  });

  it('réordonne les onglets sans sortir des bornes', () => {
    const a = store().openTab();
    const b = store().openTab();
    const c = store().openTab();
    store().moveTab(a, 1);
    expect(ids()).toEqual([b, a, c]);
    store().moveTab(b, -1);
    expect(ids()).toEqual([b, a, c]);
    store().moveTabTo(c, b);
    expect(ids()).toEqual([c, b, a]);
  });

  it("n'active pas un onglet inconnu", () => {
    store().activate('inconnu');
    expect(store().activeTabId).toBe(HOME_TAB_ID);
  });
});

describe('onglets découpés (SSH-07)', () => {
  const reset = () => {
    useSessions.setState({
      tabs: [],
      activeTabId: HOME_TAB_ID,
      layouts: {},
      focusedPane: {},
      lastPaneId: null,
    });
  };
  beforeEach(reset);

  it('ajoute un panneau sans onglet supplémentaire, avec le focus', () => {
    const root = store().openTab();
    const pane = store().splitPane(root, 'row');
    expect(pane).not.toBeNull();
    expect(
      store()
        .tabs.filter((t) => t.paneOf === null)
        .map((t) => t.id),
    ).toEqual([root]);
    expect(store().tabs.find((t) => t.id === pane)?.paneOf).toBe(root);
    expect(panesOf(store(), root)).toEqual([root, pane]);
    expect(activePaneId(store())).toBe(pane);
    expect(store().lastPaneId).toBe(pane);
  });

  it('limite le nombre de panneaux par onglet', () => {
    const root = store().openTab();
    for (let i = 1; i < MAX_PANES; i += 1) expect(store().splitPane(root, 'row')).not.toBeNull();
    expect(store().splitPane(root, 'row')).toBeNull();
    expect(panesOf(store(), root)).toHaveLength(MAX_PANES);
  });

  it('retire un panneau secondaire et revient à un onglet simple', () => {
    const root = store().openTab();
    const pane = store().splitPane(root, 'column') ?? '';
    store().removeTab(pane);
    expect(store().layouts).toEqual({});
    expect(store().focusedPane).toEqual({});
    expect(ids()).toEqual([root]);
    expect(store().activeTabId).toBe(root);
  });

  it('promeut le panneau suivant quand le panneau racine est retiré', () => {
    const before = store().openTab();
    const root = store().openTab();
    const after = store().openTab();
    store().renameTab(root, 'Prod');
    const b = store().splitPane(root, 'row') ?? '';
    const c = store().splitPane(b, 'column') ?? '';
    store().activate(root);
    store().removeTab(root);
    const visible = store().tabs.filter((t) => t.paneOf === null);
    expect(visible.map((t) => t.id)).toEqual([before, b, after]);
    expect(store().activeTabId).toBe(b);
    expect(store().tabs.find((t) => t.id === b)?.customTitle).toBe('Prod');
    expect(store().tabs.find((t) => t.id === c)?.paneOf).toBe(b);
    expect(panesOf(store(), b)).toEqual([b, c]);
  });

  it('ignore les panneaux secondaires pour la navigation et le déplacement des onglets', () => {
    const a = store().openTab();
    store().splitPane(a, 'row');
    const b = store().openTab();
    store().activate(a);
    store().activateRelative(1);
    expect(store().activeTabId).toBe(b);
    store().moveTab(b, -1);
    expect(
      store()
        .tabs.filter((t) => t.paneOf === null)
        .map((t) => t.id),
    ).toEqual([b, a]);
  });

  it('active l’onglet d’un panneau secondaire qui prend le focus', () => {
    const root = store().openTab();
    const pane = store().splitPane(root, 'row') ?? '';
    store().focusPane(root);
    store().activate(HOME_TAB_ID);
    store().activate(pane);
    expect(store().activeTabId).toBe(root);
    expect(activePaneId(store())).toBe(pane);
  });

  it('place l’onglet SFTP d’un panneau après l’onglet qui le contient', () => {
    const root = store().openTab();
    const pane = store().splitPane(root, 'row') ?? '';
    const other = store().openTab();
    const sftp = store().openSftpTab(pane);
    const visible = store()
      .tabs.filter((t) => t.paneOf === null)
      .map((t) => t.id);
    expect(visible).toEqual([root, sftp, other]);
  });
});
