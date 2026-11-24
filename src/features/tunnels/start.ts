import type { HostEntry, TunnelEntry } from '@/lib/types';
import { useTunnels } from '@/stores/tunnels';

/** Démarre un tunnel du coffre avec l'adresse de son hôte (affichée dans les dialogues). */
export function startTunnel(tunnel: TunnelEntry, hosts: readonly HostEntry[]): void {
  const host = hosts.find((h) => h.id === tunnel.hostId);
  if (!host) return;
  void useTunnels.getState().start(tunnel, { address: host.address, port: host.port });
}
