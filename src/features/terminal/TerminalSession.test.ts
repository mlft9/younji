import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import type { SessionEvent } from '@/lib/types';
import { useBroadcast } from '@/stores/broadcast';
import { type ConnectTarget, HOME_TAB_ID, useSessions } from '@/stores/sessions';
import { useSnippetRun } from '@/stores/snippetRun';
import { useSettings } from '@/stores/settings';

import { SHELL_INTEGRATION } from '@/features/sftp/osc7';

import { analyzePaste } from './paste';

import {
  closeAllTabs,
  closePane,
  closeTab,
  existingTerminalSession,
  terminalSession,
  TerminalSession,
} from './TerminalSession';

/** Accès au double de xterm, dont les méthodes sont des fonctions simulées. */
function fake(session: TerminalSession) {
  return session.terminal as unknown as {
    paste: ReturnType<typeof vi.fn>;
    dispose: ReturnType<typeof vi.fn>;
    onData: ReturnType<typeof vi.fn>;
    attachCustomKeyEventHandler: ReturnType<typeof vi.fn>;
    modes: { bracketedPasteMode: boolean };
    written: Uint8Array[];
    osc: Map<number, (data: string) => boolean>;
  };
}

// xterm.js a besoin d'un vrai moteur de rendu : on le remplace par un double minimal.
vi.mock('@xterm/xterm', () => {
  class FakeTerminal {
    cols = 80;
    rows = 24;
    options: Record<string, unknown> = {};
    unicode = { activeVersion: '' };
    modes = { bracketedPasteMode: false };
    written: Uint8Array[] = [];
    paste = vi.fn();
    focus = vi.fn();
    dispose = vi.fn();
    clearSelection = vi.fn();
    loadAddon = vi.fn();
    open = vi.fn();
    getSelection = vi.fn(() => '');
    hasSelection = vi.fn(() => false);
    onData = vi.fn();
    onBinary = vi.fn();
    attachCustomKeyEventHandler = vi.fn();
    onResize = vi.fn();
    /** Gestionnaires OSC enregistrés, par identifiant (OSC 7 : dossier courant). */
    osc = new Map<number, (data: string) => boolean>();
    parser = {
      registerOscHandler: (id: number, handler: (data: string) => boolean) => {
        this.osc.set(id, handler);
        return { dispose: vi.fn() };
      },
    };
    write(data: Uint8Array, callback?: () => void) {
      this.written.push(data);
      callback?.();
    }
  }
  return { Terminal: FakeTerminal };
});
vi.mock('@xterm/addon-fit', () => ({
  FitAddon: class {
    dispose = vi.fn();
  },
}));
vi.mock('@xterm/addon-search', () => ({
  SearchAddon: class {
    dispose = vi.fn();
  },
}));
vi.mock('@xterm/addon-unicode11', () => ({
  Unicode11Addon: class {
    dispose = vi.fn();
  },
}));
vi.mock('@xterm/addon-web-links', () => ({
  WebLinksAddon: class {
    dispose = vi.fn();
  },
}));
vi.mock('@xterm/addon-webgl', () => ({
  WebglAddon: class {
    dispose = vi.fn();
  },
}));

vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof IpcModule>();
  return {
    ...actual,
    sshConnect: vi.fn(),
    sshConnectHost: vi.fn(),
    sshWrite: vi.fn().mockResolvedValue(undefined),
    sshResize: vi.fn().mockResolvedValue(undefined),
    sshAck: vi.fn().mockResolvedValue(undefined),
    sshClose: vi.fn().mockResolvedValue(undefined),
    sshHostKeyDecision: vi.fn().mockResolvedValue(undefined),
    sshAuthAnswer: vi.fn().mockResolvedValue(undefined),
    openExternalUrl: vi.fn().mockResolvedValue(undefined),
  };
});

const TARGET: ConnectTarget = {
  host: 'srv.lab',
  port: 22,
  username: 'root',
  authKind: 'password',
  keyFile: null,
};
const KEY = { algorithm: 'ssh-ed25519', fingerprint: 'SHA256:abc' };

