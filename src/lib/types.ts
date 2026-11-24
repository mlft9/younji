// Types partagés avec le backend. Ils reflètent les structures Rust de src-tauri
// (serde en camelCase) : toute modification doit être faite des deux côtés.

/** Erreur renvoyée par une commande : uniquement un code stable, traduit par `errors.<code>`. */
export interface AppError {
  code: string;
}

/** Réponse de la commande `app_info` (src-tauri/src/commands/app.rs). */
export interface AppInfo {
  name: string;
  version: string;
  /** Commit compilé (7 caractères), pour savoir quelle compilation est testée. */
  commit: string;
}

/** Identifiant opaque d'une session SSH, attribué par le cœur. */
export type SessionId = number;

/** Fichier de clé choisi dans la boîte de dialogue du système (le chemin reste dans le cœur). */
export interface KeyFileRef {
  id: number;
  name: string;
}

/** Moyen d'authentification envoyé une seule fois à `ssh_connect`. */
export type AuthRequest =
  | { kind: 'password'; password: string }
  | { kind: 'key'; keyFileId: number; passphrase: string | null }
  /** Aucun secret d'avance : les questions du serveur (2FA) sont posées au fil de l'eau. */
  | { kind: 'interactive' };

/** Demande de connexion (src-tauri/src/commands/ssh.rs, `ConnectRequest`). */
export interface ConnectRequest {
  host: string;
  port: number;
  username: string;
  auth: AuthRequest;
  cols: number;
  rows: number;
}

/** Description d'une clé d'hôte : algorithme et empreinte `SHA256:…`. */
export interface HostKeyInfo {
  algorithm: string;
  fingerprint: string;
}

/** Question posée par le serveur (keyboard-interactive), texte déjà nettoyé par le cœur. */
export interface PromptField {
  text: string;
  /** `true` : la réponse peut s'afficher pendant la saisie (choix du serveur). */
  echo: boolean;
}

/** Demande du serveur : un tour de questions (code TOTP, mot de passe PAM…). */
export interface AuthPromptInfo {
  /** Numéro de la demande, renvoyé avec les réponses (une réponse ne vaut que pour ce tour). */
  round: number;
  name: string;
  instructions: string;
  prompts: PromptField[];
}

/** Événement d'état d'une session (`SessionEventDto`). */
export type SessionEvent =
  | { type: 'hostKeyUnknown'; key: HostKeyInfo }
  | {
      type: 'hostKeyChanged';
      key: HostKeyInfo;
      known: HostKeyInfo[];
      hostPattern: string;
    }
  | { type: 'authenticating' }
  | ({ type: 'authPrompt' } & AuthPromptInfo)
  | { type: 'keyDeployed'; added: boolean }
  | { type: 'connected' }
  | { type: 'tunnelReady'; port: number }
  | { type: 'tunnelStats'; active: number; total: number }
  | { type: 'disconnected'; code: string };

// ------------------------------------------------------------------ coffre (étape 2)

/** État du coffre (`vault_status`). */
export type VaultStatus = 'missing' | 'locked' | 'unlocked';

/** Cause d'un verrouillage. */
export type LockReason = 'manual' | 'idle' | 'reload';

/** Événement du coffre reçu par le `Channel` de `vault_watch`. */
export interface VaultEvent {
  type: 'locked';
  reason: LockReason;
}

/** Identifiant opaque d'une entrée du coffre (UUID). */
export type EntryId = string;

/** Hôte du carnet (aucun secret). */
export interface HostEntry {
  id: EntryId;
  name: string;
  address: string;
  port: number;
  username: string;
  identityId: EntryId | null;
  groupId: EntryId | null;
  tags: string[];
  notes: string;
}

/** Groupe d'hôtes. */
export interface GroupEntry {
  id: EntryId;
  name: string;
}

/** Identité : le mot de passe ne quitte jamais le cœur, seule sa présence est connue. */
export interface IdentityEntry {
  id: EntryId;
  name: string;
  username: string;
  hasPassword: boolean;
  keyId: EntryId | null;
}

/** Clé importée : partie publique uniquement. */
export interface KeyEntry {
  id: EntryId;
  name: string;
  algorithm: string;
  fingerprint: string;
  publicKey: string;
}

/** Contenu affichable du coffre (`vault_snapshot`). */
export interface VaultSnapshot {
  hosts: HostEntry[];
  groups: GroupEntry[];
  identities: IdentityEntry[];
  keys: KeyEntry[];
  snippets: SnippetEntry[];
  tunnels: TunnelEntry[];
  autoLockMinutes: number;
}

/** Type de redirection (TUN-01) : `-L`, `-R` ou `-D` (SOCKS5). */
export type TunnelKind = 'local' | 'remote' | 'dynamic';

/** Portée de l'écoute (TUN-03) : `127.0.0.1` ou `0.0.0.0`. Aucune adresse libre. */
export type BindScope = 'loopback' | 'allInterfaces';

/** Tunnel du coffre, rattaché à un hôte du carnet (TUN-02). */
export interface TunnelEntry {
  id: EntryId;
  name: string;
  hostId: EntryId;
  kind: TunnelKind;
  bindScope: BindScope;
  bindPort: number;
  /** Destination (vide pour `-D`). */
  destHost: string;
  /** Port de destination (0 pour `-D`). */
  destPort: number;
  /** Démarrage automatique au déverrouillage du coffre. */
  autoStart: boolean;
}

