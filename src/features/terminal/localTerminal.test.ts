import * as ipc from '@/lib/ipc';
import type * as IpcModule from '@/lib/ipc';
import type { SessionEvent } from '@/lib/types';
import type { LocalProfile } from '@/lib/types';
import { HOME_TAB_ID, useSessions } from '@/stores/sessions';

import { splitAndConnect } from './split';
import { closeTab, terminalSession, TerminalSession } from './TerminalSession';

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
    termOpen: vi.fn(),
    termWrite: vi.fn().mockResolvedValue(undefined),
    termResize: vi.fn().mockResolvedValue(undefined),
    termAck: vi.fn().mockResolvedValue(undefined),
    termClose: vi.fn().mockResolvedValue(undefined),
  };
});

const PROFILE: LocalProfile = {
  id: 'shell:/bin/bash',
  name: 'bash',
  kind: 'shell',
  isDefault: true,
};

interface Listeners {
  onEvent: (event: SessionEvent) => void;
  onOutput: (data: Uint8Array) => void;
}

// jsdom n'implémente pas ResizeObserver.
vi.stubGlobal(
  'ResizeObserver',
  class {
    observe = vi.fn();
    disconnect = vi.fn();
  },
);

async function openLocal(id = 3) {
  const tabId = useSessions.getState().openLocalTab(PROFILE);
  let listeners: Listeners | undefined;
  vi.mocked(ipc.termOpen).mockImplementationOnce((_request, l) => {
    listeners = l;
    return Promise.resolve(id);
  });
  const session = terminalSession(tabId);
  await session.openLocal(PROFILE);
  const tab = () => useSessions.getState().tabs.find((t) => t.id === tabId);
  return { tabId, session, tab, emit: (e: SessionEvent) => listeners?.onEvent(e) };
}

describe('terminal local', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSessions.setState({ tabs: [], activeTabId: HOME_TAB_ID, layouts: {}, focusedPane: {} });
  });

  it('ouvre le profil par son seul identifiant et devient utilisable', async () => {
    const { tab } = await openLocal();
    expect(vi.mocked(ipc.termOpen).mock.calls[0]?.[0]).toEqual({
      profileId: 'shell:/bin/bash',
      cols: 80,
      rows: 24,
    });
    expect(tab()).toMatchObject({ status: 'connected', local: PROFILE, target: null });
    expect(ipc.sshConnect).not.toHaveBeenCalled();
  });

  it('écrit, acquitte et ferme par les commandes term_*, jamais ssh_*', async () => {
    const { tabId, session } = await openLocal(9);
    session.sendText('ls', true);
    await vi.waitFor(() => {
      expect(ipc.termWrite).toHaveBeenCalledWith(9, 'ls\r', false);
    });
    closeTab(tabId);
    expect(ipc.termClose).toHaveBeenCalledWith(9);
    expect(ipc.sshWrite).not.toHaveBeenCalled();
    expect(ipc.sshClose).not.toHaveBeenCalled();
  });

  it('propose de relancer quand le shell se termine', async () => {
    const { emit, tab } = await openLocal();
    emit({ type: 'disconnected', code: 'term.exited' });
    expect(tab()).toMatchObject({
      status: 'disconnected',
      errorCode: 'term.exited',
      sessionId: null,
    });
  });

  it('affiche l’erreur du cœur si le lancement échoue', async () => {
    const tabId = useSessions.getState().openLocalTab(PROFILE);
    vi.mocked(ipc.termOpen).mockRejectedValueOnce({ code: 'term.spawn_failed' });
    await new TerminalSession(tabId).openLocal(PROFILE);
    expect(useSessions.getState().tabs.find((t) => t.id === tabId)).toMatchObject({
      status: 'form',
      errorCode: 'term.spawn_failed',
    });
  });

  it('ne branche pas une session déjà terminée avant le retour de l’identifiant', async () => {
    const tabId = useSessions.getState().openLocalTab(PROFILE);
    vi.mocked(ipc.termOpen).mockImplementationOnce((_request, l) => {
      l.onEvent({ type: 'disconnected', code: 'term.exited' });
      return Promise.resolve(4);
    });
    const session = new TerminalSession(tabId);
    await session.openLocal(PROFILE);
    expect(session.isConnected()).toBe(false);
  });

  it('un panneau découpé ouvre le même profil', async () => {
    const { tabId } = await openLocal();
    vi.mocked(ipc.termOpen).mockResolvedValueOnce(5);
    const pane = splitAndConnect(tabId, 'row');
    expect(pane).not.toBeNull();
    await vi.waitFor(() => {
      expect(ipc.termOpen).toHaveBeenCalledTimes(2);
    });
    expect(useSessions.getState().tabs.find((t) => t.id === pane)?.local).toEqual(PROFILE);
  });

  it('n’ouvre pas d’explorateur SFTP sur un terminal local', async () => {
    const { tabId } = await openLocal();
    expect(useSessions.getState().openSftpTab(tabId)).toBeNull();
  });
});
