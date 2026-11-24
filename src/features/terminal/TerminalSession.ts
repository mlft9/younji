// Contrôleur d'un onglet de terminal : relie une instance xterm.js à une session SSH du cœur.
//
// Il vit en dehors de React (un par onglet, dans `registry`) pour que le terminal, son tampon
// et sa session survivent aux re-rendus et aux montages successifs du composant. Le composant
// `TerminalView` se contente d'y attacher son conteneur.

import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { WebglAddon } from '@xterm/addon-webgl';
import { Terminal } from '@xterm/xterm';

import type { ResolvedTheme } from '@/features/settings/theme';
import {
  openExternalUrl,
  sshAck,
  sshClose,
  sshConnect,
  sshConnectHost,
  sshAuthAnswer,
  sshHostKeyDecision,
  sshResize,
  sshWrite,
  toAppError,
} from '@/lib/ipc';
import type { AuthRequest, SessionEvent, SessionId } from '@/lib/types';
import { useBroadcast } from '@/stores/broadcast';
import { type ConnectTarget, HOME_TAB_ID, useSessions } from '@/stores/sessions';
import { useSnippetRun } from '@/stores/snippetRun';
import { useSettings } from '@/stores/settings';

import { parseOsc7, SHELL_INTEGRATION } from '@/features/sftp/osc7';

import { InputQueue } from './inputQueue';
import { isOpenableUrl, isOpenGesture } from './links';
import { analyzePaste, type PasteAnalysis } from './paste';
import { TERMINAL_FONT_FAMILY, terminalTheme } from './terminalTheme';

/** Caractères refusés dans un texte envoyé par `sendText` (contrôles hors `\t` et `\n`). */
// eslint-disable-next-line no-control-regex -- recherche volontaire des caractères de contrôle
const UNSAFE_TEXT = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/;
/** Lignes conservées dans l'historique de défilement. */
const SCROLLBACK = 10_000;
/** Anti-rebond du redimensionnement envoyé au serveur. */
const RESIZE_DEBOUNCE_MS = 100;
/** Taille utilisée tant que le terminal n'a jamais été affiché. */
const DEFAULT_SIZE = { cols: 80, rows: 24 };

/** Secrets saisis dans le formulaire, transmis une seule fois puis oubliés. */
export interface ConnectSecrets {
  password?: string;
  passphrase?: string;
}

/** Fenêtres de confirmation affichées par le composant. */
export interface TerminalUiHandlers {
  confirmPaste: (analysis: PasteAnalysis) => void;
  confirmLink: (url: string) => void;
}

export class TerminalSession {
  readonly terminal: Terminal;
  private readonly fit = new FitAddon();
  private readonly search = new SearchAddon();
  private readonly host: HTMLDivElement;
  private opened = false;
  private disposed = false;
  private resizeObserver: ResizeObserver | null = null;
  private resizeTimer: ReturnType<typeof setTimeout> | null = null;
  private ui: TerminalUiHandlers | null = null;

  private sessionId: SessionId | null = null;
  /** Incrémenté à chaque connexion : les messages d'une ancienne session sont ignorés. */
  private generation = 0;
  private everConnected = false;
  private input: InputQueue | null = null;
  /** Octets affichés avant que l'identifiant de session ne soit connu. */
  private pendingAck = 0;
  private readonly unsubscribeSettings: () => void;