interface Listeners {
  onEvent: (event: SessionEvent) => void;
  onOutput: (data: Uint8Array) => void;
}

/** Ouvre un onglet et une session ; renvoie de quoi simuler le cœur. */
async function connected(sessionId = 7) {
  const tabId = useSessions.getState().openTab();
  const session = new TerminalSession(tabId);
  let listeners: Listeners | undefined;
  vi.mocked(ipc.sshConnect).mockImplementationOnce((_request, l) => {
    listeners = l;
    return Promise.resolve(sessionId);
  });
  await session.connect(TARGET, { password: 'secret' });
  const emit = (event: SessionEvent) => {
    listeners?.onEvent(event);
  };
  const output = (data: Uint8Array) => {
    listeners?.onOutput(data);
  };
  const tab = () => useSessions.getState().tabs.find((t) => t.id === tabId);
  return { session, tabId, emit, output, tab };
}

// jsdom n'implémente pas ResizeObserver.
vi.stubGlobal(
  'ResizeObserver',
  class {
    observe = vi.fn();
    disconnect = vi.fn();
  },
);

describe('TerminalSession', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSessions.setState({ tabs: [], activeTabId: HOME_TAB_ID });
  });

  it('envoie le secret une seule fois, dans la demande de connexion, sans le stocker', async () => {
    const { tab } = await connected();
    expect(ipc.sshConnect).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ipc.sshConnect).mock.calls[0]?.[0]).toEqual({
      host: 'srv.lab',
      port: 22,
      username: 'root',
      auth: { kind: 'password', password: 'secret' },
      cols: 80,
      rows: 24,
    });
    expect(JSON.stringify(useSessions.getState())).not.toContain('secret');
    expect(tab()).toMatchObject({ status: 'connecting', target: TARGET });
  });

  it('suit la clé d’hôte inconnue, la décision et la connexion', async () => {
    const { session, emit, tab } = await connected();
    emit({ type: 'hostKeyUnknown', key: KEY });
    expect(tab()).toMatchObject({ status: 'hostKey', pendingHostKey: KEY });

    await session.decideHostKey(true);
    expect(ipc.sshHostKeyDecision).toHaveBeenCalledWith(7, true);
    emit({ type: 'authenticating' });
    emit({ type: 'connected' });
    expect(tab()).toMatchObject({ status: 'connected', pendingHostKey: null, errorCode: null });
  });

  it('suit les questions du serveur (2FA) sans conserver les réponses', async () => {
    const { session, emit, tab } = await connected();
    emit({ type: 'authenticating' });
    const prompt = {
      round: 1,
      name: '',
      instructions: '',
      prompts: [{ text: 'Verification code: ', echo: false }],
    };
    emit({ type: 'authPrompt', ...prompt });
    expect(tab()).toMatchObject({ status: 'authPrompt', pendingPrompt: prompt });

    await session.answerPrompt(['123456']);
    expect(ipc.sshAuthAnswer).toHaveBeenCalledWith(7, 1, ['123456']);
    expect(tab()).toMatchObject({ status: 'authenticating', pendingPrompt: null });
    expect(JSON.stringify(useSessions.getState())).not.toContain('123456');
    emit({ type: 'authenticating' });
    emit({ type: 'connected' });
    expect(tab()?.status).toBe('connected');
  });

  it('transmet l’annulation des questions et revient au formulaire', async () => {
    const { session, emit, tab } = await connected();
    emit({
      type: 'authPrompt',
      round: 4,
      name: '',
      instructions: '',
      prompts: [{ text: 'Code', echo: true }],
    });
    await session.answerPrompt(null);
    expect(ipc.sshAuthAnswer).toHaveBeenCalledWith(7, 4, null);
    emit({ type: 'disconnected', code: 'ssh.auth_cancelled' });
    expect(tab()).toMatchObject({
      status: 'form',
      errorCode: 'ssh.auth_cancelled',
      pendingPrompt: null,
    });
  });

  it('réaffiche la question si le cœur refuse les réponses', async () => {
    const { session, emit, tab } = await connected();
    const prompt = {
      round: 2,
      name: '',
      instructions: '',
      prompts: [{ text: 'Code', echo: false }],
    };
    emit({ type: 'authPrompt', ...prompt });
    vi.mocked(ipc.sshAuthAnswer).mockRejectedValueOnce({ code: 'ssh.invalid_answers' });
    await session.answerPrompt(['x'.repeat(2000)]);
    expect(tab()).toMatchObject({
      status: 'authPrompt',
      pendingPrompt: prompt,
      errorCode: 'ssh.invalid_answers',
    });
  });

  it('se connecte en mode interactif sans aucun secret', async () => {
    const tabId = useSessions.getState().openTab();
    const session = new TerminalSession(tabId);
    vi.mocked(ipc.sshConnect).mockResolvedValueOnce(3);
    await session.connect({ ...TARGET, authKind: 'interactive' }, {});
    expect(vi.mocked(ipc.sshConnect).mock.calls[0]?.[0]).toMatchObject({
      auth: { kind: 'interactive' },
    });
  });

  it('revient au formulaire avec la cause si la connexion échoue avant le shell', async () => {
    const { emit, tab } = await connected();
    emit({ type: 'authenticating' });
    emit({ type: 'disconnected', code: 'ssh.auth_failed' });
    expect(tab()).toMatchObject({ status: 'form', errorCode: 'ssh.auth_failed' });
  });

  it('passe en « déconnecté » après une session établie', async () => {
    const { emit, tab } = await connected();
    emit({ type: 'connected' });
    emit({ type: 'disconnected', code: 'ssh.connection_lost' });
    expect(tab()).toMatchObject({ status: 'disconnected', errorCode: 'ssh.connection_lost' });
  });

  it('garde l’alerte de clé modifiée affichée après la fin de session', async () => {
    const { emit, tab } = await connected();
    emit({
      type: 'hostKeyChanged',
      key: KEY,
      known: [KEY],
      hostPattern: 'srv.lab',
    });
    emit({ type: 'disconnected', code: 'ssh.host_key_changed' });
    expect(tab()).toMatchObject({
      status: 'blocked',
      hostKeyChange: { key: KEY, known: [KEY], hostPattern: 'srv.lab' },
    });
  });

  it('acquitte la sortie une fois affichée (contrôle de flux)', async () => {
    const { session, output } = await connected();
    output(new Uint8Array([104, 105]));
    expect(fake(session).written).toHaveLength(1);
    expect(ipc.sshAck).toHaveBeenCalledWith(7, 2);
  });

  it('affiche une erreur de validation renvoyée par le cœur', async () => {
    const tabId = useSessions.getState().openTab();
    const session = new TerminalSession(tabId);
    vi.mocked(ipc.sshConnect).mockRejectedValueOnce({ code: 'ssh.key_wrong_passphrase' });
    await session.connect(TARGET, { password: 'x' });
    expect(useSessions.getState().tabs[0]).toMatchObject({
      status: 'form',
      errorCode: 'ssh.key_wrong_passphrase',
    });
  });

  it('ignore les messages de l’ancienne session après une reconnexion', async () => {
    const first = await connected(1);
    first.emit({ type: 'connected' });
    vi.mocked(ipc.sshConnect).mockResolvedValueOnce(2);
    await first.session.connect(TARGET, { password: 'secret' });
    expect(ipc.sshClose).toHaveBeenCalledWith(1);
    first.emit({ type: 'disconnected', code: 'ssh.connection_lost' });
    expect(first.tab()?.status).toBe('connecting');
  });

  it('fait confirmer un collage multi-lignes et colle directement une ligne simple', async () => {
    const { session } = await connected();
    const confirmPaste = vi.fn();
    session.attach(document.createElement('div'), { confirmPaste, confirmLink: vi.fn() });

    session.paste('ls -la');
    expect(fake(session).paste).toHaveBeenCalledWith('ls -la');

    session.paste('rm -rf /tmp/x\nreboot\n');
    expect(confirmPaste).toHaveBeenCalledWith(
      expect.objectContaining({ lineCount: 3, needsConfirmation: true }),
    );
    expect(fake(session).paste).toHaveBeenCalledTimes(1);
  });

  it('ferme la session distante à la fermeture de l’onglet', async () => {
    const { session } = await connected(9);
    session.dispose();
    expect(ipc.sshClose).toHaveBeenCalledWith(9);
    expect(fake(session).dispose).toHaveBeenCalled();
  });

  it('se connecte à un hôte du carnet par son identifiant, sans aucun secret', async () => {
    const tabId = useSessions.getState().openTab();
    const session = new TerminalSession(tabId);
    vi.mocked(ipc.sshConnectHost).mockResolvedValueOnce(9);
    await session.connectHost({ ...TARGET, hostId: 'h-1', label: 'Web' });
    expect(ipc.sshConnectHost).toHaveBeenCalledWith(
      { hostId: 'h-1', cols: 80, rows: 24 },
      expect.anything(),
    );
    expect(JSON.stringify(vi.mocked(ipc.sshConnectHost).mock.calls[0]?.[0])).not.toContain(
      'password',
    );
    const tab = useSessions.getState().tabs.find((t) => t.id === tabId);
    expect(tab?.target).toMatchObject({ hostId: 'h-1', label: 'Web' });
    session.dispose();
  });

  it('applique à chaud la taille de police des paramètres', () => {
    const session = new TerminalSession('police');
    useSettings.setState({ terminalFontSize: 18 });
    expect(session.terminal.options.fontSize).toBe(18);
    session.dispose();
    // Après destruction, le terminal ne suit plus les paramètres.
    useSettings.setState({ terminalFontSize: 20 });
    expect(session.terminal.options.fontSize).toBe(18);
    useSettings.setState({ terminalFontSize: 14 });
  });

  it('ferme tous les onglets et détruit leurs terminaux au verrouillage', async () => {
    const first = useSessions.getState().openTab();
    const second = useSessions.getState().openTab();
    const a = terminalSession(first);
    const b = terminalSession(second);
    closeAllTabs();
    expect(useSessions.getState().tabs).toHaveLength(0);
    expect(useSessions.getState().activeTabId).toBe(HOME_TAB_ID);
    expect(fake(a).dispose).toHaveBeenCalled();
    expect(fake(b).dispose).toHaveBeenCalled();
    // Le registre est vidé : un nouvel accès crée un nouveau terminal.
    expect(terminalSession(first)).not.toBe(a);
    closeAllTabs();
    await Promise.resolve();
  });

  it('suit le dossier courant annoncé par OSC 7 et ignore une séquence invalide', async () => {
    const { session, emit, tab } = await connected();
    emit({ type: 'connected' });
    const osc7 = fake(session).osc.get(7);
    expect(osc7).toBeDefined();
    osc7?.('file://srv/home/al%20ice/%C3%A9t%C3%A9');
    expect(tab()?.remoteCwd).toBe('/home/al ice/été');
    osc7?.('file://srv/tmp/%0Apiege');
    osc7?.('http://srv/autre');
    expect(tab()?.remoteCwd).toBe('/home/al ice/été');
    expect(tab()?.sessionId).toBe(7);
  });

  it('insère l’intégration au prompt sur demande, comme un collage, sans la valider', async () => {
    const { session, emit } = await connected();
    emit({ type: 'connected' });
    expect(fake(session).paste).not.toHaveBeenCalled();
    session.sendShellIntegration();
    // Vers cette seule session (jamais diffusé), comme un collage, sans Entrée ajoutée.
    await vi.waitFor(() => {
      expect(ipc.sshWrite).toHaveBeenCalledWith(
        7,
        SHELL_INTEGRATION.replace(/\r?\n/g, '\r'),
        false,
      );
    });
    expect(SHELL_INTEGRATION.endsWith('\n')).toBe(false);
  });

  it('ferme les onglets SFTP avec leur terminal', async () => {
    const { tabId } = await connected();
    const child = useSessions.getState().openSftpTab(tabId);
    expect(child).not.toBeNull();
    closeTab(tabId);
    expect(useSessions.getState().tabs).toEqual([]);
  });
});