/**
 * Création (`id` nul) ou modification d'un tunnel. `exposureConfirmed` est exigé par le cœur
 * pour une écoute sur toutes les interfaces.
 */
export type TunnelRequest = Omit<TunnelEntry, 'id'> & {
  id: EntryId | null;
  exposureConfirmed: boolean;
};

/** Snippet du coffre (SNP-01). Ce n'est pas un secret : il est affiché et envoyé au terminal. */
export interface SnippetEntry {
  id: EntryId;
  name: string;
  content: string;
  description: string;
  /** Variables `{{nom}}`, dans l'ordre d'apparition (calculées par le cœur). */
  variables: string[];
}

/** Création (`id` nul) ou modification d'un snippet. */
export interface SnippetRequest {
  id: EntryId | null;
  name: string;
  content: string;
  description: string;
}

/** Création (`id` nul) ou modification d'un hôte. */
export type HostRequest = Omit<HostEntry, 'id'> & { id: EntryId | null };

/** Changement du mot de passe d'une identité. */
export type PasswordRequest =
  { action: 'keep' } | { action: 'clear' } | { action: 'set'; value: string };

/** Création (`id` nul) ou modification d'une identité. */
export interface IdentityRequest {
  id: EntryId | null;
  name: string;
  username: string;
  password: PasswordRequest;
  keyId: EntryId | null;
}

/** Import d'une clé choisie par `ssh_pick_key_file`. */
export interface KeyImportRequest {
  keyFileId: number;
  name: string;
  passphrase: string | null;
}

/** Type de clé à générer (KEY-01). */
export type KeyKind = 'ed25519' | 'ecdsaP256' | 'ecdsaP384' | 'ecdsaP521' | 'rsa4096';

/** Génération d'une clé dans le coffre. */
export interface KeyGenerateRequest {
  name: string;
  kind: KeyKind;
}

/** Déploiement d'une clé du coffre sur un hôte du carnet (KEY-03). */
export interface KeyDeployRequest {
  hostId: EntryId;
  keyId: EntryId;
  /** Uniquement si l'hôte n'a pas d'identité ; sinon `null`. */
  password: string | null;
}

/** Clé d'hôte connue (HKY-04). */
export interface KnownHostEntry {
  /** Position de la ligne : désigne l'entrée à supprimer. */
  line: number;
  hosts: string;
  algorithm: string;
  fingerprint: string;
  revoked: boolean;
}

/** Paramètres d'apparence (`settings.json`). */
export interface AppSettings {
  language: 'system' | 'fr' | 'en';
  theme: 'system' | 'light' | 'dark';
  terminalFontSize: number;
}

/** Réponse de `settings_get`. */
export interface SettingsResponse {
  settings: AppSettings;
  /** Aucun fichier encore écrit : reprendre les anciennes préférences éventuelles. */
  firstRun: boolean;
}

// ---------------------------------------------------------------------- SFTP (étape 4)

/** Identifiant opaque d'un client SFTP, attribué par le cœur. */
export type SftpId = number;
/** Identifiant d'un transfert. */
export type JobId = number;
/** Identifiant opaque d'un dossier local choisi dans la boîte de dialogue du cœur. */
export type LocalRootId = number;
/** Identifiant d'une édition en cours. */
export type EditId = number;

export type EntryKind = 'file' | 'dir' | 'symlink' | 'other';

/** Élément d'un dossier distant. `name` est le nom exact (à ne pas afficher tel quel). */
export interface RemoteEntry {
  name: string;
  /** Nom affichable : caractères de contrôle et invisibles remplacés par le cœur. */
  displayName: string;
  /** Nom modifié pour l'affichage ou impropre à une écriture locale. */
  suspicious: boolean;
  kind: EntryKind;
  size: number;
  modified: number | null;
  permissions: number | null;
  /** Type de la cible d'un lien symbolique, si elle a pu être résolue. */
  linkTarget: EntryKind | null;
}

export interface RemoteListing {
  path: string;
  entries: RemoteEntry[];
  truncated: boolean;
}

export interface LocalEntry {
  name: string;
  kind: EntryKind;
  size: number;
  modified: number | null;
}

export interface LocalListing {
  entries: LocalEntry[];
  truncated: boolean;
}

/** Dossier local choisi : identifiant et nom, jamais le chemin. */
export interface LocalRootRef {
  id: LocalRootId;
  name: string;
}

export type ConflictAction = 'overwrite' | 'skip' | 'rename';

export type DownloadTarget = { kind: 'pick' } | { kind: 'root'; rootId: LocalRootId; path: string };

export type UploadSource =
  | { kind: 'pickFiles' }
  | { kind: 'pickFolder' }
  | { kind: 'root'; rootId: LocalRootId; paths: string[] };

/** Événement d'un client SFTP (transferts et éditions). */
export type SftpEvent =
  | {
      type: 'transferQueued';
      job: JobId;
      direction: 'download' | 'upload';
      label: string;
      count: number;
    }
  | { type: 'transferStarted'; job: JobId }
  | {
      type: 'transferProgress';
      job: JobId;
      bytesDone: number;
      bytesTotal: number;
      filesDone: number;
      filesTotal: number;
    }
  | { type: 'transferConflict'; job: JobId; name: string }
  | {
      type: 'transferFinished';
      job: JobId;
      transferred: number;
      skipped: number;
      ignored: number;
      refused: number;
      failed: number;
      code: string | null;
    }
  | { type: 'editUploaded'; edit: EditId }
  | { type: 'editConflict'; edit: EditId }
  | { type: 'editFailed'; edit: EditId; code: string };