  constructor(readonly tabId: string) {
    this.terminal = new Terminal({
      allowProposedApi: true,
      scrollback: SCROLLBACK,
      cursorBlink: true,
      fontFamily: TERMINAL_FONT_FAMILY,
      fontSize: useSettings.getState().terminalFontSize,
      // Liens OSC 8 : le texte affiché peut différer de la cible, d'où une confirmation.
      linkHandler: {
        activate: (event, uri) => {
          if (isOpenGesture(event) && isOpenableUrl(uri)) this.ui?.confirmLink(uri);
        },
        allowNonHttpProtocols: false,
      },
      // Aucune commande « fenêtre » (redimensionnement, lecture du titre…) pour le serveur.
      windowOptions: {},
    });
    this.host = document.createElement('div');
    this.host.className = 'h-full w-full';
    // Taille de police réglable dans les paramètres (APP-05), appliquée à chaud.
    this.unsubscribeSettings = useSettings.subscribe((state, previous) => {
      if (state.terminalFontSize !== previous.terminalFontSize) {
        this.terminal.options.fontSize = state.terminalFontSize;
        this.fitToContainer();
      }
    });

    this.terminal.loadAddon(this.fit);
    this.terminal.loadAddon(this.search);
    this.terminal.loadAddon(new Unicode11Addon());
    this.terminal.unicode.activeVersion = '11';
    // Liens détectés dans le texte : ouverts directement au Ctrl+clic (adresse visible).
    this.terminal.loadAddon(
      new WebLinksAddon((event, uri) => {
        if (isOpenGesture(event) && isOpenableUrl(uri)) {
          void openExternalUrl(uri).catch(() => undefined);
        }
      }),
    );
    // Pas d'addon presse-papiers : les séquences OSC 52 restent sans effet (SECURITE.md §4.2).

    // OSC 7 : dossier courant annoncé par le shell, suivi par le panneau SFTP (SFTP-01). Le
    // chemin est une donnée du serveur : décodé strictement, il ne sert qu'à le lister.
    this.terminal.parser.registerOscHandler(7, (data) => {
      const path = parseOsc7(data);
      if (path !== null) this.update({ remoteCwd: path });
      return true;
    });

    this.terminal.onData((data) => {
      this.handleInput(data, false);
    });
    this.terminal.onBinary((data) => {
      this.handleInput(data, true);
    });
    this.terminal.onResize(({ cols, rows }) => {
      this.scheduleRemoteResize(cols, rows);
    });

    // Collage (Ctrl+Maj+V, Maj+Inser, menu du navigateur) : intercepté avant xterm pour passer
    // par la garde de collage.
    this.host.addEventListener(
      'paste',
      (event) => {
        event.preventDefault();
        event.stopPropagation();
        this.paste(event.clipboardData?.getData('text/plain') ?? '');
      },
      { capture: true },
    );
  }

  // ------------------------------------------------------------------ affichage

  /** Attache le terminal à un conteneur visible (peut être rappelé après un remontage). */
  attach(container: HTMLElement, ui: TerminalUiHandlers): void {
    if (this.disposed) return;
    this.ui = ui;
    container.appendChild(this.host);
    if (!this.opened) {
      this.terminal.open(this.host);
      this.opened = true;
      this.loadWebgl();
    }
    this.resizeObserver?.disconnect();
    this.resizeObserver = new ResizeObserver(() => {
      this.fitToContainer();
    });
    this.resizeObserver.observe(this.host);
    this.fitToContainer();
  }

  detach(): void {
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    this.ui = null;
    this.host.remove();
  }

  setTheme(theme: ResolvedTheme): void {
    this.terminal.options.theme = terminalTheme(theme);
  }

  focus(): void {
    this.terminal.focus();
  }

  private loadWebgl(): void {
    try {
      const webgl = new WebglAddon();
      // Perte du contexte (pilote, veille…) : retour au rendu DOM, sans planter.
      webgl.onContextLoss(() => {
        webgl.dispose();
      });
      this.terminal.loadAddon(webgl);
    } catch {
      // WebGL indisponible (VM sans accélération, WebKitGTK) : rendu DOM par défaut.
    }
  }

  private fitToContainer(): void {
    // Un onglet masqué a une taille nulle : on attend qu'il soit de nouveau visible.
    if (this.host.clientWidth === 0 || this.host.clientHeight === 0) return;
    try {
      this.fit.fit();
    } catch {
      // Mesure impossible pendant une transition : le prochain redimensionnement corrigera.
    }
  }

  private scheduleRemoteResize(cols: number, rows: number): void {
    if (this.resizeTimer) clearTimeout(this.resizeTimer);
    this.resizeTimer = setTimeout(() => {
      this.resizeTimer = null;
      if (this.sessionId !== null) {
        void sshResize(this.sessionId, cols, rows).catch(() => undefined);
      }
    }, RESIZE_DEBOUNCE_MS);
  }

  // ------------------------------------------------------------------ saisie

  /**
   * Saisie tapée (ou collée) dans ce terminal. En diffusion (SSH-08), si ce terminal est une
   * cible, elle est recopiée dans la file d'entrée de **chaque** cible connectée, qui garde son
   * propre contrôle de flux. Un terminal qui n'est pas une cible n'envoie qu'à sa session.
   */
  private handleInput(data: string, binary: boolean): void {
    const { targets } = useBroadcast.getState();
    if (targets.length >= 2 && targets.includes(this.tabId)) {
      for (const id of targets) registry.get(id)?.pushInput(data, binary);
    } else {
      this.pushInput(data, binary);
    }
  }

