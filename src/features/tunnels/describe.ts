import type { BindScope, TunnelEntry, TunnelKind } from '@/lib/types';

/** Adresse d'écoute correspondant à une portée (TUN-03). */
export function bindAddress(scope: BindScope): string {
  return scope === 'allInterfaces' ? '0.0.0.0' : '127.0.0.1';
}

/** Hôte affiché, avec des crochets pour une adresse IPv6. */
function hostPort(host: string, port: number): string {
  return host.includes(':') && !host.startsWith('[') ? `[${host}]:${port}` : `${host}:${port}`;
}

/** Écoute affichée : `127.0.0.1:8080`. */
export function listenLabel(tunnel: Pick<TunnelEntry, 'bindScope' | 'bindPort'>): string {
  return hostPort(bindAddress(tunnel.bindScope), tunnel.bindPort);
}

/** Destination affichée : `intranet:80`, ou vide pour `-D`. */
export function destinationLabel(tunnel: Pick<TunnelEntry, 'destHost' | 'destPort'>): string {
  return tunnel.destHost === '' ? '' : hostPort(tunnel.destHost, tunnel.destPort);
}

/** Équivalent OpenSSH, pour les habitués : `-L 127.0.0.1:8080:intranet:80`. */
export function openSshFlag(
  tunnel: Omit<TunnelEntry, 'id' | 'name' | 'hostId' | 'autoStart'>,
): string {
  const listen = listenLabel(tunnel);
  switch (tunnel.kind) {
    case 'local':
      return `-L ${listen}:${destinationLabel(tunnel)}`;
    case 'remote':
      return `-R ${listen}:${destinationLabel(tunnel)}`;
    case 'dynamic':
      return `-D ${listen}`;
  }
}

/** Port valide : entier de 1 à 65535. */
export function isValidPort(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 65535;
}

/**
 * Destination admise : mêmes caractères que le cœur (lettres, chiffres, `.`, `-`, `_`, `:`,
 * `[`, `]`), 255 caractères au plus. Le cœur revalide la forme exacte.
 */
export function isValidDestination(host: string): boolean {
  return /^[A-Za-z0-9._:[\]-]{1,255}$/.test(host);
}

/** Les types qui ont une destination fixe. */
export function hasDestination(kind: TunnelKind): boolean {
  return kind !== 'dynamic';
}
