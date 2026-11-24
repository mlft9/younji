// Tunnels démarrés (TUN-02). Un tunnel du coffre est démarré sur une connexion SSH dédiée ;
// cet état vit hors de la section Tunnels pour que les tunnels continuent de tourner (et que
// leurs demandes de clé d'hôte ou de 2FA s'affichent) quelle que soit la section affichée.
// Il n'est jamais persisté : au verrouillage, le cœur ferme les tunnels et l'état est vidé.

import { create } from 'zustand';

import { sshAuthAnswer, sshClose, sshHostKeyDecision, toAppError, tunnelStart } from '@/lib/ipc';
import type {
  AuthPromptInfo,
  EntryId,
  HostKeyInfo,
  SessionEvent,
  SessionId,
  TunnelEntry,
} from '@/lib/types';

/** Décision attendue de l'utilisateur pour un tunnel en cours de connexion. */
export type TunnelPending =
  | { kind: 'hostKey'; key: HostKeyInfo }
  | { kind: 'prompt'; prompt: AuthPromptInfo; errorCode: string | null };

export interface TunnelRun {
  /** Jeton propre à ce démarrage : les événements d'un démarrage précédent sont ignorés. */
  runKey: number;
  sessionId: SessionId | null;
  /** `connecting` jusqu'à ce que l'écoute soit en place. */
  status: 'connecting' | 'running';
  /** Hôte et port SSH, pour les dialogues de clé d'hôte et de 2FA. */
  address: string;
  port: number;
  pending: TunnelPending | null;
  active: number;
  total: number;
  /** Arrêt demandé avant que le cœur ait renvoyé l'identifiant de session. */
  stopRequested: boolean;
  /** Réponses données avant que l'identifiant de session soit connu, envoyées dès qu'il l'est. */
  deferred: ((sessionId: SessionId) => void)[];
}

interface TunnelsState {
  runs: Record<EntryId, TunnelRun>;
  /** Dernière erreur par tunnel (code), effacée au démarrage suivant. */
  errors: Record<EntryId, string>;
  /** Démarre un tunnel (sans effet s'il tourne déjà). */
  start: (tunnel: TunnelEntry, host: { address: string; port: number }) => Promise<void>;
  /** Arrête un tunnel : le cœur ferme l'écoute (port libéré) et les connexions relayées. */
  stop: (tunnelId: EntryId) => void;
  decideHostKey: (tunnelId: EntryId, accept: boolean) => void;
  answerPrompt: (tunnelId: EntryId, answers: string[] | null) => void;
  /** Oubli total (verrouillage) : le cœur a déjà fermé les connexions. */
  reset: () => void;
}

let nextRunKey = 1;

/** Copie de `record` sans la clé `key`. */
function without<T>(record: Record<EntryId, T>, key: EntryId): Record<EntryId, T> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => k !== key));
}