/** Ouvre une session connectée **enregistrée** dans le registre (diffusion, snippets). */
async function registered(sessionId: number, tabId = useSessions.getState().openTab()) {
  const session = terminalSession(tabId);
  let listeners: Listeners | undefined;
  vi.mocked(ipc.sshConnect).mockImplementationOnce((_request, l) => {
    listeners = l;
    return Promise.resolve(sessionId);
  });
  await session.connect(TARGET, { password: 'x' });
  listeners?.onEvent({ type: 'connected' });
  const onData = (data: string) => {
    const handler = fake(session).onData.mock.calls[0]?.[0] as (d: string) => void;
    handler(data);
  };
  /** Simule une frappe de l'utilisateur dans ce terminal (touche puis données). */
  const type = (data: string) => {
    const keyHandler = fake(session).attachCustomKeyEventHandler.mock.calls[0]?.[0] as (
      e: unknown,
    ) => boolean;
    keyHandler({});
    onData(data);
  };
  /** Simule une réponse automatique d'xterm (sans frappe). */
  const report = onData;
  const disconnect = () => {
    listeners?.onEvent({ type: 'disconnected', code: 'ssh.connection_lost' });
  };
  return { session, tabId, type, report, disconnect };
}

/** Données envoyées à une session du cœur. */
function writesTo(sessionId: number): string[] {
  return vi
    .mocked(ipc.sshWrite)
    .mock.calls.filter((call) => call[0] === sessionId)
    .map((call) => call[1]);
}

