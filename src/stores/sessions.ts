import { create } from 'zustand';

import {
  type LayoutNode,
  MAX_PANES,
  paneIds,
  removePane,
  setRatio,
  type SplitDirection,
  splitPane as splitLayout,
} from '@/features/terminal/layout';
import type { AuthPromptInfo, EntryId, HostKeyInfo, KeyFileRef, LocalProfile } from '@/lib/types';

/** Onglet d'accueil (sections de la barre latérale), toujours présent en premier. */
export const HOME_TAB_ID = 'home';

/** Longueur maximale d'un titre d'onglet personnalisé. */
export const MAX_TAB_TITLE = 64;

/**
 * État d'un onglet de session :
 * - `form` : formulaire de connexion (nouvel onglet, échec avant connexion, reconnexion) ;
 * - `connecting` / `authenticating` : connexion en cours ;
 * - `hostKey` : clé d'hôte inconnue, en attente de la décision de l'utilisateur ;
 * - `authPrompt` : le serveur pose des questions (2FA), en attente des réponses ;
 * - `blocked` : clé d'hôte modifiée, connexion refusée sans contournement ;
 * - `connected` : terminal utilisable ;
 * - `disconnected` : session terminée après avoir été établie.
 */
export type TabStatus =
  | 'form'
  | 'connecting'
  | 'hostKey'
  | 'authenticating'
  | 'authPrompt'
  | 'blocked'
  | 'connected'
  | 'disconnected';

/** Cible d'une connexion, **sans aucun secret** (le mot de passe n'est jamais stocké). */
export interface ConnectTarget {
  host: string;
  port: number;
  username: string;
  authKind: 'password' | 'key' | 'interactive';
  keyFile: KeyFileRef | null;
  /** Hôte du carnet : la connexion utilise son identité enregistrée, sans rien redemander. */
  hostId?: EntryId;
  /** Nom de l'hôte dans le carnet, utilisé comme titre d'onglet. */
  label?: string;
}

export interface HostKeyChange {
  key: HostKeyInfo;
  known: HostKeyInfo[];
  /** Forme de l'hôte dans les clés connues (`hôte` ou `[hôte]:port`). */
  hostPattern: string;
}

/** `terminal` : session SSH avec terminal ; `sftp` : explorateur double panneau (SFTP-02). */
export type TabKind = 'terminal' | 'sftp';

export interface SessionTab {
  id: string;
  kind: TabKind;
  /** Onglet de terminal dont un onglet SFTP utilise la connexion. */
  parentTabId: string | null;
  /**
   * Panneau d'un onglet découpé (SSH-07) : identifiant de l'onglet qui l'affiche. `null` pour
   * un onglet de la barre d'onglets (qui est aussi le premier panneau de sa disposition).
   */
  paneOf: string | null;
  customTitle: string | null;
  /** Dernière cible tentée, pour pré-remplir le formulaire de reconnexion. */
  target: ConnectTarget | null;
  /**
   * Terminal local (étape 7) : profil détecté par le cœur. Un onglet local n'a ni cible SSH, ni
   * panneau SFTP, et ne participe ni à la diffusion ni à l'envoi de snippets.
   */
  local: LocalProfile | null;
  status: TabStatus;
  /** Code d'erreur (`errors.<code>`) de la dernière tentative ou de la déconnexion. */
  errorCode: string | null;
  /** Clé d'hôte inconnue en attente de décision. */
  pendingHostKey: HostKeyInfo | null;
  /** Clé d'hôte modifiée (connexion bloquée). */
  hostKeyChange: HostKeyChange | null;
  /** Questions du serveur en attente de réponse (aucune réponse n'est conservée ici). */
  pendingPrompt: AuthPromptInfo | null;
  /** Barre de recherche du terminal ouverte. */
  searchOpen: boolean;
  /** Identifiant de la session du cœur (opaque, sans secret), tant qu'elle est ouverte. */
  sessionId: number | null;
  /** Dossier courant du shell annoncé par OSC 7 (donnée du serveur, non fiable). */
  remoteCwd: string | null;
  /** Panneau SFTP latéral ouvert (onglet de terminal). */
  sftpPanelOpen: boolean;
}

