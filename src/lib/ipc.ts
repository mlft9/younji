// Seul point d'appel au backend Rust. Chaque commande a sa fonction typée ;
// les composants n'appellent jamais `invoke` directement (règle ESLint).
import { Channel, invoke } from '@tauri-apps/api/core';

import type {
  AppError,
  AppInfo,
  AppSettings,
  ConflictAction,
  ConnectRequest,
  DownloadTarget,
  EditId,
  EntryId,
  HostRequest,
  IdentityRequest,
  KeyEntry,
  KeyDeployRequest,
  KeyFileRef,
  KeyGenerateRequest,
  KeyImportRequest,
  KnownHostEntry,
  JobId,
  LocalListing,
  LocalRootId,
  LocalRootRef,
  RemoteListing,
  SessionEvent,
  SessionId,
  SnippetRequest,
  SettingsResponse,
  SftpEvent,
  SftpId,
  TunnelRequest,
  UploadSource,
  VaultEvent,
  VaultSnapshot,
  VaultStatus,
} from './types';

/** Code utilisé quand une erreur ne vient pas du backend sous la forme attendue. */
export const UNKNOWN_ERROR_CODE = 'unknown';

/** Vérifie qu'une valeur reçue est bien une `AppError` (le backend reste la seule source des codes). */
export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    /^[a-z0-9_]+(\.[a-z0-9_]+)*$/.test(value.code)
  );
}

/**
 * Normalise toute erreur en `AppError`. Le message brut éventuel n'est jamais propagé
 * vers l'interface : il pourrait contenir des données non maîtrisées.
 */
export function toAppError(error: unknown): AppError {
  return isAppError(error) ? { code: error.code } : { code: UNKNOWN_ERROR_CODE };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    // L'appelant reçoit une AppError normalisée (objet simple, volontairement pas une instance d'Error).
    // eslint-disable-next-line @typescript-eslint/only-throw-error
    throw toAppError(error);
  }
}

/** Nom et version de l'application. */
export function appInfo(): Promise<AppInfo> {
  return call<AppInfo>('app_info');
}

/** Destinataires des flux d'une session SSH. */
export interface SessionListeners {
  /** Changement d'état (clé d'hôte, connexion, fin). */
  onEvent: (event: SessionEvent) => void;
  /** Sortie du terminal, en octets bruts, à acquitter avec `sshAck` une fois affichée. */
  onOutput: (data: Uint8Array) => void;
}

/**
 * Ouvre une session SSH. Le secret éventuel de `request.auth` n'est transmis qu'ici, une seule
 * fois ; il ne revient jamais du cœur.
 */
export function sshConnect(
  request: ConnectRequest,
  listeners: SessionListeners,
): Promise<SessionId> {
  return call<SessionId>('ssh_connect', { request, ...sessionChannels(listeners) });
}

/** Ouvre une session vers un hôte du carnet, avec son identité enregistrée (aucun secret ici). */
export function sshConnectHost(
  request: { hostId: EntryId; cols: number; rows: number },
  listeners: SessionListeners,
): Promise<SessionId> {
  return call<SessionId>('ssh_connect_host', { request, ...sessionChannels(listeners) });
}

function sessionChannels(listeners: SessionListeners) {
  const onEvent = new Channel<SessionEvent>(listeners.onEvent);
  const onOutput = new Channel<ArrayBuffer>((buffer) => {
    listeners.onOutput(new Uint8Array(buffer));
  });
  return { onEvent, onOutput };
}

/** Envoie une saisie. `binary` : chaque caractère représente un octet brut (0–255). */
export function sshWrite(sessionId: SessionId, data: string, binary = false): Promise<void> {
  return call<undefined>('ssh_write', { sessionId, data, binary });
}

/** Répercute la taille du terminal sur le serveur. */
export function sshResize(sessionId: SessionId, cols: number, rows: number): Promise<void> {
  return call<undefined>('ssh_resize', { sessionId, cols, rows });
}

