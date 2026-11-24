import { useEffect, useRef } from 'react';

import { AuthPromptDialog } from '@/features/connect/AuthPromptDialog';
import { HostKeyDialog } from '@/features/connect/HostKeyDialog';
import { useTunnels } from '@/stores/tunnels';
import { EMPTY_SNAPSHOT, useVault } from '@/stores/vault';

import { startTunnel } from './start';

/**
 * Partie des tunnels montée avec l'application (hors de la section Tunnels) :
 * - les demandes de clé d'hôte et de 2FA d'un tunnel en cours de connexion, quelle que soit la
 *   section affichée (une à la fois) ;
 * - le démarrage automatique (TUN-02), une seule fois par déverrouillage du coffre : ce
 *   composant n'est monté que coffre déverrouillé.
 */
export function TunnelBackground() {
  const snapshot = useVault((s) => s.snapshot);
  const runs = useTunnels((s) => s.runs);
  const autoStarted = useRef(false);

  useEffect(() => {
    // Attendre le contenu réel du coffre (le contenu vide est affiché pendant sa lecture).
    if (autoStarted.current || snapshot === EMPTY_SNAPSHOT) return;
    autoStarted.current = true;
    for (const tunnel of snapshot.tunnels) {
      if (tunnel.autoStart) startTunnel(tunnel, snapshot.hosts);
    }
  }, [snapshot]);

  const waiting = Object.entries(runs).find(([, run]) => run.pending !== null);
  if (!waiting) return null;
  const [tunnelId, run] = waiting;
  const pending = run.pending;
  const address = run.port === 22 ? run.address : `${run.address}:${run.port}`;

  if (pending?.kind === 'hostKey') {
    return (
      <HostKeyDialog
        host={run.address}
        port={run.port}
        hostKey={pending.key}
        onDecision={(accept) => {
          useTunnels.getState().decideHostKey(tunnelId, accept);
        }}
      />
    );
  }
  if (pending?.kind === 'prompt') {
    return (
      <AuthPromptDialog
        key={`${tunnelId}-${String(pending.prompt.round)}`}
        address={address}
        prompt={pending.prompt}
        errorCode={pending.errorCode}
        onSubmit={(answers) => {
          useTunnels.getState().answerPrompt(tunnelId, answers);
        }}
        onCancel={() => {
          useTunnels.getState().answerPrompt(tunnelId, null);
        }}
      />
    );
  }
  return null;
}