  /** Envoie des octets à la session, sans diffusion. Renvoie `false` sans session ouverte. */
  private pushInput(data: string, binary: boolean): boolean {
    if (this.sessionId === null || !this.input) return false;
    this.input.push(data, binary);
    return true;
  }

  /** Vrai si la session est ouverte (terminal utilisable). */
  isConnected(): boolean {
    return this.sessionId !== null && this.input !== null;
  }

  /**
   * Envoie un texte déjà validé (snippet, SNP-03) à cette seule session, sans passer par la
   * diffusion. Le texte est encadré comme un collage si le programme distant l'a demandé ;
   * `execute` ajoute Entrée. Renvoie `false` si la session n'est pas ouverte.
   */
  sendText(text: string, execute: boolean): boolean {
    // Défense en profondeur : aucun caractère de contrôle hors saut de ligne et tabulation (un
    // ESC ferait sortir du collage encadré).
    if (UNSAFE_TEXT.test(text)) return false;
    const body = text.replace(/\r?\n/g, '\r');
    const framed = this.terminal.modes.bracketedPasteMode ? `\x1b[200~${body}\x1b[201~` : body;
    return this.pushInput(execute ? `${framed}\r` : framed, false);
  }

  // ------------------------------------------------------------------ session

  /** Ouvre une session SSH dans ce terminal (première connexion ou reconnexion). */
  async connect(target: ConnectTarget, secrets: ConnectSecrets): Promise<void> {
    let auth: AuthRequest;
    if (target.authKind === 'password') {
      auth = { kind: 'password', password: secrets.password ?? '' };
    } else if (target.authKind === 'key') {
      auth = {
        kind: 'key',
        keyFileId: target.keyFile?.id ?? -1,
        passphrase: secrets.passphrase ?? null,
      };
    } else {
      auth = { kind: 'interactive' };
    }
    await this.open(target, (size, listeners) =>
      sshConnect(
        { host: target.host, port: target.port, username: target.username, auth, ...size },
        listeners,
      ),
    );
  }

  /**
   * Ouvre une session vers un hôte du carnet : le cœur lit l'identité dans le coffre, aucun
   * secret ne passe par l'interface (reconnexion comprise).
   */
  async connectHost(target: ConnectTarget & { hostId: string }): Promise<void> {
    await this.open(target, (size, listeners) =>
      sshConnectHost({ hostId: target.hostId, ...size }, listeners),
    );
  }

  private async open(
    target: ConnectTarget,
    start: (
      size: { cols: number; rows: number },
      listeners: Parameters<typeof sshConnect>[1],
    ) => Promise<SessionId>,
  ): Promise<void> {
    if (this.disposed) return;
    this.closeRemote();
    const generation = ++this.generation;
    this.everConnected = false;
    this.pendingAck = 0;
    this.update({
      target,
      status: 'connecting',
      errorCode: null,
      pendingHostKey: null,
      hostKeyChange: null,
      pendingPrompt: null,
    });
    const size = this.opened
      ? { cols: this.terminal.cols, rows: this.terminal.rows }
      : DEFAULT_SIZE;

    let sessionId: SessionId;
    try {
      sessionId = await start(size, {
        onEvent: (event) => {
          if (generation === this.generation) this.handleEvent(event);
        },
        onOutput: (data) => {
          if (generation === this.generation) this.handleOutput(data);
        },
      });
    } catch (error) {
      if (generation === this.generation) {
        this.update({ status: 'form', errorCode: toAppError(error).code });
      }
      return;
    }
    if (generation !== this.generation || this.isDisposed()) {
      // Onglet fermé ou nouvelle tentative pendant l'ouverture : on abandonne celle-ci.
      void sshClose(sessionId).catch(() => undefined);
      return;
    }
    this.sessionId = sessionId;
    this.update({ sessionId });
    this.input = new InputQueue(
      (data, binary) => sshWrite(sessionId, data, binary),
      () => undefined,
    );
    if (this.pendingAck > 0) {
      void sshAck(sessionId, this.pendingAck).catch(() => undefined);
      this.pendingAck = 0;
    }
  }