/** Acquitte des octets de sortie affichés (contrôle de flux). */
export function sshAck(sessionId: SessionId, bytes: number): Promise<void> {
  return call<undefined>('ssh_ack', { sessionId, bytes });
}

/**
 * Répond aux questions du serveur (keyboard-interactive), ou annule (`null`). Les réponses
 * (codes TOTP…) ne sont transmises qu'ici, une fois ; elles ne reviennent jamais du cœur.
 */
export function sshAuthAnswer(
  sessionId: SessionId,
  round: number,
  answers: string[] | null,
): Promise<void> {
  return call<undefined>('ssh_auth_answer', { sessionId, round, answers });
}

/** Accepte ou refuse une clé d'hôte inconnue. */
export function sshHostKeyDecision(sessionId: SessionId, accept: boolean): Promise<void> {
  return call<undefined>('ssh_host_key_decision', { sessionId, accept });
}

/** Ferme une session. */
export function sshClose(sessionId: SessionId): Promise<void> {
  return call<undefined>('ssh_close', { sessionId });
}

/** Ouvre la boîte de dialogue du système pour choisir une clé privée (`null` si annulé). */
export function sshPickKeyFile(): Promise<KeyFileRef | null> {
  return call<KeyFileRef | null>('ssh_pick_key_file');
}

/** Ouvre un lien http(s) dans le navigateur du système (revalidé par le cœur). */
export function openExternalUrl(url: string): Promise<void> {
  return call<undefined>('open_external_url', { url });
}

// ------------------------------------------------------------------ coffre

/** État du coffre : absent, verrouillé ou déverrouillé. */
export function vaultStatus(): Promise<VaultStatus> {
  return call<VaultStatus>('vault_status');
}

/** Crée le coffre (mot de passe transmis une seule fois, politique vérifiée par le cœur). */
export function vaultCreate(password: string): Promise<void> {
  return call<undefined>('vault_create', { password });
}

/** Déverrouille le coffre. */
export function vaultUnlock(password: string): Promise<void> {
  return call<undefined>('vault_unlock', { password });
}

/** Verrouille le coffre ; le cœur ferme aussi toutes les sessions. */
export function vaultLock(): Promise<void> {
  return call<undefined>('vault_lock');
}

/** Change le mot de passe maître. */
export function vaultChangePassword(current: string, next: string): Promise<void> {
  return call<undefined>('vault_change_password', { current, new: next });
}

/** Signale une activité de l'utilisateur au minuteur de verrouillage du cœur. */
export function vaultTouch(): Promise<void> {
  return call<undefined>('vault_touch');
}

/** S'abonne aux événements du coffre (verrouillage automatique). */
export async function vaultWatch(onEvent: (event: VaultEvent) => void): Promise<void> {
  // `async` : hors de Tauri, la création du Channel échoue en rejet plutôt qu'en exception.
  await call<undefined>('vault_watch', { onEvent: new Channel<VaultEvent>(onEvent) });
}

/** Change le délai de verrouillage automatique. */
export function vaultSetAutoLock(minutes: number): Promise<void> {
  return call<undefined>('vault_set_auto_lock', { minutes });
}

/** Contenu affichable du coffre (sans secret). */
export function vaultSnapshot(): Promise<VaultSnapshot> {
  return call<VaultSnapshot>('vault_snapshot');
}

// ------------------------------------------------------------------ carnet et clés

/** Crée ou modifie un hôte. */
export function hostSave(request: HostRequest): Promise<EntryId> {
  return call<EntryId>('host_save', { request });
}

/** Supprime un hôte. */
export function hostDelete(id: EntryId): Promise<void> {
  return call<undefined>('host_delete', { id });
}

/** Crée ou renomme un groupe. */
export function groupSave(request: { id: EntryId | null; name: string }): Promise<EntryId> {
  return call<EntryId>('group_save', { request });
}

/** Supprime un groupe (ses hôtes passent « sans groupe »). */
export function groupDelete(id: EntryId): Promise<void> {
  return call<undefined>('group_delete', { id });
}