describe('diffusion de la saisie (SSH-08)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    closeAllTabs();
    useSessions.setState({ tabs: [], activeTabId: HOME_TAB_ID, layouts: {}, focusedPane: {} });
  });

  it('recopie la frappe d’une cible vers toutes les cibles, pas vers les autres sessions', async () => {
    const a = await registered(1);
    const b = await registered(2);
    const c = await registered(3);
    const outside = await registered(4);
    useBroadcast.getState().start([a.tabId, b.tabId, c.tabId]);

    a.type('ls\r');
    await vi.waitFor(() => {
      expect(writesTo(3)).toEqual(['ls\r']);
    });
    expect(writesTo(1)).toEqual(['ls\r']);
    expect(writesTo(2)).toEqual(['ls\r']);
    expect(writesTo(4)).toEqual([]);

    // Une session hors diffusion n'envoie qu'à elle-même.
    outside.type('pwd\r');
    await vi.waitFor(() => {
      expect(writesTo(4)).toEqual(['pwd\r']);
    });
    expect(writesTo(1)).toEqual(['ls\r']);
  });

  it('retire une cible déconnectée et s’arrête sous deux cibles', async () => {
    const a = await registered(1);
    const b = await registered(2);
    const c = await registered(3);
    useBroadcast.getState().start([a.tabId, b.tabId, c.tabId]);
    c.disconnect();
    expect(useBroadcast.getState().targets).toEqual([a.tabId, b.tabId]);
    b.disconnect();
    expect(useBroadcast.getState().targets).toEqual([]);
    a.type('x');
    await vi.waitFor(() => {
      expect(writesTo(1)).toEqual(['x']);
    });
  });

  it('s’arrête quand une cible est fermée ou au verrouillage', async () => {
    const a = await registered(1);
    const b = await registered(2);
    useBroadcast.getState().start([a.tabId, b.tabId]);
    closeTab(b.tabId);
    expect(useBroadcast.getState().targets).toEqual([]);

    const c = await registered(3);
    useBroadcast.getState().start([a.tabId, c.tabId]);
    useBroadcast.getState().openDialog();
    useSnippetRun.getState().openPicker([a.tabId]);
    closeAllTabs();
    expect(useBroadcast.getState()).toMatchObject({ targets: [], dialogOpen: false });
    expect(useSnippetRun.getState().pickerTargets).toBeNull();
  });

  it('ne diffuse pas les réponses automatiques du terminal', async () => {
    const a = await registered(1);
    const b = await registered(2);
    useBroadcast.getState().start([a.tabId, b.tabId]);
    // Réponse à une requête du serveur, sans frappe : vers sa seule session.
    a.report('\x1b[12;40R');
    // Même juste après une frappe, une réponse reconnue n'est pas diffusée.
    a.type('\x1b[?1;2c');
    a.type('\x1b]11;rgb:0000/0000/0000\x1b\\');
    a.type('\x1b[?12;40R');
    a.type('\x1b[0n');
    a.type('\x1b[?2004;1$y');
    a.type('\x1b[32;10;5M');
    await vi.waitFor(() => {
      expect(writesTo(1).join('')).toBe(
        '\x1b[12;40R\x1b[?1;2c\x1b]11;rgb:0000/0000/0000\x1b\\' +
          '\x1b[?12;40R\x1b[0n\x1b[?2004;1$y\x1b[32;10;5M',
      );
    });
    expect(writesTo(2)).toEqual([]);
    // Les frappes ordinaires (flèches, touches de fonction) restent diffusées.
    a.type('\x1b[A\x1b[15~');
    await vi.waitFor(() => {
      expect(writesTo(2)).toEqual(['\x1b[A\x1b[15~']);
    });
  });

  it('confirme tout collage diffusé et l’encadre selon le mode de chaque cible', async () => {
    const a = await registered(1);
    const b = await registered(2);
    const confirmPaste = vi.fn();
    a.session.attach(document.createElement('div'), { confirmPaste, confirmLink: vi.fn() });
    fake(a.session).modes.bracketedPasteMode = true;
    useBroadcast.getState().start([a.tabId, b.tabId]);

    // Une seule ligne, sans contrôle : confirmation quand même, rien d'envoyé avant.
    a.session.paste('uptime');
    expect(confirmPaste).toHaveBeenCalledTimes(1);
    expect(fake(a.session).paste).not.toHaveBeenCalled();

    a.session.pasteConfirmed(analyzePaste('ls\nwhoami\x1b[201~'));
    await vi.waitFor(() => {
      expect(writesTo(2)).toEqual(['ls\rwhoami']);
    });
    expect(writesTo(1)).toEqual(['\x1b[200~ls\rwhoami\x1b[201~']);
    expect(fake(a.session).paste).not.toHaveBeenCalled();
  });

  it('refuse d’activer la diffusion avec moins de deux cibles', () => {
    useBroadcast.getState().start(['seul', 'seul']);
    expect(useBroadcast.getState().targets).toEqual([]);
  });
});

