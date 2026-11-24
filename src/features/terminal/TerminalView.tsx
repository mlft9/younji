import '@xterm/xterm/css/xterm.css';

import { useCallback, useEffect, useRef, useState } from 'react';

import { resolveTheme, systemPrefersDark, watchSystemTheme } from '@/features/settings/theme';
import { openExternalUrl } from '@/lib/ipc';
import { useBroadcast } from '@/stores/broadcast';
import { useSessions } from '@/stores/sessions';
import { useSettings } from '@/stores/settings';

import { LinkConfirmDialog } from './LinkConfirmDialog';
import type { PasteAnalysis } from './paste';
import { PasteConfirmDialog } from './PasteConfirmDialog';
import { TerminalContextMenu } from './TerminalContextMenu';
import { TerminalSearch } from './TerminalSearch';
import { terminalSession } from './TerminalSession';

interface TerminalViewProps {
  tabId: string;
  active: boolean;
  /** Vrai si la session est utilisable (le terminal peut prendre le focus). */
  interactive: boolean;
}

/** Terminal xterm.js d'un onglet (SSH-02, SSH-05). Reste monté quand l'onglet est masqué. */
export function TerminalView({ tabId, active, interactive }: TerminalViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const searchOpen = useSessions((s) => s.tabs.find((t) => t.id === tabId)?.searchOpen ?? false);
  const update = useSessions((s) => s.update);
  const themePreference = useSettings((s) => s.theme);
  const [pasteRequest, setPasteRequest] = useState<PasteAnalysis | null>(null);
  const [linkRequest, setLinkRequest] = useState<string | null>(null);
  // Cibles de la diffusion si ce terminal en fait partie : le collage leur sera envoyé.
  const targets = useBroadcast((s) => s.targets);
  const tabs = useSessions((s) => s.tabs);
  const broadcastTo =
    targets.length >= 2 && targets.includes(tabId)
      ? tabs.filter((tab) => targets.includes(tab.id))
      : [];
  const [menu, setMenu] = useState<{ x: number; y: number; canCopy: boolean } | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return undefined;
    const session = terminalSession(tabId);
    session.attach(container, { confirmPaste: setPasteRequest, confirmLink: setLinkRequest });
    return () => {
      session.detach();
    };
  }, [tabId]);

  // Thème du terminal aligné sur celui de l'application, y compris en mode « système ».
  useEffect(() => {
    const session = terminalSession(tabId);
    session.setTheme(resolveTheme(themePreference, systemPrefersDark()));
    if (themePreference !== 'system') return undefined;
    return watchSystemTheme((dark) => {
      session.setTheme(resolveTheme('system', dark));
    });
  }, [tabId, themePreference]);

  useEffect(() => {
    if (active && interactive && !searchOpen) terminalSession(tabId).focus();
  }, [active, interactive, searchOpen, tabId]);

  const closeSearch = useCallback(() => {
    terminalSession(tabId).clearSearch();
    update(tabId, { searchOpen: false });
    terminalSession(tabId).focus();
  }, [tabId, update]);

  const closeMenu = useCallback(() => {
    setMenu(null);
  }, []);

  async function pasteFromClipboard() {
    try {
      terminalSession(tabId).paste(await navigator.clipboard.readText());
    } catch {
      // Lecture du presse-papiers refusée : Ctrl+Maj+V et Maj+Inser restent disponibles.
    }
  }

  return (
    <div className="relative h-full w-full">
      <div
        ref={containerRef}
        className="h-full w-full px-2 pt-1"
        onContextMenu={(event) => {
          event.preventDefault();
          setMenu({
            x: event.clientX,
            y: event.clientY,
            canCopy: terminalSession(tabId).terminal.hasSelection(),
          });
        }}
      />
      {searchOpen && (
        <TerminalSearch
          findNext={(term) => terminalSession(tabId).findNext(term)}
          findPrevious={(term) => terminalSession(tabId).findPrevious(term)}
          onClose={closeSearch}
        />
      )}
      {menu && (
        <TerminalContextMenu
          {...menu}
          onCopy={() =>
            void terminalSession(tabId)
              .copySelection()
              .catch(() => undefined)
          }
          onPaste={() => void pasteFromClipboard()}
          onSearch={() => {
            update(tabId, { searchOpen: true });
          }}
          onClose={closeMenu}
        />
      )}
      {pasteRequest && (
        <PasteConfirmDialog
          analysis={pasteRequest}
          broadcastTo={broadcastTo}
          onConfirm={() => {
            terminalSession(tabId).pasteConfirmed(pasteRequest);
            setPasteRequest(null);
          }}
          onCancel={() => {
            setPasteRequest(null);
            terminalSession(tabId).focus();
          }}
        />
      )}
      {linkRequest && (
        <LinkConfirmDialog
          url={linkRequest}
          onConfirm={() => {
            void openExternalUrl(linkRequest).catch(() => undefined);
            setLinkRequest(null);
          }}
          onCancel={() => {
            setLinkRequest(null);
            terminalSession(tabId).focus();
          }}
        />
      )}
    </div>
  );
}