/** Crée ou modifie un snippet (SNP-01). */
export function snippetSave(request: SnippetRequest): Promise<EntryId> {
  return call<EntryId>('snippet_save', { request });
}

/** Crée ou modifie un tunnel (TUN-02). */
export function tunnelSave(request: TunnelRequest): Promise<EntryId> {
  return call<EntryId>('tunnel_save', { request });
}

/** Supprime un tunnel (l'arrêter d'abord s'il tourne). */
export function tunnelDelete(id: EntryId): Promise<void> {
  return call<undefined>('tunnel_delete', { id });
}

/**
 * Démarre un tunnel du coffre sur une connexion SSH dédiée. L'état arrive par `onEvent` ; le
 * tunnel se pilote comme une session (`sshHostKeyDecision`, `sshAuthAnswer`) et s'arrête par
 * `sshClose`, ce qui libère le port.
 */
export async function tunnelStart(
  tunnelId: EntryId,
  onEvent: (event: SessionEvent) => void,
): Promise<SessionId> {
  return call<SessionId>('tunnel_start', {
    tunnelId,
    onEvent: new Channel<SessionEvent>(onEvent),
  });
}

/** Supprime un snippet. */
export function snippetDelete(id: EntryId): Promise<void> {
  return call<undefined>('snippet_delete', { id });
}

/** Crée ou modifie une identité (le mot de passe éventuel n'est transmis qu'ici). */
export function identitySave(request: IdentityRequest): Promise<EntryId> {
  return call<EntryId>('identity_save', { request });
}

/** Supprime une identité. */
export function identityDelete(id: EntryId): Promise<void> {
  return call<undefined>('identity_delete', { id });
}

/** Importe dans le coffre une clé choisie par `sshPickKeyFile`. */
export function keyImport(request: KeyImportRequest): Promise<KeyEntry> {
  return call<KeyEntry>('key_import', { request });
}

/** Génère une clé dans le coffre ; seule la partie publique revient. */
export function keyGenerate(request: KeyGenerateRequest): Promise<KeyEntry> {
  return call<KeyEntry>('key_generate', { request });
}

/** Exporte la clé publique dans un fichier choisi par le système (`false` si annulé). */
export function keyExportPublic(id: EntryId): Promise<boolean> {
  return call<boolean>('key_export_public', { id });
}

/**
 * Déploie une clé du coffre sur un hôte du carnet. Le déroulé (clé d'hôte, questions du
 * serveur, issue) arrive par `onEvent` ; la session se pilote comme une session SSH
 * (`sshHostKeyDecision`, `sshAuthAnswer`, `sshClose`).
 */
export async function keyDeploy(
  request: KeyDeployRequest,
  onEvent: (event: SessionEvent) => void,
): Promise<SessionId> {
  return call<SessionId>('key_deploy', { request, onEvent: new Channel<SessionEvent>(onEvent) });
}

/** Renomme une clé. */
export function keyRename(id: EntryId, name: string): Promise<void> {
  return call<undefined>('key_rename', { id, name });
}

/** Supprime une clé. */
export function keyDelete(id: EntryId): Promise<void> {
  return call<undefined>('key_delete', { id });
}

/** Clés d'hôte connues (HKY-04). */
export function knownHostList(): Promise<KnownHostEntry[]> {
  return call<KnownHostEntry[]>('known_host_list');
}

/** Supprime une clé d'hôte connue ; renvoie le nombre de lignes retirées. */
export function knownHostDelete(entry: KnownHostEntry): Promise<number> {
  return call<number>('known_host_delete', { entry });
}

// ------------------------------------------------------------------ paramètres

/** Paramètres d'apparence enregistrés. */
export function settingsGet(): Promise<SettingsResponse> {
  return call<SettingsResponse>('settings_get');
}

/** Enregistre les paramètres d'apparence. */
export function settingsSet(settings: AppSettings): Promise<void> {
  return call<undefined>('settings_set', { settings });
}

// ---------------------------------------------------------------------- SFTP (étape 4)