interface SessionsState {
  /** Onglets de la barre d'onglets **et** panneaux (`paneOf` non nul), dans l'ordre d'affichage. */
  tabs: SessionTab[];
  /** Onglet affiché (jamais un panneau secondaire). */
  activeTabId: string;
  /** Disposition des onglets découpés, par onglet. Absente : un seul panneau, l'onglet lui-même. */
  layouts: Record<string, LayoutNode>;
  /** Panneau qui a le focus, par onglet découpé. */
  focusedPane: Record<string, string>;
  /** Dernier panneau de terminal qui a eu le focus (cible par défaut des snippets). */
  lastPaneId: string | null;
  /**
   * Découpe le panneau `paneId` et ajoute un nouveau panneau (formulaire de connexion), qui
   * prend le focus. Renvoie son identifiant, ou `null` (panneau inconnu ou limite atteinte).
   */
  splitPane: (paneId: string, direction: SplitDirection) => string | null;
  /** Change la part d'une séparation. */
  resizeSplit: (tabId: string, splitId: string, ratio: number) => void;
  /** Donne le focus à un panneau (et affiche son onglet). */
  focusPane: (paneId: string) => void;
  /** Ouvre un onglet de connexion et l'active. Renvoie son identifiant. */
  openTab: () => string;
  /** Ouvre un onglet de terminal local (profil détecté) et l'active. Renvoie son identifiant. */
  openLocalTab: (profile: LocalProfile) => string;
  /**
   * Ouvre un onglet SFTP double panneau sur la connexion d'un onglet de terminal et l'active.
   * Renvoie son identifiant, ou `null` si l'onglet parent n'existe pas.
   */
  openSftpTab: (parentTabId: string) => string | null;
  /**
   * Retire un onglet ou un panneau (la session doit avoir été fermée par l'appelant). Si le
   * panneau racine d'un onglet découpé est retiré, le panneau suivant devient l'onglet.
   */
  removeTab: (id: string) => void;
  activate: (id: string) => void;
  /** Active l'onglet suivant (+1) ou précédent (-1), accueil compris, en boucle. */
  activateRelative: (delta: number) => void;
  /** Titre personnalisé ; une chaîne vide rétablit le titre par défaut. */
  renameTab: (id: string, title: string) => void;
  /** Déplace un onglet de session vers la gauche (-1) ou la droite (+1). */
  moveTab: (id: string, delta: number) => void;
  /** Place l'onglet `id` à la position de l'onglet `targetId`. */
  moveTabTo: (id: string, targetId: string) => void;
  update: (id: string, patch: Partial<Omit<SessionTab, 'id'>>) => void;
}

let counter = 0;
function newTabId(): string {
  counter += 1;
  return `session-${Date.now().toString(36)}-${counter}`;
}

function blankTerminal(id: string, paneOf: string | null): SessionTab {
  return {
    id,
    kind: 'terminal',
    parentTabId: null,
    paneOf,
    customTitle: null,
    target: null,
    local: null,
    status: 'form',
    errorCode: null,
    pendingHostKey: null,
    hostKeyChange: null,
    pendingPrompt: null,
    searchOpen: false,
    sessionId: null,
    remoteCwd: null,
    sftpPanelOpen: false,
  };
}

/** Onglet (de la barre) qui affiche un onglet ou un panneau. */
export function rootOf(tab: Pick<SessionTab, 'id' | 'paneOf'>): string {
  return tab.paneOf ?? tab.id;
}

/** Panneaux d'un onglet, dans l'ordre de lecture (l'onglet seul s'il n'est pas découpé). */
export function panesOf(state: Pick<SessionsState, 'layouts'>, tabId: string): string[] {
  const layout = state.layouts[tabId];
  return layout ? paneIds(layout) : [tabId];
}

/** Panneau actif de l'onglet affiché (l'onglet lui-même s'il n'est pas découpé). */
export function activePaneId(state: Pick<SessionsState, 'activeTabId' | 'focusedPane'>): string {
  return state.focusedPane[state.activeTabId] ?? state.activeTabId;
}

function withoutKey<T>(record: Record<string, T>, key: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => k !== key));
}