export const useTunnels = create<TunnelsState>()((set, get) => {
  /** Applique `change` au démarrage `runKey` du tunnel, s'il est toujours le démarrage actuel. */
  function update(tunnelId: EntryId, runKey: number, change: (run: TunnelRun) => TunnelRun) {
    const run = get().runs[tunnelId];
    if (run?.runKey !== runKey) return;
    set((s) => ({ runs: { ...s.runs, [tunnelId]: change(run) } }));
  }

  function finish(tunnelId: EntryId, runKey: number, code: string | null) {
    const run = get().runs[tunnelId];
    if (run?.runKey !== runKey) return;
    set((s) => {
      const runs = without(s.runs, tunnelId);
      const errors = { ...s.errors };
      if (code !== null) errors[tunnelId] = code;
      return { runs, errors };
    });
  }

  function onEvent(tunnelId: EntryId, runKey: number, event: SessionEvent) {
    switch (event.type) {
      case 'hostKeyUnknown':
        update(tunnelId, runKey, (run) => ({
          ...run,
          pending: { kind: 'hostKey', key: event.key },
        }));
        break;
      case 'authPrompt':
        update(tunnelId, runKey, (run) => ({
          ...run,
          pending: {
            kind: 'prompt',
            prompt: {
              round: event.round,
              name: event.name,
              instructions: event.instructions,
              prompts: event.prompts,
            },
            errorCode: null,
          },
        }));
        break;
      case 'authenticating':
        update(tunnelId, runKey, (run) => ({ ...run, pending: null }));
        break;
      case 'tunnelReady':
        update(tunnelId, runKey, (run) => ({ ...run, status: 'running', pending: null }));
        break;
      case 'tunnelStats':
        update(tunnelId, runKey, (run) => ({
          ...run,
          active: event.active,
          total: event.total,
        }));
        break;
      case 'disconnected':
        // Arrêt demandé : pas une erreur. Clé d'hôte modifiée : le code l'explique.
        finish(tunnelId, runKey, event.code === 'ssh.session_closed' ? null : event.code);
        break;
      case 'hostKeyChanged':
      case 'keyDeployed':
      case 'connected':
        break;
    }
  }

  return {
    runs: {},
    errors: {},

    start: async (tunnel, host) => {
      if (get().runs[tunnel.id]) return;
      const runKey = nextRunKey++;
      set((s) => {
        const errors = without(s.errors, tunnel.id);
        return {
          errors,
          runs: {
            ...s.runs,
            [tunnel.id]: {
              runKey,
              sessionId: null,
              status: 'connecting',
              address: host.address,
              port: host.port,
              pending: null,
              active: 0,
              total: 0,
              stopRequested: false,
              deferred: [],
            },
          },
        };
      });
      try {
        const sessionId = await tunnelStart(tunnel.id, (event) => {
          onEvent(tunnel.id, runKey, event);
        });
        const run = get().runs[tunnel.id];
        if (run?.runKey !== runKey || run.stopRequested) {
          // Arrêté (ou verrouillé) pendant le démarrage.
          void sshClose(sessionId).catch(() => undefined);
          finish(tunnel.id, runKey, null);
          return;
        }
        update(tunnel.id, runKey, (r) => ({ ...r, sessionId, deferred: [] }));
        for (const send of run.deferred) send(sessionId);
      } catch (failure) {
        finish(tunnel.id, runKey, toAppError(failure).code);
      }
    },

    stop: (tunnelId) => {
      const run = get().runs[tunnelId];
      if (!run) return;
      if (run.sessionId === null) {
        update(tunnelId, run.runKey, (r) => ({ ...r, stopRequested: true }));
        return;
      }
      void sshClose(run.sessionId).catch(() => undefined);
      finish(tunnelId, run.runKey, null);
    },

    decideHostKey: (tunnelId, accept) => {
      const run = get().runs[tunnelId];
      if (run?.pending?.kind !== 'hostKey') return;
      const send = (sessionId: SessionId) => {
        void sshHostKeyDecision(sessionId, accept).catch(() => undefined);
      };
      update(tunnelId, run.runKey, (r) => ({
        ...r,
        pending: null,
        deferred: r.sessionId === null ? [...r.deferred, send] : r.deferred,
      }));
      if (run.sessionId !== null) send(run.sessionId);
    },

    answerPrompt: (tunnelId, answers) => {
      const run = get().runs[tunnelId];
      const pending = run?.pending;
      if (!run || pending?.kind !== 'prompt') return;
      const { runKey } = run;
      const send = (sessionId: SessionId) => {
        sshAuthAnswer(sessionId, pending.prompt.round, answers).catch((failure: unknown) => {
          const code = toAppError(failure).code;
          // Réponses refusées par le cœur : la question est toujours en attente.
          if (code === 'ssh.invalid_answers') {
            update(tunnelId, runKey, (r) => ({
              ...r,
              pending: { kind: 'prompt', prompt: pending.prompt, errorCode: code },
            }));
          }
        });
      };
      update(tunnelId, runKey, (r) => ({
        ...r,
        pending: null,
        deferred: r.sessionId === null ? [...r.deferred, send] : r.deferred,
      }));
      if (run.sessionId !== null) send(run.sessionId);
    },

    reset: () => {
      set({ runs: {}, errors: {} });
    },
  };
});