/**
 * Ouvre un client SFTP sur la connexion d'une session de terminal (sans nouvelle
 * authentification). Les transferts et éditions de ce client émettent sur `onEvent`.
 */
export async function sftpOpen(
  sessionId: SessionId,
  onEvent: (event: SftpEvent) => void,
): Promise<SftpId> {
  return call<SftpId>('sftp_open', { sessionId, onEvent: new Channel<SftpEvent>(onEvent) });
}

export function sftpClose(sftpId: SftpId): Promise<void> {
  return call<undefined>('sftp_close', { sftpId });
}

/** Dossier personnel de l'utilisateur distant. */
export function sftpHome(sftpId: SftpId): Promise<string> {
  return call<string>('sftp_home', { sftpId });
}

export function sftpList(sftpId: SftpId, path: string): Promise<RemoteListing> {
  return call<RemoteListing>('sftp_list', { sftpId, path });
}

export function sftpMkdir(sftpId: SftpId, path: string): Promise<void> {
  return call<undefined>('sftp_mkdir', { sftpId, path });
}

/** Renomme sans jamais écraser (la destination ne doit pas exister). */
export function sftpRename(sftpId: SftpId, from: string, to: string): Promise<void> {
  return call<undefined>('sftp_rename', { sftpId, from, to });
}

/** Supprime un élément (récursif pour un dossier, sans suivre les liens). */
export function sftpRemove(sftpId: SftpId, path: string): Promise<void> {
  return call<undefined>('sftp_remove', { sftpId, path });
}

export function sftpChmod(sftpId: SftpId, path: string, mode: number): Promise<void> {
  return call<undefined>('sftp_chmod', { sftpId, path, mode });
}

/**
 * Ajoute un téléchargement à la file ; `null` si la boîte de dialogue a été annulée. Les
 * conflits sont toujours posés à l'utilisateur (aucun écrasement décidé d'avance).
 */
export function sftpDownload(
  sftpId: SftpId,
  paths: string[],
  target: DownloadTarget,
): Promise<JobId | null> {
  return call<JobId | null>('sftp_download', { sftpId, paths, target });
}

/** Ajoute un envoi à la file ; `null` si la boîte de dialogue a été annulée. */
export function sftpUpload(
  sftpId: SftpId,
  source: UploadSource,
  remoteDir: string,
): Promise<JobId | null> {
  return call<JobId | null>('sftp_upload', { sftpId, source, remoteDir });
}

export function sftpTransferCancel(sftpId: SftpId, job: JobId): Promise<void> {
  return call<undefined>('sftp_transfer_cancel', { sftpId, job });
}

export function sftpConflictDecision(
  sftpId: SftpId,
  job: JobId,
  action: ConflictAction,
  applyToAll: boolean,
): Promise<void> {
  return call<undefined>('sftp_conflict_decision', { sftpId, job, action, applyToAll });
}

/** Ouvre le fichier dans l'éditeur de texte local ; chaque enregistrement est renvoyé. */
export async function sftpEdit(
  sftpId: SftpId,
  path: string,
  onEvent: (event: SftpEvent) => void,
): Promise<EditId> {
  return call<EditId>('sftp_edit', { sftpId, path, onEvent: new Channel<SftpEvent>(onEvent) });
}

export function sftpEditClose(editId: EditId): Promise<void> {
  return call<undefined>('sftp_edit_close', { editId });
}

/** Renvoie la copie locale malgré un conflit (écrase le fichier distant). */
export function sftpEditForce(editId: EditId): Promise<void> {
  return call<undefined>('sftp_edit_force', { editId });
}

/** Choisit un dossier local dans la boîte de dialogue du cœur ; `null` si annulé. */
export function localPickRoot(): Promise<LocalRootRef | null> {
  return call<LocalRootRef | null>('local_pick_root');
}

/** Liste un dossier (chemin relatif, `/` comme séparateur) d'un dossier local choisi. */
export function localList(rootId: LocalRootId, path: string): Promise<LocalListing> {
  return call<LocalListing>('local_list', { rootId, path });
}