describe('envoi de texte (snippets, SNP-03)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    closeAllTabs();
  });

  it('envoie le texte à cette seule session, même en diffusion', async () => {
    const a = await registered(1);
    const b = await registered(2);
    useBroadcast.getState().start([a.tabId, b.tabId]);
    expect(a.session.sendText('ls\nwhoami', true)).toBe(true);
    await vi.waitFor(() => {
      expect(writesTo(1)).toEqual(['ls\rwhoami\r']);
    });
    expect(writesTo(2)).toEqual([]);
  });

  it('encadre le texte si le shell a demandé le collage encadré, et n’ajoute Entrée qu’à la demande', async () => {
    const a = await registered(1);
    fake(a.session).modes.bracketedPasteMode = true;
    a.session.sendText('ls', false);
    await vi.waitFor(() => {
      expect(writesTo(1)).toEqual(['\x1b[200~ls\x1b[201~']);
    });
  });

  it('refuse un texte avec des caractères de contrôle ou sans session', async () => {
    const a = await registered(1);
    expect(a.session.sendText('ls\x1b[201~; rm -rf /', true)).toBe(false);
    expect(a.session.sendText('a\rb', false)).toBe(false);
    a.disconnect();
    expect(a.session.sendText('ls', true)).toBe(false);
    expect(writesTo(1)).toEqual([]);
  });
});