export const useSessions = create<SessionsState>()((set, get) => ({
  tabs: [],
  activeTabId: HOME_TAB_ID,
  layouts: {},
  focusedPane: {},
  lastPaneId: null,

  openTab: () => {
    const id = newTabId();
    const tab = blankTerminal(id, null);
    set((s) => ({
      // Les panneaux secondaires restent en fin de liste : l'ordre des onglets visibles ne
      // dépend que des onglets eux-mêmes.
      tabs: [...s.tabs.filter((t) => t.paneOf === null), tab, ...s.tabs.filter((t) => t.paneOf)],
      activeTabId: id,
    }));
    return id;
  },

  openLocalTab: (profile) => {
    const id = newTabId();
    const tab: SessionTab = { ...blankTerminal(id, null), local: profile, status: 'connecting' };
    set((s) => ({
      tabs: [...s.tabs.filter((t) => t.paneOf === null), tab, ...s.tabs.filter((t) => t.paneOf)],
      activeTabId: id,
    }));
    return id;
  },

  splitPane: (paneId, direction) => {
    const state = get();
    const pane = state.tabs.find((t) => t.id === paneId && t.kind === 'terminal');
    if (!pane) return null;
    const root = rootOf(pane);
    const layout: LayoutNode = state.layouts[root] ?? { type: 'pane', id: root };
    if (paneIds(layout).length >= MAX_PANES) return null;
    const id = newTabId();
    set((s) => ({
      tabs: [...s.tabs, blankTerminal(id, root)],
      layouts: { ...s.layouts, [root]: splitLayout(layout, paneId, id, direction) },
      focusedPane: { ...s.focusedPane, [root]: id },
      activeTabId: root,
      lastPaneId: id,
    }));
    return id;
  },

  resizeSplit: (tabId, splitId, ratio) => {
    set((s) => {
      const layout = s.layouts[tabId];
      if (!layout) return s;
      return { layouts: { ...s.layouts, [tabId]: setRatio(layout, splitId, ratio) } };
    });
  },

  focusPane: (paneId) => {
    const pane = get().tabs.find((t) => t.id === paneId);
    if (pane?.kind !== 'terminal') return;
    const root = rootOf(pane);
    const s = get();
    if (s.activeTabId === root && s.focusedPane[root] === paneId && s.lastPaneId === paneId) return;
    set({
      activeTabId: root,
      focusedPane: { ...s.focusedPane, [root]: paneId },
      lastPaneId: paneId,
    });
  },

  openSftpTab: (parentTabId) => {
    // Un terminal local n'a pas de connexion SFTP.
    const parent = get().tabs.find(
      (t) => t.id === parentTabId && t.kind === 'terminal' && t.local === null,
    );
    if (!parent) return null;
    const root = rootOf(parent);
    const id = newTabId();
    const tab: SessionTab = {
      id,
      kind: 'sftp',
      parentTabId,
      paneOf: null,
      customTitle: null,
      target: parent.target,
      local: null,
      status: 'connected',
      errorCode: null,
      pendingHostKey: null,
      hostKeyChange: null,
      pendingPrompt: null,
      searchOpen: false,
      sessionId: null,
      remoteCwd: parent.remoteCwd,
      sftpPanelOpen: false,
    };
    set((s) => {
      // Placé juste après l'onglet du terminal (et les onglets SFTP de ses panneaux).
      const group = new Set(panesOf(s, root));
      let index = s.tabs.findIndex((t) => t.id === root);
      while (group.has(s.tabs[index + 1]?.parentTabId ?? '')) index += 1;
      const tabs = [...s.tabs];
      tabs.splice(index + 1, 0, tab);
      return { tabs, activeTabId: id };
    });
    return id;
  },

  removeTab: (id) => {
    set((s) => {
      const removed = s.tabs.find((t) => t.id === id);
      if (!removed) return s;
      const root = rootOf(removed);
      const layout = s.layouts[root];
      const lastPaneId = s.lastPaneId === id ? null : s.lastPaneId;
      const remaining = layout ? removePane(layout, id) : null;

      if (!remaining) {
        // Onglet ordinaire (ou dernier panneau) : retiré de la barre d'onglets.
        const visible = s.tabs.filter((t) => t.paneOf === null);
        const index = visible.findIndex((t) => t.id === id);
        const tabs = s.tabs.filter((t) => t.id !== id);
        let activeTabId = s.activeTabId;
        if (activeTabId === id) {
          // Comme dans un navigateur : l'onglet voisin de droite, sinon de gauche, sinon l'accueil.
          const rest = visible.filter((t) => t.id !== id);
          activeTabId = rest[index]?.id ?? rest[index - 1]?.id ?? HOME_TAB_ID;
        }
        return {
          tabs,
          activeTabId,
          layouts: withoutKey(s.layouts, id),
          focusedPane: withoutKey(s.focusedPane, id),
          lastPaneId,
        };
      }

      const panes = paneIds(remaining);
      const single = panes.length === 1;
      if (removed.paneOf !== null) {
        // Panneau secondaire : son voisin prend la place.
        const focused = s.focusedPane[root] === id ? panes[0] : s.focusedPane[root];
        return {
          tabs: s.tabs.filter((t) => t.id !== id),
          layouts: single ? withoutKey(s.layouts, root) : { ...s.layouts, [root]: remaining },
          focusedPane:
            single || !focused
              ? withoutKey(s.focusedPane, root)
              : { ...s.focusedPane, [root]: focused },
          lastPaneId,
        };
      }

      // Panneau racine d'un onglet découpé : le premier panneau restant devient l'onglet, à la
      // même place dans la barre, avec le titre personnalisé de l'onglet.
      const heir = panes[0] ?? root;
      const tabs = s.tabs
        .filter((t) => t.id !== heir)
        .map((t): SessionTab => {
          if (t.id === id) {
            const heirTab = s.tabs.find((h) => h.id === heir);
            return heirTab
              ? {
                  ...heirTab,
                  paneOf: null,
                  customTitle: removed.customTitle ?? heirTab.customTitle,
                }
              : t;
          }
          return t.paneOf === root ? { ...t, paneOf: heir } : t;
        });
      const focused = s.focusedPane[root] === id ? heir : (s.focusedPane[root] ?? heir);
      const layouts = withoutKey(s.layouts, root);
      const focusedPane = withoutKey(s.focusedPane, root);
      return {
        tabs,
        activeTabId: s.activeTabId === root ? heir : s.activeTabId,
        layouts: single ? layouts : { ...layouts, [heir]: remaining },
        focusedPane: single ? focusedPane : { ...focusedPane, [heir]: focused },
        lastPaneId,
      };
    });
  },

  activate: (id) => {
    const tab = get().tabs.find((t) => t.id === id);
    if (id === HOME_TAB_ID || tab?.paneOf === null) {
      set({ activeTabId: id });
    } else if (tab) {
      get().focusPane(id);
    }
  },

  activateRelative: (delta) => {
    const ids = [
      HOME_TAB_ID,
      ...get()
        .tabs.filter((t) => t.paneOf === null)
        .map((t) => t.id),
    ];
    const current = Math.max(0, ids.indexOf(get().activeTabId));
    const next = (current + delta + ids.length) % ids.length;
    set({ activeTabId: ids[next] ?? HOME_TAB_ID });
  },

  renameTab: (id, title) => {
    const trimmed = title.trim().slice(0, MAX_TAB_TITLE);
    get().update(id, { customTitle: trimmed === '' ? null : trimmed });
  },

  moveTab: (id, delta) => {
    set((s) => {
      const visible = s.tabs.filter((t) => t.paneOf === null);
      const index = visible.findIndex((t) => t.id === id);
      const target = index + delta;
      if (index === -1 || target < 0 || target >= visible.length) return s;
      const [moved] = visible.splice(index, 1);
      if (moved) visible.splice(target, 0, moved);
      return { tabs: [...visible, ...s.tabs.filter((t) => t.paneOf !== null)] };
    });
  },

  moveTabTo: (id, targetId) => {
    const visible = get().tabs.filter((t) => t.paneOf === null);
    const target = visible.findIndex((t) => t.id === targetId);
    const index = visible.findIndex((t) => t.id === id);
    if (target !== -1 && index !== -1) get().moveTab(id, target - index);
  },

  update: (id, patch) => {
    set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? { ...t, ...patch } : t)) }));
  },
}));

/** Titre affiché : titre personnalisé, sinon `utilisateur@hôte`, sinon `null` (« Nouvelle session »). */
export function defaultTabTitle(tab: SessionTab): string | null {
  if (tab.customTitle) return tab.customTitle;
  if (tab.local) return tab.local.name;
  if (!tab.target) return null;
  if (tab.target.label) return tab.target.label;
  const host = tab.target.host.includes(':') ? `[${tab.target.host}]` : tab.target.host;
  return `${tab.target.username}@${host}`;
}
