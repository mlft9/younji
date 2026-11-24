import type { SessionTab } from '@/stores/sessions';

/** Adresse complète d'une session (`utilisateur@hôte:port`), pour vérifier la cible. */
export function sessionAddress(tab: SessionTab): string {
  if (!tab.target) return '';
  const { host, port, username } = tab.target;
  const shownHost = host.includes(':') ? `[${host}]` : host;
  return `${username}@${shownHost}${port === 22 ? '' : `:${port}`}`;
}