describe('fermeture des panneaux (SSH-07)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    closeAllTabs();
    useSessions.setState({ tabs: [], activeTabId: HOME_TAB_ID, layouts: {}, focusedPane: {} });
  });

  it('ferme tous les panneaux d’un onglet, ou un seul panneau', async () => {
    const root = await registered(1);
    const second = useSessions.getState().splitPane(root.tabId, 'row') ?? '';
    await registered(2, second);
    const third = useSessions.getState().splitPane(second, 'column') ?? '';
    await registered(3, third);

    closePane(third);
    expect(ipc.sshClose).toHaveBeenCalledWith(3);
    expect(existingTerminalSession(third)).toBeUndefined();
    expect(useSessions.getState().tabs.map((t) => t.id)).toEqual([root.tabId, second]);

    closeTab(root.tabId);
    expect(ipc.sshClose).toHaveBeenCalledWith(1);
    expect(ipc.sshClose).toHaveBeenCalledWith(2);
    expect(useSessions.getState().tabs).toEqual([]);
    expect(useSessions.getState().layouts).toEqual({});
  });
});

describe('saisie des lettres accentuées (WebKitGTK)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    closeAllTabs();
  });

  /** Touche enfoncée puis événement `input` qui remonte jusqu'au conteneur du terminal. */
  function keyThenInput(session: TerminalSession, data: string, init: InputEventInit = {}) {
    const keyHandler = fake(session).attachCustomKeyEventHandler.mock.calls[0]?.[0] as (e: {
      type: string;
    }) => boolean;
    keyHandler({ type: 'keydown' });
    const { host } = session as unknown as { host: HTMLElement };
    host.dispatchEvent(
      new InputEvent('input', { data, inputType: 'insertText', bubbles: true, ...init }),
    );
  }

  it('envoie « é » quand xterm ne l’a pas traité', async () => {
    const a = await registered(1);
    keyThenInput(a.session, 'é');
    keyThenInput(a.session, 'è');
    await vi.waitFor(() => {
      expect(writesTo(1).join('')).toBe('éè');
    });
  });

  it('n’envoie rien de plus quand xterm envoie la touche en différé (code 229)', async () => {
    const a = await registered(1);
    const keyHandler = fake(a.session).attachCustomKeyEventHandler.mock.calls[0]?.[0] as (e: {
      type: string;
    }) => boolean;
    const onData = fake(a.session).onData.mock.calls[0]?.[0] as (d: string) => void;
    keyHandler({ type: 'keydown' });
    // Comme CompositionHelper : envoi programmé au keydown, avant l'événement input.
    setTimeout(() => {
      onData('é');
    }, 0);
    (a.session as unknown as { host: HTMLElement }).host.dispatchEvent(
      new InputEvent('input', { data: 'é', inputType: 'insertText', bubbles: true }),
    );
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(writesTo(1).join('')).toBe('é');
  });

  it('n’envoie rien de plus si xterm a déjà envoyé la touche', async () => {
    const a = await registered(1);
    a.type('a'); // xterm a émis la donnée pendant la touche…
    (a.session as unknown as { host: HTMLElement }).host.dispatchEvent(
      new InputEvent('input', { data: 'a', inputType: 'insertText', bubbles: true }),
    );
    await vi.waitFor(() => {
      expect(writesTo(1)).toEqual(['a']);
    });
  });

  it('laisse à xterm les saisies composées et les autres types d’entrée', async () => {
    const a = await registered(1);
    keyThenInput(a.session, 'ê', { isComposing: true });
    keyThenInput(a.session, 'x', { inputType: 'insertFromPaste' });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(writesTo(1)).toEqual([]);
  });
});