  /** Décision sur une clé d'hôte inconnue. */
  async decideHostKey(accept: boolean): Promise<void> {
    if (this.sessionId === null) return;
    this.update({ pendingHostKey: null, status: accept ? 'authenticating' : 'connecting' });
    try {
      await sshHostKeyDecision(this.sessionId, accept);
    } catch {
      // La session s'est terminée entre-temps : l'événement de fin met l'onglet à jour.
    }
  }

  /**
   * Réponses aux questions du serveur (`null` : annulation). Les réponses ne sont pas
   * conservées : l'appelant vide ses champs dès l'appel. Si le cœur les refuse (longueur,
   * nombre), la question est réaffichée avec la cause : elle est toujours en attente.
   */
  async answerPrompt(answers: string[] | null): Promise<void> {
    const prompt = useSessions.getState().tabs.find((t) => t.id === this.tabId)?.pendingPrompt;
    if (this.sessionId === null || !prompt) return;
    this.update({ pendingPrompt: null, status: 'authenticating', errorCode: null });
    try {
      await sshAuthAnswer(this.sessionId, prompt.round, answers);
    } catch (error) {
      const code = toAppError(error).code;
      if (code === 'ssh.invalid_answers') {
        this.update({ pendingPrompt: prompt, status: 'authPrompt', errorCode: code });
      }
      // Sinon, la session s'est terminée entre-temps : l'événement de fin met l'onglet à jour.
    }
  }

  private handleEvent(event: SessionEvent): void {
    switch (event.type) {
      case 'hostKeyUnknown':
        this.update({ status: 'hostKey', pendingHostKey: event.key });
        break;
      case 'hostKeyChanged':
        this.update({
          status: 'blocked',
          hostKeyChange: {
            key: event.key,
            known: event.known,
            hostPattern: event.hostPattern,
          },
        });
        break;
      case 'authenticating':
        this.update({ status: 'authenticating', pendingHostKey: null, pendingPrompt: null });
        break;
      case 'authPrompt':
        this.update({
          status: 'authPrompt',
          errorCode: null,
          pendingPrompt: {
            round: event.round,
            name: event.name,
            instructions: event.instructions,
            prompts: event.prompts,
          },
        });
        break;
      case 'keyDeployed':
        // Réservé aux déploiements de clé : sans objet pour une session de terminal.
        break;
      case 'connected':
        this.everConnected = true;
        this.update({ status: 'connected', errorCode: null });
        this.focus();
        break;
      case 'disconnected':
        this.handleDisconnected(event.code);
        break;
    }
  }

  private handleDisconnected(code: string): void {
    // Une session terminée quitte la diffusion pour de bon : une reconnexion ne la réintègre pas.
    useBroadcast.getState().removeTarget(this.tabId);
    this.sessionId = null;
    this.update({ sessionId: null });
    this.input?.close();
    this.input = null;
    const current = useSessions.getState().tabs.find((t) => t.id === this.tabId);
    if (current?.status === 'blocked') return; // L'alerte de clé modifiée reste affichée.
    if (code === 'ssh.session_closed') return; // Fermeture demandée par l'interface.
    if (this.everConnected) {
      this.update({
        status: 'disconnected',
        errorCode: code,
        pendingHostKey: null,
        pendingPrompt: null,
      });
    } else {
      // Échec avant l'ouverture du shell : retour au formulaire avec la cause.
      this.update({ status: 'form', errorCode: code, pendingHostKey: null, pendingPrompt: null });
    }
  }

  private handleOutput(data: Uint8Array): void {
    const length = data.length;
    // Acquittement une fois les octets traités par xterm : contrôle de flux de bout en bout.
    this.terminal.write(data, () => {
      this.acknowledge(length);
    });
  }

  private acknowledge(bytes: number): void {
    if (this.sessionId === null) {
      this.pendingAck += bytes;
      return;
    }
    void sshAck(this.sessionId, bytes).catch(() => undefined);
  }

  private closeRemote(): void {
    useBroadcast.getState().removeTarget(this.tabId);
    this.input?.close();
    this.input = null;
    if (this.sessionId !== null) {
      void sshClose(this.sessionId).catch(() => undefined);
      this.sessionId = null;
      this.update({ sessionId: null, remoteCwd: null });
    }
  }

  /**
   * Insère l'intégration au prompt (OSC 7) pour que le panneau SFTP suive les `cd`, comme un
   * collage (encadré si le programme distant l'a demandé), **sans la valider** : l'utilisateur
   * la relit et appuie sur Entrée. Uniquement sur demande explicite ; le texte est constant.
   */
  sendShellIntegration(): void {
    if (this.sessionId === null) return;
    this.terminal.paste(SHELL_INTEGRATION);
    this.focus();
  }

  private update(patch: Parameters<ReturnType<typeof useSessions.getState>['update']>[1]): void {
    if (!this.disposed) useSessions.getState().update(this.tabId, patch);
  }

  /** Méthode plutôt que champ lu directement : l'état change pendant les `await`. */
  private isDisposed(): boolean {
    return this.disposed;
  }

  // ------------------------------------------------------------------ presse-papiers, recherche

  /** Colle du texte en passant par la garde de collage. */
  paste(raw: string): void {
    if (raw === '' || this.sessionId === null) return;
    const analysis = analyzePaste(raw);
    if (analysis.needsConfirmation && this.ui) {
      this.ui.confirmPaste(analysis);
    } else if (!analysis.needsConfirmation) {
      this.terminal.paste(analysis.text);
    }
  }

  /** Colle après confirmation (texte déjà nettoyé par `analyzePaste`). */
  pasteConfirmed(analysis: PasteAnalysis): void {
    if (this.sessionId !== null) this.terminal.paste(analysis.text);
    this.focus();
  }

  /** Copie la sélection dans le presse-papiers. Renvoie `false` s'il n'y a rien à copier. */
  async copySelection(): Promise<boolean> {
    const selection = this.terminal.getSelection();
    if (selection === '') return false;
    await navigator.clipboard.writeText(selection);
    return true;
  }

  findNext(term: string): boolean {
    return term !== '' && this.search.findNext(term);
  }

  findPrevious(term: string): boolean {
    return term !== '' && this.search.findPrevious(term);
  }

  clearSearch(): void {
    this.search.clearDecorations();
    this.terminal.clearSelection();
  }

  /** Ferme la session et libère le terminal (fermeture de l'onglet). */
  dispose(): void {
    if (this.disposed) return;
    this.closeRemote();
    this.disposed = true;
    this.generation++;
    if (this.resizeTimer) clearTimeout(this.resizeTimer);
    this.unsubscribeSettings();
    this.detach();
    this.terminal.dispose();
  }
}

// ------------------------------------------------------------------ registre

const registry = new Map<string, TerminalSession>();

/** Contrôleur de l'onglet, créé au premier accès. */
export function terminalSession(tabId: string): TerminalSession {
  let session = registry.get(tabId);
  if (!session) {
    session = new TerminalSession(tabId);
    registry.set(tabId, session);
  }
  return session;
}

/** Contrôleur existant, sans en créer. */
export function existingTerminalSession(tabId: string): TerminalSession | undefined {
  return registry.get(tabId);
}

/**
 * Ferme la session d'un panneau (ou d'un onglet non découpé), libère son terminal et le retire.
 * Les onglets SFTP qui utilisent sa connexion sont fermés avec lui.
 */
export function closePane(paneId: string): void {
  for (const child of useSessions.getState().tabs.filter((t) => t.parentTabId === paneId)) {
    closePane(child.id);
  }
  registry.get(paneId)?.dispose();
  registry.delete(paneId);
  useSessions.getState().removeTab(paneId);
}

/**
 * Ferme un onglet entier : tous ses panneaux (SSH-07) et les onglets SFTP qui utilisent leurs
 * connexions. Pour un panneau secondaire, ne ferme que ce panneau.
 */
export function closeTab(tabId: string): void {
  for (const pane of useSessions.getState().tabs.filter((t) => t.paneOf === tabId)) {
    closePane(pane.id);
  }
  closePane(tabId);
}

/**
 * Ferme tous les onglets de session et détruit leurs terminaux, tampon de défilement compris
 * (verrouillage du coffre, ADR-026). La diffusion s'arrête.
 */
export function closeAllTabs(): void {
  // Diffusion arrêtée et dialogues d'envoi fermés : rien ne survit au verrouillage.
  useBroadcast.setState({ targets: [], dialogOpen: false });
  useSnippetRun.getState().close();
  for (const tab of useSessions.getState().tabs) closePane(tab.id);
  for (const [tabId, session] of registry) {
    session.dispose();
    registry.delete(tabId);
  }
  useSessions.getState().activate(HOME_TAB_ID);
}
