# Architecture

> Reflète le code à la fin de l'étape 5 (split, snippets, diffusion). L'agent `docs-keeper` la met à jour à chaque étape. Les éléments marqués « à venir » sont prévus mais pas encore dans le code.

## 1. Vue d'ensemble

Younji est une application **Tauri 2**. Elle tourne en deux parties :
- **Le cœur Rust** (processus principal) : il détient les secrets, ouvre les connexions réseau, lit et écrit les fichiers.
- **La WebView** (WebView2 sous Windows, WebKitGTK sous Linux) : elle affiche l'interface React. Elle est considérée comme **non fiable**. Elle demande, le cœur valide et exécute.

```mermaid
flowchart LR
    subgraph WebView["WebView — interface (non fiable)"]
        UI[React + xterm.js]
        IPCJS[src/lib/ipc.ts]
        UI --> IPCJS
    end
    ISO["Iframe d'isolation<br/>isolation/index.js<br/>(liste blanche)"]
    subgraph Core["Cœur Rust (fiable)"]
        ACL[Capabilities / ACL]
        CMD["src-tauri : commandes IPC<br/>SessionManager (+ SftpRegistry), VaultState, SettingsStore"]
        ACL --> CMD
        subgraph Crates["crates/"]
            SSH[core-ssh]
            VAULT[core-vault]
            TERM["core-term (étape 7)"]
            IMP["core-import (étape 9)"]
            RDP["core-rdp (étape 8)"]
        end
        CMD --> SSH & VAULT
        CMD -.-> TERM & IMP & RDP
        SSH -- "clés d'hôte<br/>(trait KnownHostsStorage)" --> CMD
    end
    IPCJS -- "invoke()" --> ISO
    ISO -- "message chiffré (AES-GCM)" --> ACL
    CMD -- "réponse / Channel&lt;T&gt; (flux)" --> IPCJS
    SSH -- "SSH, SFTP" --> SRV[(Serveurs)]
    VAULT -- "vault.younji (chiffré)" --> DISK[(Disque)]
    CMD -- "settings.json (en clair, sans secret)" --> DISK
    SSH -- "fichiers transférés<br/>(dossiers choisis par le cœur)" --> DISK
    RDP -. RDP .-> WIN[(Postes Windows)]
```
Traits pleins : en place. Pointillés : à venir.

**Frontière de confiance** : tout ce qui arrive de la WebView est validé comme une entrée externe (types, bornes, chemins, identifiants). Les secrets ne repassent jamais dans la WebView.

**Barrières, par ordre d'importance** (ADR-015) :
1. **Validation côté Rust** de chaque entrée de commande.
2. **Capabilities** : une commande n'est appelable que si une capability de la fenêtre l'accorde explicitement.
3. **Isolation** : défense en profondeur « au mieux ». Un script de la fenêtre principale peut la contourner en appelant l'IPC directement ; il se heurte alors aux capabilities. On ne compte jamais sur elle pour autoriser ou valider quoi que ce soit.

## 2. Découpage du code

État actuel (étape 5) :

```
.
├── src/                          Interface React
│   ├── components/
│   │   ├── layout/               AppShell (avec bandeau de diffusion), Sidebar (avec bouton « Verrouiller »),
│   │   │                         TabBar (onglets de terminal et SFTP, nombre de panneaux, pastille de
│   │   │                         diffusion), SectionPlaceholder (Tunnels)
│   │   ├── ui/                   Composants de base (button, avec variante destructive ; input, modal, native-select)
│   │   ├── ConfirmDialog.tsx     Confirmation générique (bouton rouge pour une action destructrice)
│   │   ├── SessionChecklist.tsx  Sessions à cocher, avec nom et adresse (diffusion, envoi de snippet)
│   │   └── NameDialog.tsx        Saisie d'un nom (groupe, clé), longueur bornée comme dans le cœur
│   ├── features/
│   │   ├── vault/                Porte du coffre (VaultGate), création avec indicateur de robustesse,
│   │   │                         déverrouillage, verrouillage (lock.ts), lien avec le minuteur du cœur
│   │   │                         (useVaultSession), estimation zxcvbn-ts chargée à la demande (strength.ts)
│   │   ├── hosts/                Carnet d'hôtes : liste par groupe, recherche (filter.ts), formulaire,
│   │   │                         ouverture d'une session depuis le carnet (openHost.ts)
│   │   ├── keys/                 Clés générées (KeyGenerateDialog) ou importées, export de la clé
│   │   │                         publique, déploiement sur un hôte (KeyDeployDialog), identités,
│   │   │                         clés d'hôte connues (HKY-04)
│   │   ├── connect/              Connexion rapide (dont le mode « Questions du serveur (2FA) »), dialogue
│   │   │                         des questions du serveur (AuthPromptDialog), dialogue de clé d'hôte
│   │   │                         inconnue, alerte de clé modifiée
│   │   ├── terminal/             Contrôleur xterm.js par panneau (TerminalSession, avec diffusion de la
│   │   │                         saisie et envoi de texte), vue, recherche, menu contextuel, garde de
│   │   │                         collage, liens, file de saisie, suivi OSC 7 ; panneaux : arbre de
│   │   │                         disposition (layout.ts), découpe et navigation (split.ts), affichage
│   │   │                         récursif et séparateurs (SplitView), onglet découpé (SessionPanel),
│   │   │                         panneau et son en-tête (TerminalPane), barre d'outils (PaneToolbar)
│   │   ├── broadcast/            Diffusion de la saisie : dialogue de choix des cibles (BroadcastDialog),
│   │   │                         bandeau rouge global avec arrêt (BroadcastBanner)
│   │   ├── snippets/             Bibliothèque (SnippetsView, SnippetForm), choix rapide (SnippetPicker),
│   │   │                         envoi avec variables, aperçu et cibles (SnippetRunDialog), variables
│   │   │                         et validation (variables.ts, mêmes règles que le cœur), recherche
│   │   ├── sftp/                 Panneau latéral (SftpSidePanel), onglet double panneau (SftpTabPanel),
│   │   │                         listes distante et locale (RemoteBrowser, LocalBrowser), file de
│   │   │                         transferts (TransferList), dialogues de conflit et de chmod,
│   │   │                         ouverture du client (useSftpClient), OSC 7 et intégration au prompt
│   │   │                         (osc7.ts), glisser-déposer interne (dnd.ts), section SFTP
│   │   ├── shortcuts/            Raccourcis clavier (Ctrl+Maj, Ctrl+Alt+flèches), blocage du rechargement
│   │   │                         hors terminal
│   │   └── settings/             Paramètres (apparence, police, sécurité, raccourcis), changement
│   │                             du mot de passe maître
│   ├── i18n/                     Initialisation i18next, détection de la langue
│   ├── locales/{fr,en}.json      Traductions (embarquées, aucun chargement réseau)
│   ├── stores/
│   │   ├── vault.ts              État du coffre et contenu affichable (sans secret), vidé au verrouillage
│   │   ├── sessions.ts           Onglets et panneaux : terminal ou SFTP (kind, parentTabId), onglet d'un
│   │   │                         panneau secondaire (paneOf), disposition par onglet (layouts), panneau
│   │   │                         actif par onglet (focusedPane), état, cible sans secret, clé d'hôte en
│   │   │                         attente, dossier OSC 7, panneau SFTP ouvert
│   │   ├── broadcast.ts          Cibles de la diffusion (jamais persistées), dialogue d'activation
│   │   ├── snippetRun.ts         Dialogues d'envoi de snippet ouverts (choix, envoi)
│   │   ├── sftp.ts               Transferts et éditions affichés, alimentés par les événements du cœur
│   │   ├── settings.ts           Préférences (langue, thème, police), persistées par le cœur (voir §4)
│   │   └── ui.ts                 Section active de la barre latérale (non persistée)
│   ├── lib/
│   │   ├── ipc.ts                Seul point d'appel au backend (fonctions typées)
│   │   ├── sessionAddress.ts     Adresse affichée d'une session (utilisateur@hôte:port)
│   │   ├── types.ts              Types partagés (miroir des structures Rust)
│   │   └── utils.ts              Utilitaire de classes CSS (cn)
│   ├── test/setup.ts             Initialisation des tests Vitest
│   └── index.css                 Tailwind, thèmes, masquage de l'iframe d'isolation
├── isolation/                    Application d'isolation Tauri (index.html + index.js, sans dépendance)
├── src-tauri/                    Application Tauri (fine)
│   ├── src/
│   │   ├── main.rs               Point d'entrée
│   │   ├── lib.rs                Builder Tauri, fenêtre, plugin dialog (Rust seul), minuteur de
│   │   │                         verrouillage, verrouillage au rechargement, enregistrement des commandes
│   │   ├── state.rs              SessionManager : sessions ouvertes (64 max), fichiers de clé par identifiant,
│   │   │                         registre SFTP fermé avec les sessions
│   │   ├── sftp_state.rs         SftpRegistry : clients SFTP (64 max), dossiers locaux choisis (32 max),
│   │   │                         éditions (16 max)
│   │   ├── editor.rs             Éditeur de texte imposé (Bloc-notes, liste fixe sous Linux, open -e)
│   │   ├── vault_state.rs        VaultState : coffre verrouillé ou non, tentatives et délai progressif,
│   │   │                         activité (double horloge), Channel de vault_watch ; VaultKnownHosts
│   │   ├── settings.rs           SettingsStore : settings.json validé, écriture atomique
│   │   ├── commands/app.rs       Commande app_info
│   │   ├── commands/ssh.rs       Commandes ssh_* : validation, appel à core-ssh, deux Channel par session ;
│   │   │                         ssh_connect_host (identité lue dans le coffre), ssh_auth_answer (2FA),
│   │   │                         key_deploy (déploiement de clé)
│   │   ├── commands/vault.rs     Commandes vault_* (cycle de vie du coffre) et lock_everything
│   │   ├── commands/book.rs      Carnet : vault_snapshot, host_*, group_*, identity_*, key_* (dont
│   │   │                         key_generate et key_export_public), known_host_*
│   │   ├── commands/snippets.rs  snippet_save, snippet_delete ; SnippetDto (variables calculées par le cœur)
│   │   ├── commands/settings.rs  settings_get, settings_set
│   │   ├── commands/sftp.rs      Commandes sftp_* et local_* : validation, RootPolicy, boîtes de dialogue,
│   │   │                         Channel d'événements SFTP
│   │   ├── commands/url.rs       open_external_url : revalidation http(s), ouverture via la crate open
│   │   ├── error.rs              AppError { code } sérialisable, conversion depuis SshError et VaultError
│   │   ├── logging.rs            Journaux rotatifs (tracing)
│   │   └── navigation.rs         Garde de navigation de la fenêtre
│   ├── build.rs                  Déclaration des commandes applicatives (AppManifest)
│   ├── capabilities/main.json    Permissions de la fenêtre principale (commandes applicatives seules)
│   ├── permissions/autogenerated/ Permissions générées par build.rs (versionnées)
│   └── tauri.conf.json           Config, CSP, isolation
├── crates/
│   ├── core-ssh/                 Client SSH, sans dépendance à Tauri ni à core-vault
│   │   ├── src/
│   │   │   ├── target.rs         Cible (hôte, port, utilisateur) et taille du terminal, validées
│   │   │   ├── auth.rs           Moyens d'authentification (Credentials : mot de passe, clé, clé puis
│   │   │   │                     mot de passe, Interactive) ; import (ré-encodage sans passphrase)
│   │   │   ├── prompt.rs         Questions keyboard-interactive : nettoyage, bornes, reconnaissance
│   │   │   │                     stricte de la question « mot de passe », réponses (PromptAnswers)
│   │   │   ├── keygen.rs         Génération de clés (Ed25519, ECDSA P-256/384/521, RSA 4096), aléa du système
│   │   │   ├── deploy.rs         Déploiement dans authorized_keys : commande constante, ligne de clé
│   │   │   │                     validée (AuthorizedKeyLine) envoyée par l'entrée standard
│   │   │   ├── config.rs         Algorithmes autorisés (liste explicite) et délais
│   │   │   ├── known_hosts.rs    Vérification, ajout, liste et suppression des clés d'hôte (format
│   │   │   │                     OpenSSH), stockage abstrait (trait KnownHostsStorage, FileStorage)
│   │   │   ├── session.rs        Tâche de session (SessionHandle) : connexion, TOFU, authentification en
│   │   │   │                     plusieurs méthodes, PTY, E/S, contrôle de flux, ouverture SFTP
│   │   │   │                     (open_sftp) ; ou déploiement de clé (spawn_deploy) sur le même chemin
│   │   │   ├── sftp/             SFTP (SFTP-01 à 05), sans Tauri :
│   │   │   │   ├── mod.rs        Trait RemoteFs (accès distant), types d'éléments et d'attributs
│   │   │   │   ├── client.rs     SftpClient : deux canaux (RawSftpSession pour les métadonnées et les
│   │   │   │   │                 listages bornés, SftpSession pour le contenu), erreurs traduites
│   │   │   │   ├── names.rs      RemotePath (chemin distant validé), check_local_name, nom affichable
│   │   │   │   ├── local.rs      LocalRoot (dossier local canonicalisé, confinement), RootPolicy
│   │   │   │   ├── ops.rs        Listage affichable, suppression récursive, mkdir, renommage, chmod
│   │   │   │   ├── transfer.rs   Envoi et téléchargement en flux, conflits, écriture par fichier
│   │   │   │   │                 temporaire puis renommage, Mark-of-the-Web
│   │   │   │   ├── queue.rs      TransferQueue : file séquentielle par client, annulation, questions
│   │   │   │   ├── edit.rs       EditSession : copie privée, surveillance, renvoi, conflit, nettoyage
│   │   │   │   └── memfs.rs, tests.rs  Faux système de fichiers en mémoire et tests (cfg(test))
│   │   │   └── error.rs          SshError et ses codes stables ssh.*
│   │   └── tests/                local_server.rs (serveur russh en mémoire), openssh.rs (#[ignore]),
│   │                             fixtures/test_host_*.pub (clés publiques uniquement)
│   └── core-vault/               Coffre chiffré, sans dépendance à Tauri ni à core-ssh
│       └── src/
│           ├── format.rs         Format de fichier v1 (en-tête, bornes, lecture sans allocation excessive)
│           ├── crypto.rs         Argon2id → KEK, enveloppe de la DEK, XChaCha20-Poly1305, aléa du système
│           ├── store.rs          Vault : création, déverrouillage, sauvegarde atomique + .bak,
│           │                     changement de mot de passe, permissions
│           ├── model.rs          Hôtes, groupes, identités, clés, clés d'hôte, réglages, snippets ;
│           │                     validation (revérifiée au chargement) et intégrité référentielle
│           ├── policy.rs         Politique du mot de passe maître (12 caractères, zxcvbn ≥ 3)
│           └── error.rs          VaultError et ses codes stables vault.*
├── tests/
│   ├── security/                 Garde-fous de configuration (CSP, capabilities, isolation)
│   └── docker/sshd/              Serveur OpenSSH jetable (avec openssh-sftp-server) pour les tests d'intégration
├── scripts/
│   ├── vitest.mjs                Lanceur Vitest (normalise la lettre de lecteur sous Windows)
│   └── integration.mjs           Lanceur des tests d'intégration (charge .env.test s'il existe)
├── Cargo.toml                    Workspace (src-tauri, crates/core-ssh, crates/core-vault), profils
├── deny.toml                     Règles cargo-deny (avis, licences, sources)
└── rust-toolchain.toml           Toolchain stable + rustfmt + clippy
```

À venir : `core-term` (étape 7), `core-rdp` (étape 8) et `core-import` (étape 9) rejoignent le workspace à leur étape ; `core-ssh` s'enrichira des tunnels (étape 6). Côté interface, `features/` accueillera `tunnels/`.

### Dépendances principales ajoutées à l'étape 1
| Dépendance | Rôle | Note |
|---|---|---|
| `russh` 0.65.2 (`ring`, `rsa`) | Client SSH | Sans aws-lc-rs ni compression (ADR-019) |
| `tokio`, `tokio-util` | Tâches de session, annulation | |
| `secrecy`, `zeroize` | Secrets en mémoire | |
| `tempfile` | Écritures atomiques | |
| `tauri-plugin-dialog` | Choix de la clé privée | Appelé depuis Rust seulement, aucune permission pour la WebView (ADR-020) |
| `open` | Ouverture des liens dans le navigateur | À la place de `tauri-plugin-opener` (ADR-022) |
| `@xterm/xterm` 5.5 + addons fit, search, web-links, unicode11, webgl | Terminal | WebGL avec repli sur le rendu DOM (ADR-018) ; version 5.5 et non 6, incompatible avec `freezePrototype` (ADR-025) |

### Dépendances ajoutées à l'étape 2
| Dépendance | Rôle | Note |
|---|---|---|
| `argon2` 0.6 (RustCrypto) | Dérivation Argon2id de la KEK | Sans fonctionnalités par défaut, `alloc` + `zeroize` |
| `chacha20poly1305` 0.11 (RustCrypto) | XChaCha20-Poly1305 (DEK et données) | `alloc` + `zeroize` ; l'aléa est fourni par `getrandom` |
| `getrandom` 0.4 | Aléa du système d'exploitation (équivalent `OsRng`) | Appel direct à `getrandom::fill` |
| `uuid` 1.29 (`v4`, `serde`) | Identifiants opaques des entrées du coffre | Un identifiant mal formé est refusé à la désérialisation |
| `zxcvbn` 3.1 | Contrôle de robustesse du mot de passe maître, qui fait foi (ADR-029) | Sans la fonctionnalité `builder` |
| `@zxcvbn-ts/core` 4.2.0, `@zxcvbn-ts/language-common` 4.1.3, `@zxcvbn-ts/language-en` 4.1.1 | Indicateur de robustesse en direct (ADR-029) | Versions épinglées, chargées à la demande (fichier séparé) |

Toutes sous licence MIT ou MIT/Apache-2.0.

### Dépendance ajoutée à l'étape 3
| Dépendance | Rôle | Note |
|---|---|---|
| `getrandom` 0.4.3 (`sys_rng`), dans `core-ssh` | Générateur du système (`SysRng`, équivalent d'`OsRng`) pour la génération de clés (ADR-032) | MIT OR Apache-2.0 ; déjà dans l'arbre via `core-vault`. `sys_rng` réexporte `rand_core` 0.10, déjà présent via `ssh-key` |

Aucune dépendance npm ajoutée.

### Dépendance ajoutée à l'étape 4
| Dépendance | Rôle | Note |
|---|---|---|
| `russh-sftp` 3.0.1, dans `core-ssh` | Protocole SFTP v3 côté client, sur un canal russh | Apache-2.0, prévue par ADR-002. Son `SftpSession::read_dir` n'étant pas borné, les listages passent par `RawSftpSession` (ADR-034). Tire `chrono`, non maintenue : exception cargo-deny RUSTSEC-2026-0543, code jamais appelé côté client |
| `tokio` : fonctionnalités `fs` et `io-util` (déjà présent) | Lecture et écriture locales en flux | MIT |

Aucune dépendance npm : le glisser-déposer utilise l'API HTML5.

**Dépendances entre crates** : `core-ssh` et `core-vault` ne dépendent pas l'une de l'autre (ADR-004). C'est `src-tauri` qui fait le lien :
- il lit une identité dans le coffre (mot de passe, clé privée) et la passe à `core-ssh` sous forme de `Credentials` ;
- il stocke dans le coffre une clé générée par `core-ssh` (`generate_key`), comme une clé importée ;
- pour un déploiement, il re-dérive la ligne de clé publique de la clé privée du coffre (`AuthorizedKeyLine::from_private`) ;
- il implémente le trait `KnownHostsStorage` de `core-ssh` pour le coffre (`VaultKnownHosts`, ADR-027).

Les futures crates `core-term`, `core-import` et `core-rdp` suivront la même règle.

**Règles de compilation communes (workspace)** : edition 2024, `unsafe_code = "forbid"`, et `unwrap`, `expect`, `panic`, `dbg!`, `todo!` interdits par clippy hors tests. En profil `dev`, `argon2`, `blake2`, `rsa`, `crypto-bigint` et `crypto-primes` sont compilés en `opt-level = 3` : sans cela, une dérivation Argon2id (64 Mio, 3 passes) ou la génération d'une clé RSA 4096 rendraient les tests et `npm run tauri dev` très lents. Sans effet sur la compilation de production.

## 3. Communication interface ↔ cœur

| Mécanisme | Usage | Exemple |
|---|---|---|
| `invoke()` (commande) | Action ponctuelle avec un résultat | `app_info`, `ssh_connect`, `vault_unlock`, `host_save` |
| `Channel<T>` passé en argument | Flux ou notification ciblée | Sortie et état d'une session SSH ; verrouillage du coffre (`vault_watch`) ; transferts et éditions SFTP (`sftp_open`, `sftp_edit`) ; plus tard état d'un tunnel |

Aucun événement global de Tauri n'est utilisé : la permission `core:event:*` n'est pas accordée à la fenêtre (ADR-028).

### Commandes existantes

Session SSH et utilitaires :

| Commande | Entrée | Sortie | Permission |
|---|---|---|---|
| `app_info` | aucune | `AppInfo { name, version }` | `allow-app-info` |
| `ssh_connect` | `request { host, port, username, auth, cols, rows }`, `onEvent: Channel<SessionEventDto>`, `onOutput: Channel<octets>` | `SessionId` (entier opaque) | `allow-ssh-connect` |
| `ssh_connect_host` | `request { hostId, cols, rows }`, `onEvent`, `onOutput` | `SessionId` | `allow-ssh-connect-host` |
| `ssh_write` | `sessionId`, `data`, `binary` (octets 0–255 si vrai, sinon UTF-8) ; 64 Kio max | aucune | `allow-ssh-write` |
| `ssh_resize` | `sessionId`, `cols` (1–1000), `rows` (1–500) | aucune | `allow-ssh-resize` |
| `ssh_ack` | `sessionId`, `bytes` (octets affichés) | aucune | `allow-ssh-ack` |
| `ssh_host_key_decision` | `sessionId`, `accept` | aucune | `allow-ssh-host-key-decision` |
| `ssh_auth_answer` | `sessionId`, `round` (numéro de la demande), `answers[] \| null` (`null` = annuler) ; une réponse par question, 1 Kio max chacune | aucune ; `ssh.no_pending_prompt` si rien n'attend pour ce tour | `allow-ssh-auth-answer` |
| `ssh_close` | `sessionId` | aucune | `allow-ssh-close` |
| `ssh_pick_key_file` | aucune (boîte de dialogue ouverte par le cœur) | `{ id, name } \| null` : identifiant opaque et nom du fichier, jamais le chemin | `allow-ssh-pick-key-file` |
| `open_external_url` | `url` (`http:`/`https:`, 2048 caractères max, revalidée) | aucune | `allow-open-external-url` |

Coffre :

| Commande | Entrée | Sortie | Permission |
|---|---|---|---|
| `vault_status` | aucune | `"missing" \| "locked" \| "unlocked"` | `allow-vault-status` |
| `vault_create` | `password` (politique vérifiée par le cœur) | aucune ; le coffre reste déverrouillé | `allow-vault-create` |
| `vault_unlock` | `password` | aucune ; tout échec de déchiffrement donne `vault.wrong_password_or_corrupted` | `allow-vault-unlock` |
| `vault_lock` | aucune | aucune ; ferme aussi toutes les sessions | `allow-vault-lock` |
| `vault_change_password` | `current`, `new` | aucune | `allow-vault-change-password` |
| `vault_touch` | aucune (signale une activité) | aucune | `allow-vault-touch` |
| `vault_watch` | `onEvent: Channel<VaultEventDto>` | aucune | `allow-vault-watch` |
| `vault_set_auto_lock` | `minutes` (1, 5, 10, 15, 30 ou 60) | aucune | `allow-vault-set-auto-lock` |
| `vault_snapshot` | aucune | `{ hosts[], groups[], identities[], keys[], snippets[], autoLockMinutes }` (voir ci-dessous) | `allow-vault-snapshot` |

Carnet, clés et paramètres :

| Commande | Entrée | Sortie | Permission |
|---|---|---|---|
| `host_save` | `request { id \| null, name, address, port, username, identityId, groupId, tags[], notes }` | `EntryId` | `allow-host-save` |
| `host_delete` | `id` | aucune | `allow-host-delete` |
| `group_save` | `request { id \| null, name }` | `EntryId` | `allow-group-save` |
| `group_delete` | `id` (ses hôtes passent « sans groupe ») | aucune | `allow-group-delete` |
| `identity_save` | `request { id \| null, name, username, password, keyId }` | `EntryId` | `allow-identity-save` |
| `identity_delete` | `id` (refusé si un hôte l'utilise) | aucune | `allow-identity-delete` |
| `key_import` | `request { keyFileId, name, passphrase \| null }` (`keyFileId` issu de `ssh_pick_key_file`) | `{ id, name, algorithm, fingerprint, publicKey }` | `allow-key-import` |
| `key_generate` | `request { name, kind }`, `kind` = `ed25519`, `ecdsaP256`, `ecdsaP384`, `ecdsaP521` ou `rsa4096` | même forme que `key_import` | `allow-key-generate` |
| `key_export_public` | `id` (boîte de dialogue d'enregistrement ouverte par le cœur, extension `.pub` imposée) | `true`, ou `false` si l'utilisateur annule ; le chemin ne remonte pas | `allow-key-export-public` |
| `key_deploy` | `request { hostId, keyId, password \| null }` (mot de passe seulement si l'hôte n'a pas d'identité), `onEvent: Channel<SessionEventDto>` | `SessionId` (même pilotage qu'une session : clé d'hôte, questions, fermeture) | `allow-key-deploy` |
| `key_rename` | `id`, `name` | aucune | `allow-key-rename` |
| `key_delete` | `id` (refusé si une identité l'utilise) | aucune | `allow-key-delete` |
| `snippet_save` | `request { id \| null, name, content, description }` (validé par `core-vault`, ADR-037) | `EntryId` | `allow-snippet-save` |
| `snippet_delete` | `id` | aucune | `allow-snippet-delete` |
| `known_host_list` | aucune | `[{ line, hosts, algorithm, fingerprint, revoked }]` | `allow-known-host-list` |
| `known_host_delete` | `entry` (une entrée telle que reçue de `known_host_list`) | nombre de lignes retirées (0 ou 1) | `allow-known-host-delete` |
| `settings_get` | aucune (lisible avant le déverrouillage) | `{ settings { language, theme, terminalFontSize }, firstRun }` | `allow-settings-get` |
| `settings_set` | `settings { language, theme, terminalFontSize }` | aucune | `allow-settings-set` |

SFTP (étape 4, ADR-034). Tous les chemins distants sont revalidés par `RemotePath::parse` (absolu, 4 Kio au plus, sans NUL). Aucun chemin local absolu n'est accepté : un dossier local est désigné par `rootId`, plus un chemin relatif confiné.

| Commande | Entrée | Sortie | Permission |
|---|---|---|---|
| `sftp_open` | `sessionId` (terminal connecté), `onEvent: Channel<SftpEventDto>` | `SftpId` (entier opaque) ; `vault.locked` si le coffre est verrouillé | `allow-sftp-open` |
| `sftp_close` | `sftpId` | aucune ; transferts annulés, éditions arrêtées | `allow-sftp-close` |
| `sftp_home` | `sftpId` | chemin du dossier personnel distant | `allow-sftp-home` |
| `sftp_list` | `sftpId`, `path` | `{ path, entries[{ name, displayName, suspicious, kind, size, modified, permissions, linkTarget }], truncated }` (10 000 entrées au plus) | `allow-sftp-list` |
| `sftp_mkdir` | `sftpId`, `path` | aucune | `allow-sftp-mkdir` |
| `sftp_rename` | `sftpId`, `from`, `to` (n'écrase jamais) | aucune | `allow-sftp-rename` |
| `sftp_remove` | `sftpId`, `path` (récursif pour un dossier, liens supprimés sans être suivis) | aucune | `allow-sftp-remove` |
| `sftp_chmod` | `sftpId`, `path`, `mode` (0 à 0o7777) | aucune | `allow-sftp-chmod` |
| `sftp_download` | `sftpId`, `paths[]` (1 à 1 000), `target` = `{ kind: "pick" }` (boîte de dialogue du cœur) ou `{ kind: "root", rootId, path }` | `JobId`, ou `null` si la boîte de dialogue est annulée | `allow-sftp-download` |
| `sftp_upload` | `sftpId`, `source` = `{ kind: "pickFiles" }`, `{ kind: "pickFolder" }` ou `{ kind: "root", rootId, paths[] }`, `remoteDir` | `JobId` ou `null` | `allow-sftp-upload` |
| `sftp_transfer_cancel` | `sftpId`, `job` | aucune | `allow-sftp-transfer-cancel` |
| `sftp_conflict_decision` | `sftpId`, `job`, `action` (`overwrite`, `skip`, `rename`), `applyToAll` | aucune ; `ssh.transfer_not_found` si aucune question n'attend | `allow-sftp-conflict-decision` |
| `sftp_edit` | `sftpId`, `path`, `onEvent: Channel<SftpEventDto>` | `EditId` ; la copie est ouverte dans l'éditeur imposé | `allow-sftp-edit` |
| `sftp_edit_close` | `editId` | aucune ; copie locale supprimée | `allow-sftp-edit-close` |
| `sftp_edit_force` | `editId` | aucune ; renvoi malgré un conflit (écrase le fichier distant) | `allow-sftp-edit-force` |
| `local_pick_root` | aucune (boîte de dialogue ouverte par le cœur, `RootPolicy` appliquée) | `{ id, name } \| null` : identifiant et nom du dossier, jamais le chemin | `allow-local-pick-root` |
| `local_list` | `rootId`, `path` (relatif, confiné) | `{ entries[{ name, kind, size, modified }], truncated }` | `allow-local-list` |

Aucune commande n'accepte de politique de conflit : tout transfert démarre en « demander ». Les structures de requête refusent les champs inconnus, si bien qu'un chemin absolu ne peut pas être glissé dans `target` ou `source`.

**Secrets en entrée** : seuls `ssh_connect` (`auth`), `ssh_auth_answer` (`answers`), `vault_create`, `vault_unlock`, `vault_change_password`, `identity_save` (`password`), `key_import` (`passphrase`) et `key_deploy` (`password`) en transportent, une seule fois. Côté Rust, ils sont placés dans un `SecretString` dès la désérialisation. **Aucune commande n'en renvoie** :
- `auth` vaut `{ kind: "password", password }`, `{ kind: "key", keyFileId, passphrase | null }` ou `{ kind: "interactive" }` (aucun secret d'avance : le serveur pose ses questions, ADR-031) ;
- le `password` d'une identité vaut `{ action: "keep" }`, `{ action: "clear" }` ou `{ action: "set", value }` ;
- une identité renvoyée par `vault_snapshot` est `{ id, name, username, hasPassword, keyId }` : la présence d'un mot de passe, jamais sa valeur ;
- un snippet est renvoyé comme `{ id, name, content, description, variables[] }` : ce n'est pas un secret (ADR-037), et ses variables sont calculées par le cœur ;
- une clé, importée ou générée, est renvoyée avec sa seule partie publique (sans commentaire) et son empreinte ; un hôte avec ses champs descriptifs. Aucune commande n'exporte une clé privée (ADR-032).

**Validation** : les structures Rust refusent les champs inconnus (`deny_unknown_fields`) ; un `EntryId` doit être un UUID ; adresse, port et utilisateur d'un hôte passent par les mêmes règles que la connexion SSH ; noms, étiquettes, notes et nombres d'entrées sont bornés dans `core-vault` (`model.rs`). Toute commande du carnet sur un coffre verrouillé renvoie `vault.locked`.

### Événements

**Événements de session** (`SessionEventDto`, JSON sur `onEvent`, champ `type`) :

| Type | Contenu | Signification |
|---|---|---|
| `hostKeyUnknown` | `key { algorithm, fingerprint }` | Première connexion : attendre `ssh_host_key_decision` (120 s) |
| `hostKeyChanged` | `key`, `known[]`, `hostPattern` | Clé différente de celle enregistrée : connexion refusée. L'alerte renvoie vers Clés > Clés d'hôte connues pour débloquer (HKY-04) |
| `authenticating` | aucun | Clé d'hôte acceptée, authentification en cours ; émis à nouveau après chaque réponse aux questions du serveur |
| `authPrompt` | `round`, `name`, `instructions`, `prompts[{ text, echo }]` | Le serveur pose des questions (keyboard-interactive) : attendre `ssh_auth_answer` avec le même `round` (2 min). Texte nettoyé et borné par le cœur, affiché en texte brut |
| `keyDeployed` | `added` (`true` : ajoutée ; `false` : déjà présente) | Déploiement réussi (`key_deploy`) ; `disconnected` (`ssh.session_ended`) suit aussitôt |
| `connected` | aucun | Shell ouvert |
| `disconnected` | `code` (`ssh.*`) | Fin de session, toujours le dernier événement |

**Événements SFTP** (`SftpEventDto`, JSON sur le `onEvent` de `sftp_open` pour les transferts, de `sftp_edit` pour les éditions, champ `type`). Ils ne contiennent aucun chemin local, seulement des noms.

| Type | Contenu | Signification |
|---|---|---|
| `transferQueued` | `job`, `direction` (`download`, `upload`), `label` (nom du premier élément), `count` | Transfert ajouté à la file du client (64 en attente au plus) |
| `transferStarted` | `job` | Début de la copie (un transfert à la fois par client) |
| `transferProgress` | `job`, `bytesDone`, `bytesTotal`, `filesDone`, `filesTotal` | Progression, 10 messages par seconde au plus |
| `transferConflict` | `job`, `name` | La destination existe : attendre `sftp_conflict_decision` (10 min, puis l'élément et les conflits suivants sont ignorés) |
| `transferFinished` | `job`, `transferred`, `skipped`, `ignored` (liens, fichiers spéciaux), `refused` (noms dangereux), `failed`, `code` | Bilan ; `code` vaut `null` ou un code `ssh.*` (`ssh.transfer_cancelled` après annulation) |
| `editUploaded` | `edit` | Enregistrement renvoyé au serveur |
| `editConflict` | `edit` | Le fichier distant a changé : renvoi suspendu jusqu'à `sftp_edit_force` ou `sftp_edit_close` |
| `editFailed` | `edit`, `code` | Échec du renvoi |

**Événement du coffre** (`VaultEventDto`, JSON sur le `Channel` de `vault_watch`) : `{ type: "locked", reason }`, avec `reason` = `manual`, `idle` ou `reload`. L'interface s'abonne une fois au démarrage ; à la réception, elle détruit les terminaux et affiche l'écran de déverrouillage avec la cause.

La sortie du terminal passe sur `onOutput` en **octets bruts**, par lots de 8 ms ou 32 Kio. Les messages de plus de 1 Kio sont récupérés par la commande interne `plugin:__TAURI_CHANNEL__|fetch`, que l'isolation laisse passer (ADR-024).

**Contrôle de flux** (ADR-024) : l'interface acquitte par `ssh_ack` les octets affichés par xterm.js. Au-delà de 2 Mio non acquittés, la session arrête de lire le canal SSH ; le serveur est alors freiné par la fenêtre SSH. La saisie part par une file côté interface (`inputQueue.ts`) : un seul `ssh_write` en cours, pour que les frappes ne soient jamais réordonnées.

**Cycle de vie d'une session** : chaque session vit dans une tâche tokio annulable (`SessionHandle`), enregistrée dans `SessionManager` (64 au plus). Un déploiement de clé (`SessionHandle::spawn_deploy`) est une session sans terminal : il compte dans cette limite, se pilote par les mêmes commandes et se ferme au verrouillage (ADR-033). Elle se retire d'elle-même à l'événement `disconnected`. Tout verrouillage du coffre ferme toutes les sessions (voir ci-dessous).

**Cycle de vie d'un client SFTP** (ADR-034) : il vit sur la connexion d'une session de terminal et est enregistré dans le `SftpRegistry` du `SessionManager`. La fin de la session (fermeture, coupure, `disconnected`) ferme ses clients SFTP, annule leurs transferts et arrête leurs éditions (copies locales supprimées). Un verrouillage ou un rechargement ferme tout et oublie en plus les dossiers locaux choisis.

### Coffre : états et verrouillage

```mermaid
stateDiagram-v2
    [*] --> missing: aucun vault.younji
    [*] --> locked: vault.younji présent
    missing --> unlocked: vault_create (politique vérifiée par le cœur)
    locked --> unlocked: vault_unlock (Argon2id, délai progressif)
    unlocked --> locked: vault_lock, inactivité ou rechargement
```

- **Toute l'application est derrière le déverrouillage** (ADR-028) : `VaultGate` n'affiche le carnet, les terminaux et la connexion rapide qu'à l'état `unlocked`. `ssh_connect` et `ssh_connect_host` refusent de se connecter sur un coffre verrouillé (`vault.locked`), puisque les clés d'hôte connues sont dans le coffre (ADR-027).
- **Création** : refusée si un coffre existe déjà (`vault.already_exists`) ou si seule la copie `.bak` existe (`vault.backup_only`) ; dans ce cas `vault_status` renvoie `missing` et l'utilisateur restaure la copie en la renommant. Un ancien fichier `known_hosts` en clair (étape 1) est repris dans le coffre, puis supprimé.
- **Déverrouillage** : tentatives sérialisées dans `VaultState` ; après 3 échecs consécutifs, chaque tentative attend 1 s, 2 s, 4 s… jusqu'à 30 s. La vérification du mot de passe actuel dans `vault_change_password` partage ce compteur.
- **Verrouillage** (`lock_everything`, ADR-026) : le coffre est verrouillé d'abord (DEK et secrets effacés de la mémoire), puis toutes les sessions sont fermées, avec leurs clients SFTP, transferts et éditions ; les dossiers locaux choisis sont oubliés. Une session enregistrée pendant ce temps revérifie l'état du coffre et se referme aussitôt. Déclencheurs : `vault_lock` (bouton de la barre latérale, Ctrl+Maj+L), minuteur d'inactivité, rechargement de la page (`PageLoadEvent::Started`).
- **Minuteur d'inactivité** (`lib.rs`, `vault_state.rs`) : tâche tokio qui vérifie toutes les 5 s. L'interface signale l'activité au clavier, à la souris ou à la molette par `vault_touch`, au plus toutes les 15 s. Le délai (dans le coffre, 15 min par défaut) est comparé à deux horloges, monotone et murale : la mise en veille compte, et une horloge murale qui recule n'empêche pas le verrouillage.
- **Changement de mot de passe** : nouveau sel, nouvelle DEK, rechiffrement complet ; le `.bak` est ensuite recopié depuis le nouveau fichier, ou supprimé si la copie échoue (`SECURITE.md` §3.6).

### Trajet d'un appel IPC

```mermaid
sequenceDiagram
    participant UI as Composant React
    participant IPC as src/lib/ipc.ts
    participant ISO as Iframe d'isolation
    participant ACL as Capabilities (cœur)
    participant CMD as Commande Rust
    UI->>IPC: appInfo()
    IPC->>ISO: invoke("app_info")
    alt commande dans la liste blanche
        ISO->>ACL: message chiffré (AES-GCM)
        alt permission accordée à la fenêtre
            ACL->>CMD: exécution (entrées validées côté Rust)
            CMD-->>IPC: résultat ou AppError { code }
        else permission absente
            ACL-->>IPC: refus
        end
    else commande hors liste
        ISO->>ACL: younji_isolation_denied, sans charge utile
        ACL-->>IPC: erreur (commande inexistante)
    end
    IPC-->>UI: résultat ou { code } normalisé
```

Un appel qui contourne l'iframe (appel direct à `http://ipc.localhost/<commande>`) arrive directement aux capabilities : c'est pourquoi elles restent la barrière de référence.

### Ajouter une commande
Une commande doit être déclarée à trois endroits, sinon `tests/security/tauri-config.test.ts` échoue :
1. `src-tauri/build.rs` (`AppManifest::commands`), qui génère `allow-<commande>` ;
2. la liste blanche `ALLOWED_COMMANDS` de `isolation/index.js` ;
3. une capability (`src-tauri/capabilities/`) et la liste de permissions attendue du test.

Puis l'enregistrer dans `generate_handler!` (`lib.rs`) et l'exposer par une fonction typée dans `src/lib/ipc.ts` (types dans `src/lib/types.ts`). Les composants n'appellent jamais `invoke` directement (règle ESLint).

Les commandes internes de Tauri ont leur propre liste dans l'isolation (`INTERNAL_COMMANDS`), limitée à `plugin:__TAURI_CHANNEL__|fetch` (ADR-024). Un test vérifie qu'elle ne contient rien d'autre.

### Connexion SSH avec vérification de la clé d'hôte

```mermaid
sequenceDiagram
    participant UI as Interface
    participant C as src-tauri
    participant V as Coffre (VaultState)
    participant S as core-ssh
    participant SRV as Serveur SSH
    alt connexion rapide
        UI->>C: ssh_connect(request, onEvent, onOutput)
        C->>C: coffre déverrouillé ? validation (cible, taille, secret), clé lue via son identifiant
    else hôte du carnet
        UI->>C: ssh_connect_host({ hostId, cols, rows }, onEvent, onOutput)
        C->>V: hôte, identité (mot de passe, clé) lus dans le coffre
    end
    C->>S: SessionHandle::spawn(...)
    C->>V: coffre toujours déverrouillé ? sinon session refermée (vault.locked)
    C-->>UI: SessionId
    S->>SRV: TCP + échange de clés
    SRV-->>S: clé d'hôte (signature vérifiée par russh)
    S->>V: check(hôte, port, clé) via KnownHostsStorage
    alt clé enregistrée pour cet hôte
        S-->>UI: authenticating
    else hôte jamais vu
        S-->>UI: hostKeyUnknown(algorithme, SHA256)
        UI->>C: ssh_host_key_decision(accept)
        C->>S: décision (120 s max)
        S->>V: add (si acceptée, coffre rechiffré et réécrit)
        S-->>UI: authenticating (sinon disconnected ssh.host_key_rejected)
    else hôte connu, clé absente de la liste (même d'un autre type)
        S-->>UI: hostKeyChanged, puis disconnected ssh.host_key_changed
    else clé marquée @revoked
        S-->>UI: disconnected ssh.host_key_revoked
    end
    S->>SRV: authentification (voir le schéma suivant)
    S->>SRV: PTY xterm-256color + shell
    S-->>UI: connected
    loop session
        SRV-->>S: données
        S-->>UI: onOutput (octets bruts, par lots)
        UI->>C: ssh_ack / ssh_write / ssh_resize
    end
    S-->>UI: disconnected(code)
```

Délais (`config.rs`) : connexion TCP 15 s, poignée de main 20 s, décision sur la clé d'hôte 120 s, chaque échange réseau de l'authentification et l'ouverture du shell 30 s, temps réseau cumulé de l'authentification 60 s (hors saisie), réponse aux questions du serveur 2 min par demande, keepalive toutes les 15 s, connexion déclarée perdue après 3 keepalives sans réponse. Les bannières du serveur sont affichées comme texte brut, nettoyées de toute séquence de contrôle (8 Kio par bannière, 64 Kio par session).

### Authentification en plusieurs méthodes et 2FA (ADR-031)

```mermaid
sequenceDiagram
    participant UI as Interface
    participant C as src-tauri
    participant S as core-ssh
    participant SRV as Serveur SSH
    S->>SRV: méthode none
    SRV-->>S: méthodes acceptées
    loop selon les méthodes restantes (succès partiels compris), chaque moyen une fois
        alt publickey proposée et clé disponible
            S->>SRV: clé (RSA en SHA-2)
        else password proposée et mot de passe disponible
            S->>SRV: mot de passe
        else keyboard-interactive proposée (une fois)
            loop 16 demandes au plus
                SRV-->>S: demande (8 questions au plus)
                alt question unique sans écho « Password » / « Mot de passe », mot de passe enregistré inutilisé
                    S->>SRV: mot de passe enregistré (une fois)
                else autres questions
                    S-->>UI: authPrompt(round, questions nettoyées)
                    UI->>C: ssh_auth_answer(sessionId, round, réponses ou null)
                    C->>S: answer_prompts (bon tour, bon nombre de réponses)
                    S-->>UI: authenticating
                    S->>SRV: réponses
                end
            end
        end
        SRV-->>S: succès, succès partiel ou échec (méthodes restantes)
    end
```

Aucun moyen restant compatible avec ce que propose le serveur : `ssh.auth_failed`. Annulation : `ssh.auth_cancelled` ; délai de saisie dépassé : `ssh.auth_prompt_timeout` ; demande hors bornes : `ssh.auth_prompt_invalid`. Fermer l'onglet ou verrouiller le coffre interrompt l'authentification.

### Déploiement d'une clé (KEY-03, ADR-033)

`key_deploy` désigne un hôte du carnet et une clé du coffre. Le cœur prend l'identité de l'hôte, sinon le mot de passe saisi pour ce seul déploiement, sinon `Credentials::Interactive` ; il re-dérive la ligne `algorithme base64` de la clé privée et lance `SessionHandle::spawn_deploy`. Connexion, TOFU et authentification suivent les deux schémas ci-dessus. Une fois authentifié, le cœur ouvre un canal, exécute la commande constante `sh -c '…'` et envoie la ligne de clé sur l'entrée standard. Le script répond `YOUNJI_ADDED` ou `YOUNJI_PRESENT` (sortie lue sur 1 Kio au plus) ; tout autre résultat donne `ssh.deploy_failed`. Chaque étape du canal est bornée à 30 s et le déploiement entier à 60 s une fois authentifié. La session se termine ensuite (`keyDeployed`, puis `disconnected`).

Pour un hôte du carnet, l'utilisateur de l'hôte prime ; s'il est vide, celui de l'identité est utilisé. Un hôte sans identité ne passe pas par `ssh_connect_host` (`book.no_identity`) : l'interface ouvre le formulaire de connexion rapide pré-rempli. La reconnexion d'un onglet ouvert depuis le carnet rappelle `ssh_connect_host`, sans rien redemander (ADR-030).

### SFTP : ouverture et transfert (ADR-034)

```mermaid
sequenceDiagram
    participant UI as Interface
    participant C as src-tauri
    participant S as core-ssh
    participant SRV as Serveur SSH
    UI->>C: sftp_open(sessionId, onEvent)
    C->>C: coffre déverrouillé ? session connue ?
    C->>S: SessionHandle::open_sftp()
    S->>SRV: 2 canaux sur la connexion existante + sous-système « sftp » (sans nouvelle authentification)
    S-->>C: SftpClient (RawSftpSession + SftpSession), TransferQueue démarrée
    C-->>UI: SftpId
    UI->>C: local_pick_root()
    C->>C: boîte de dialogue du système, canonicalisation, RootPolicy
    C-->>UI: { id, name } (jamais le chemin)
    UI->>C: sftp_download(sftpId, paths, { kind: root, rootId, path })
    C->>C: chemins distants revalidés, chemin local résolu et confiné à la racine
    C->>S: TransferQueue::submit (politique « demander »)
    C-->>UI: JobId
    S-->>UI: transferQueued, transferStarted
    S->>SRV: parcours borné (lstat, liens non suivis), noms vérifiés (check_local_name)
    loop chaque fichier
        alt la destination existe
            S-->>UI: transferConflict(job, name)
            UI->>C: sftp_conflict_decision(job, action, applyToAll)
        end
        S->>SRV: lecture en pipeline (blocs de 256 Kio)
        S->>S: écriture dans un fichier temporaire du dossier de destination
        S->>S: renommage (sans écrasement sauf « écraser »), Mark-of-the-Web sous Windows
        S-->>UI: transferProgress (10/s au plus)
    end
    S-->>UI: transferFinished(bilan, code)
```

Un envoi suit le même chemin dans l'autre sens : sources choisies dans la boîte de dialogue du cœur (`pickFiles`, `pickFolder`) ou dans une racine (`root`), liens locaux non suivis, écriture directe du fichier distant ; à l'annulation, seul un fichier distant **créé** par le transfert est supprimé.

**Édition** (`sftp_edit`, SFTP-05) : le fichier (16 Mio au plus) est copié dans `<app_local_data>/edits/younji-edit-<aléatoire>/<nom>` (dossier 0700, fichier 0600 sous Unix), puis ouvert dans l'éditeur imposé (`editor.rs`). La copie est surveillée chaque seconde ; à chaque enregistrement, la taille et la date distantes sont comparées à celles connues, puis la copie est renvoyée (`editUploaded`) ou le renvoi est suspendu (`editConflict`). La copie est supprimée à `sftp_edit_close`, à la fin de la session et au verrouillage ; au démarrage, les copies abandonnées depuis plus de 24 h sont supprimées.

Bornes et délais (`client.rs`, `ops.rs`, `transfer.rs`, `queue.rs`) : ouverture du client 45 s ; 30 s par requête ; paquets de 256 Kio au plus ; listage de 10 000 entrées affichées, 20 000 requêtes READDIR et 120 s au plus ; résolution des cibles de liens d'un listage limitée à 256 liens et 10 s ; profondeur 64 et 100 000 éléments par opération récursive ; 1 000 éléments sélectionnés par transfert ; 64 transferts en attente par client.

## 4. Fenêtre, stockage et journaux

### Fenêtre principale
- Créée côté Rust dans `lib.rs` (aucune fenêtre dans `tauri.conf.json`), avec le titre `APP_NAME`. Étiquette `main`, qui correspond à `capabilities/main.json`.
- **Glisser-déposer natif désactivé** (`disable_drag_drop_handler`, ADR-034) : le glisser-déposer HTML5 entre les panneaux SFTP fonctionne sous Windows, et les chemins des fichiers déposés depuis le système ne sont jamais exposés à l'interface ; un tel dépôt est ignoré (`installExternalDropGuard`, `dnd.ts`).
- **Garde de navigation** (`navigation.rs`) : seule l'origine de l'application est autorisée (`tauri://localhost`, `http(s)://tauri.localhost`), plus le serveur Vite (`http://127.0.0.1:1420`) en développement uniquement. Toute autre navigation est refusée, et l'ouverture de nouvelles fenêtres aussi. Seul le schéma de l'adresse refusée est journalisé.

### Dossier de données
- **Installé** : le dossier `app_data_dir` de Tauri (`%APPDATA%\<identifier>\` sous Windows, `~/.local/share/<identifier>/` sous Linux).
- **Portable** (étape 13) : `./data/` à côté de l'exécutable si un fichier `portable.flag` y est présent.

| Fichier | Contenu | Protection | Disponible |
|---|---|---|---|
| `vault.younji` | Hôtes, groupes, identités, clés, clés d'hôte connues (format OpenSSH), délai de verrouillage, snippets (étape 5, champ `snippets` ajouté sans changer `SCHEMA_VERSION`, ADR-037) ; plus tard tunnels | Chiffré et authentifié, format v1 (`SECURITE.md` §3.3). Écriture atomique, 0600 (dossier 0700) sous Linux. Charge utile limitée à 64 Mio, vérifiée avant toute allocation. | Oui (étape 2) |
| `vault.younji.bak` | Version précédente du coffre | Chiffré. Copié de façon atomique avant chaque remplacement ; recopié depuis le nouveau fichier après un changement de mot de passe, ou supprimé si la copie échoue | Oui (étape 2) |
| `edits/` (dans `app_local_data_dir`) | Copies temporaires des fichiers distants ouverts en édition (`younji-edit-*`) | Dossier 0700, fichier 0600 sous Linux. Supprimées à la fin de l'édition, de la session et au verrouillage ; copies abandonnées depuis plus de 24 h supprimées au démarrage | Oui (étape 4) |
| `settings.json` | Langue, thème, taille de police du terminal | En clair, **aucune donnée sensible** (il doit être lisible avant le déverrouillage). Écriture atomique ; lecture limitée à 64 Kio ; chaque valeur relue est validée, une valeur inattendue reprend sa valeur par défaut | Oui (étape 2) |

**Clés d'hôte connues** : ce n'est plus un fichier. Elles sont dans le coffre (ADR-027), ce qui tranche le point ouvert d'ADR-006 et d'ADR-023. Un fichier `known_hosts` en clair laissé par l'étape 1 est repris à la création du coffre, puis supprimé. Une clé d'hôte se supprime dans Clés > Clés d'hôte connues (HKY-04).

**Fichiers de clé privée** : jamais lus en dehors d'une connexion rapide ou d'un import. Leur chemin reste en mémoire dans `SessionManager` pendant l'exécution, derrière un identifiant opaque (ADR-020). Une clé importée est stockée dans le coffre, sans sa passphrase d'origine (ADR-030) ; le fichier d'origine n'est ni modifié ni supprimé. Une clé générée n'existe que dans le coffre ; seule sa clé publique peut être exportée, dans un fichier `.pub` choisi par l'utilisateur (ADR-032).

**Préférences** : `settings.json`, lu par `settings_get` avant le premier rendu (la langue est fixée tout de suite). Le délai de verrouillage n'y figure pas : il est dans le coffre, authentifié (ADR-028). Au premier lancement de l'étape 2 (`firstRun`), les anciennes valeurs du `localStorage` (clé `younji.settings`, étape 1) sont reprises une fois, validées, puis la clé est supprimée.

### Journaux
- Dossier `app_log_dir` de Tauri (`%LOCALAPPDATA%\<identifier>\logs\` sous Windows, `~/.local/share/<identifier>/logs/` sous Linux), créé en 0700 sous Linux.
- Fichiers `younji.<date>.log`, rotation quotidienne, 7 fichiers au plus. En développement, les journaux sont aussi affichés sur la console. Si le dossier est inutilisable, repli sur la console (jamais sur un dossier temporaire partagé).
- `YOUNJI_LOG` (`error` à `trace`, `info` par défaut) ne règle que le niveau du **code de Younji** : cibles `younji`, `younji_lib`, `core_ssh` et `core_vault` (`OWN_TARGETS` dans `logging.rs` ; chaque nouvelle crate `core_*` s'y ajoute). Les dépendances, dont `russh`, restent plafonnées à `warn`, pour qu'un niveau élevé ne fasse jamais journaliser des paquets réseau ou des arguments IPC.
- `core_ssh` journalise la cible (`utilisateur@hôte:port`) et la cause de fin de session au niveau `debug`, un changement de clé d'hôte au niveau `warn`. Le coffre journalise les verrouillages (avec leur cause), le nombre d'échecs de déverrouillage et les erreurs d'écriture, jamais un contenu.
- Le SFTP ne journalise ni nom de fichier, ni chemin, ni contenu : seulement des compteurs (bilan d'un transfert), des identifiants opaques et des codes d'erreur.
- Aucun secret, aucun contenu de terminal, de coffre ni de fichier transféré (`SECURITE.md` §6).

## 5. Gestion des erreurs
1. Les crates renvoient des erreurs `thiserror` précises. `SshError` (`core-ssh`) et `VaultError` (`core-vault`) ne transportent aucune donnée : ni secret, ni entrée utilisateur, ni texte du serveur.
2. `src-tauri` les convertit en `AppError { code }`. Seul le code est sérialisé. Un test Rust vérifie que chaque code a sa traduction FR et EN.
   - `ssh.*` : par exemple `ssh.auth_failed`, `ssh.host_key_changed`, `ssh.connection_lost`. Ajoutés à l'étape 3 : `auth_prompt_invalid`, `auth_prompt_timeout`, `auth_cancelled`, `no_pending_prompt`, `invalid_answers`, `key_generation_failed`, `invalid_public_key`, `deploy_failed`. Ajoutés à l'étape 4 : `sftp_unavailable`, `sftp_failed`, `sftp_not_found`, `sftp_permission_denied`, `sftp_already_exists`, `sftp_not_directory`, `sftp_invalid_path`, `sftp_not_file`, `sftp_invalid_mode`, `sftp_unsafe_name`, `sftp_too_many_entries`, `local_io`, `local_outside_root`, `local_root_forbidden`, `transfer_cancelled`, `transfer_not_found`, `edit_too_large`, `edit_conflict`. Le texte d'état renvoyé par le serveur SFTP n'est jamais recopié : il est traduit en code.
   - `vault.*` (`VaultError`) : `not_found`, `already_exists`, `backup_only`, `locked`, `wrong_password_or_corrupted`, `invalid_file`, `unsupported_version`, `kdf_params_rejected`, `io`, `random_unavailable`, `password_too_short`, `password_too_weak`, `invalid_name`, `invalid_address`, `invalid_port`, `invalid_username`, `invalid_tags`, `invalid_notes`, `invalid_secret`, `identity_incomplete`, `invalid_auto_lock`, `too_many_entries`, `entry_not_found`, `key_in_use`, `identity_in_use`, `known_hosts_too_large` ; depuis l'étape 5 : `invalid_snippet`. Toute erreur de déchiffrement donne **le même** code, `vault.wrong_password_or_corrupted`, pour ne pas distinguer un mauvais mot de passe d'un fichier altéré.
   - Codes propres à `src-tauri` : `ssh.session_not_found`, `ssh.too_many_sessions`, `ssh.key_file_unknown`, `dialog.failed`, `url.invalid`, `url.open_failed`, `book.no_identity`, `settings.invalid`, `settings.io`, `keys.export_failed`, `internal` ; depuis l'étape 4 : `sftp.not_found`, `sftp.too_many`, `sftp.edit_not_found`, `sftp.too_many_edits`, `local.root_unknown`, `editor.open_failed`.
3. `src/lib/ipc.ts` normalise toute erreur reçue en `{ code }` ; un message brut n'est jamais propagé, et une erreur de forme inattendue devient `unknown`.
4. L'interface affiche `t('errors.<code>')`. Le détail technique part dans les journaux, **sans secret**.

## 6. Interface
- **Porte du coffre** (`features/vault/`) : au premier lancement, création du coffre (mot de passe et confirmation, indicateur de robustesse en direct, avertissement « aucune récupération possible ») ; ensuite, écran de déverrouillage, avec la cause du dernier verrouillage. Rien d'autre n'est affiché tant que le coffre est verrouillé.
- Mise en page type Termius : barre latérale (Hôtes, Clés, Snippets, Tunnels, SFTP, Paramètres, et bouton « Verrouiller ») et zone d'onglets. Le premier onglet (accueil) affiche la section choisie ; Tunnels affiche encore un emplacement réservé.
- **Carnet d'hôtes** (`features/hosts/`, HST-01, HST-02) : hôtes classés par groupe (un seul niveau), filtre « tous / sans groupe / groupe », recherche instantanée sur le nom, l'adresse, l'utilisateur et les étiquettes (insensible à la casse et aux accents) ; création, modification, suppression (avec confirmation) ; double-clic pour se connecter (`openHost.ts`).
- **Clés** (`features/keys/`, KEY-01 à KEY-03, HST-03, HKY-04) : clés générées dans le coffre (`KeyGenerateDialog`, Ed25519 par défaut) ou importées (boîte de dialogue du système) ; renommage, copie et export `.pub` de la clé publique, déploiement sur un hôte du carnet (`KeyDeployDialog`, qui reprend les dialogues de clé d'hôte et de questions du serveur), suppression ; identités (utilisateur, mot de passe et/ou clé), clés d'hôte connues (liste, suppression après un avertissement qui rappelle de vérifier l'empreinte).
- **Onglets de session** (`stores/sessions.ts`, `TabBar`) : deux sortes d'onglets, `kind: 'terminal'` et `kind: 'sftp'` (voir SFTP ci-dessous). Ouvrir, fermer, renommer (double-clic ou F2), réordonner (glisser ou raccourci). États : `form` → `connecting` → `hostKey` → `authenticating` (↔ `authPrompt`) → `connected` → `disconnected`, ou `blocked` si la clé d'hôte a changé. En `authPrompt`, l'onglet affiche `AuthPromptDialog` : hôte qui pose les questions, texte du serveur encadré comme « Message envoyé par le serveur », rappel que Younji ne demande jamais le mot de passe maître ici ; réponses masquées sauf si le serveur autorise l'écho, champs vidés dès l'envoi. Le store garde la demande en attente, jamais les réponses. La connexion rapide propose un troisième mode, « Questions du serveur (2FA) », sans aucun secret saisi d'avance. Le store ne contient **aucun secret** : pour un hôte du carnet, il garde l'identifiant de l'hôte ; pour une connexion rapide, la cible sert à pré-remplir le formulaire de reconnexion, qui redemande le mot de passe ou la passphrase (ADR-021). L'alerte de clé modifiée propose d'ouvrir la gestion des clés d'hôte.
- **Terminal** (`features/terminal/`) : un contrôleur `TerminalSession` par panneau, hors de React, qui garde l'instance xterm.js, son tampon (10 000 lignes) et sa session quand l'onglet est masqué. Taille de police réglable, appliquée à chaud. Addons fit (avec `ResizeObserver` et anti-rebond de 100 ms vers `ssh_resize`), search, web-links, unicode11 et WebGL (repli sur le rendu DOM si WebGL est indisponible). Au verrouillage, tous les contrôleurs sont détruits.
- **Protections du terminal** : pas d'addon presse-papiers, donc les séquences OSC 52 restent sans effet ; garde de collage (`paste.ts`) qui retire les marqueurs de « bracketed paste » et demande confirmation, avec aperçu, pour un texte multiligne ou contenant des caractères de contrôle ; liens `http(s)` ouverts au Ctrl+clic, avec confirmation de l'adresse réelle pour les liens OSC 8 (`links.ts`, ADR-022).
- **Panneaux** (SSH-07, ADR-035) : un onglet de terminal se découpe à droite (Ctrl+Maj+D) ou en bas (Ctrl+Maj+O), 8 panneaux au plus. Chaque panneau est une session à part (sa connexion, son `TerminalSession`, son panneau SFTP), rangée dans `tabs` avec `paneOf` = onglet qui l'affiche ; seul le panneau racine apparaît dans la barre d'onglets. `layouts[onglet]` garde l'arbre de disposition (`layout.ts` : feuilles = panneaux, nœuds `row` / `column` avec un ratio), `focusedPane[onglet]` le panneau actif. Un nouveau panneau reprend la cible du panneau d'origine (`split.ts`) : connexion directe pour un hôte du carnet, formulaire pré-rempli pour une connexion rapide. `SplitView` dessine l'arbre ; ses séparateurs se déplacent à la souris ou au clavier. Navigation par Ctrl+Alt+flèches, fermeture d'un panneau par Ctrl+Maj+X (le voisin prend la place ; si c'est le panneau racine, le suivant est promu onglet). Chaque panneau affiche en tête le nom de son serveur, et une barre d'outils (`PaneToolbar`) : découpe, snippets, diffusion, SFTP, fermeture.
- **Diffusion de la saisie** (`features/broadcast/`, store `broadcast`, SSH-08, ADR-036) : activée par Ctrl+Maj+I ou la barre d'outils, après le dialogue `BroadcastDialog` (sessions connectées de tous les onglets, cochées une à une, nom et adresse, 2 au moins). `TerminalSession.handleInput` recopie la saisie d'une cible dans la file d'entrée de chaque cible ; les réponses automatiques d'xterm et les rapports souris binaires restent dans leur session. Un collage diffusé passe toujours par la confirmation. Indicateurs : `BroadcastBanner` en haut de la fenêtre, bordure rouge et en-tête « DIFFUSION » sur les cibles, pastille dans `TabBar`. Arrêt : bouton, Ctrl+Maj+I, verrouillage, moins de 2 cibles (une cible déconnectée est retirée).
- **Snippets** (`features/snippets/`, SNP-01 à SNP-03, ADR-037) : section Snippets (liste, recherche, création, modification, suppression) ; la liste vient de `vault_snapshot`, l'enregistrement passe par `snippet_save` / `snippet_delete`. Ctrl+Maj+S ou la barre d'outils d'un panneau ouvre `SnippetPicker`, puis `SnippetRunDialog` : valeurs des variables (une ligne, validées), aperçu du texte final, sessions cochées, « Insérer » ou « Exécuter ». L'envoi passe par `TerminalSession.sendText`, vers les seules sessions cochées, sans diffusion. Les dialogues sont pilotés par le store `snippetRun`.
- **SFTP** (`features/sftp/`, SFTP-01 à SFTP-05, ADR-034) :
  - **Panneau latéral** à droite du terminal, ouvert par un bouton ou Ctrl+Maj+E quand la session est connectée. Il **suit le dossier du shell** grâce à la séquence OSC 7 (`registerOscHandler(7)`, chemin décodé strictement par `osc7.ts` et rangé dans `remoteCwd`). Si le shell n'envoie pas OSC 7, le bouton « Activer le suivi » insère dans le terminal une intégration au prompt constante (bash, zsh), comme un collage et sans Entrée : l'utilisateur la relit et la valide.
  - **Onglet double panneau** (`SftpTabPanel`, `kind: 'sftp'`, `parentTabId` = onglet de terminal) : dossier local à gauche (`LocalBrowser`, dans une racine choisie par `local_pick_root`, navigation seule), serveur à droite (`RemoteBrowser`), glisser-déposer interne entre les deux (types MIME propres à Younji, `dnd.ts`). Il utilise la connexion de son terminal : il se ferme avec lui et affiche un message si la session se termine. La section SFTP de la barre latérale liste les sessions connectées pour ouvrir l'un ou l'autre.
  - **Opérations distantes** : navigation, dossier parent et personnel, création de dossier, renommage, suppression (confirmation ; récursive pour un dossier, liens supprimés sans être suivis), chmod (cases ou valeur octale), téléchargement, envoi de fichiers ou d'un dossier, ouverture dans l'éditeur. Les noms suspects (caractères invisibles, réservés) sont signalés.
  - **File de transferts** (`TransferList`, store `useSftp`) : progression, annulation, bilan (copiés, ignorés, liens non copiés, noms refusés, échecs) ; dialogue de conflit (écraser, ignorer, garder les deux, « appliquer aux suivants ») ; état des éditions (renvoyé, conflit avec renvoi forcé, arrêt). Le store est vidé au verrouillage.
- **Raccourcis** (`features/shortcuts/`) : tous en Ctrl+Maj (ou Ctrl+Tab, Ctrl+PgPréc/PgSuiv, Ctrl+Alt+flèches) pour laisser au shell ses propres raccourcis, dont Ctrl+Maj+L pour verrouiller, Ctrl+Maj+E pour le panneau SFTP, Ctrl+Maj+D / Ctrl+Maj+O pour découper à droite / en bas, Ctrl+Maj+X pour fermer un panneau, Ctrl+Alt+flèches pour passer au panneau voisin, Ctrl+Maj+S pour envoyer un snippet et Ctrl+Maj+I pour activer ou arrêter la diffusion ; liste affichée dans les Paramètres. Hors du terminal, F5 et Ctrl+R sont bloqués, car un rechargement verrouille le coffre et ferme toutes les sessions.
- **Paramètres** (APP-05, VLT-03, VLT-04) : langue (système, français, anglais), thème (système, clair, sombre) et taille de police du terminal (8 à 32), appliqués à chaud et enregistrés dans `settings.json` ; délai de verrouillage automatique (1, 5, 10, 15, 30 ou 60 min, enregistré dans le coffre) ; changement du mot de passe maître ; raccourcis clavier ; nom et version via `app_info`.
- **Langue** : en mode « système », détectée depuis `navigator.languages` (première langue prise en charge), repli sur l'anglais. Fixée avant le premier rendu pour éviter un affichage dans la mauvaise langue.
- **Thème** : en mode « système », suit `prefers-color-scheme` à chaud ; classe `light` / `dark` sur `<html>`.
- Stores zustand : `useSettings`, `useUi`, `useSessions` (onglets et panneaux), `useBroadcast` (cibles de la diffusion), `useSnippetRun` (dialogues d'envoi de snippet), `useSftp` (transferts et éditions affichés) et `useVault` (état du coffre et contenu affichable, sans secret, vidé dès le verrouillage).

## 7. Outillage et CI/CD

### Vérifications locales
`npm run check` enchaîne ESLint, Prettier, `tsc -b`, Vitest (via `scripts/vitest.mjs`), `cargo fmt --check`, `cargo clippy -D warnings` et `cargo test`. `npm run audit` lance `cargo deny check` et `npm audit --omit=dev --audit-level=high`.

### Tests du coffre
`cargo test -p core-vault` : aller-retour création / déverrouillage ; mauvais mot de passe (code générique) ; un octet modifié à n'importe quelle position refusé ; paramètres Argon2 hors bornes et longueurs incohérentes refusés avant allocation ; nonce neuf à chaque sauvegarde et `.bak` présent ; changement de mot de passe (sel et DEK renouvelés) ; création qui n'écrase rien et refuse un `.bak` orphelin ; aucun nom d'hôte lisible dans le fichier ; permissions 0600/0700 sous Linux ; politique de mot de passe ; validation du modèle et intégrité référentielle ; `Debug` sans secret. Côté `src-tauri` : délai progressif, double horloge du minuteur, DTO sans secret, désérialisation stricte, connexion depuis le carnet selon l'identité.

### Tests SSH
- **Sans serveur externe** (`cargo test`) : `crates/core-ssh/tests/local_server.rs` lance un serveur russh en mémoire sur 127.0.0.1 et vérifie le TOFU, le blocage d'une clé modifiée (y compris d'un autre type), l'authentification et le contrôle de flux. Depuis l'étape 3 : keyboard-interactive (questions, mot de passe puis code après succès partiel, remplissage automatique limité, mauvais code, annulation, délai, serveur abusif, réponse sans question en attente), plafond du temps réseau de l'authentification, et déploiement (commande constante, clé reçue sur l'entrée standard, clé déjà présente, échecs, déploiement avec 2FA). Les tests unitaires de `deploy.rs` exécutent aussi le script avec `sh` dans un dossier temporaire (ajout, absence de doublon, permissions) ; ils sont ignorés proprement si `sh` est absent. Toutes les clés sont générées à l'exécution.
- **Contre OpenSSH** (`npm run test:integration`) : `crates/core-ssh/tests/openssh.rs`, tests `#[ignore]`, dont le déploiement d'une clé générée (ajout, second passage sans doublon, connexion par clé, permissions 700/600, puis retrait de la ligne). `scripts/integration.mjs` charge `.env.test` s'il existe puis lance `cargo test --workspace -- --ignored`. La cible est lue dans les variables `YOUNJI_TEST_*` (`ENVIRONNEMENT_DE_TEST.md` §6) ; un test dont la variable manque est ignoré, sauf si `YOUNJI_TEST_REQUIRED=1`.
- **SFTP sans serveur** (`cargo test -p core-ssh`) : la logique est testée contre un faux système de fichiers en mémoire (`sftp/memfs.rs`, qui implémente `RemoteFs`) et un vrai dossier local temporaire : listage (noms impossibles écartés, suspects signalés), suppression récursive sans suivre les liens, renommage sans écrasement, chmod borné, dossier récursif avec accents, espaces et fichiers vides, noms malveillants qui n'écrivent jamais hors de la destination, liens distants non suivis, lien local planté remplacé et non suivi, politiques de conflit, annulation sans fichier partiel (local et distant), liens locaux non suivis à l'envoi, arborescences démesurées refusées, file (conflits, annulation), édition (renvoi, conflit, noms refusés, taille), nettoyage des copies abandonnées, Mark-of-the-Web. Côté `src-tauri` : désérialisation stricte des requêtes (aucun chemin absolu accepté), événements sans chemin local, registre (racines oubliées au verrouillage), éditeur imposé, traduction de chaque code. Côté interface (Vitest) : décodage OSC 7, intégration au prompt constante et jamais validée d'office, panneau, listes, file de transferts, dialogues.
- **SFTP contre OpenSSH** : `sftp_round_trip_of_a_recursive_folder` (`openssh.rs`, `#[ignore]`) ouvre le SFTP sur une session, envoie un dossier (accents, espaces, fichier et dossier vides, fichier de 3 Mio), le liste, le retélécharge et compare le contenu octet par octet ; puis renommage sans écrasement, chmod, suppression récursive, et vérifie que le SFTP ne répond plus après la fin de la session.
- **Serveur de test** : `tests/docker/sshd/` (Debian 12 épinglée par empreinte, OpenSSH, sous-système SFTP fourni par `openssh-sftp-server`). Aucun secret dans l'image : le mot de passe arrive par un fichier d'environnement, les clés autorisées par un volume en lecture seule, et les clés d'hôte sont régénérées à chaque démarrage.

### `ci.yml` (Linux, à chaque push et PR)
Permissions en lecture seule, actions épinglées par SHA, `persist-credentials: false`. Cinq jobs :

| Job | Contenu |
|---|---|
| Interface | `npm ci --ignore-scripts`, lint, types, tests ; `npm audit --omit=dev --audit-level=high` **bloquant** ; audit npm complet affiché sans bloquer (exception documentée dans `SECURITE.md` §8) |
| Rust | Dépendances WebKitGTK, `cargo fmt --check`, `cargo clippy --locked -D warnings`, `cargo test --locked` |
| Intégration SSH | Génère à chaque exécution un mot de passe et des clés de test (Ed25519 avec passphrase, ECDSA, RSA), masqués dans les journaux ; démarre le conteneur `tests/docker/sshd` sur 127.0.0.1:2222 ; lance `cargo test -p core-ssh -- --ignored` avec `YOUNJI_TEST_REQUIRED=1`. Aucun secret GitHub. |
| cargo-deny | Avis RustSec, licences compatibles GPL-3.0, sources (`deny.toml`) |
| gitleaks | Binaire épinglé et vérifié par SHA-256, analyse de tout l'historique |

### `build.yml` (manuel ou tag `v*`)
Matrice `windows-latest` (NSIS + MSI) et `ubuntu-22.04` (deb + AppImage). `npm ci --ignore-scripts`, puis compilation directe par la CLI Tauri (`npx tauri build --ci --bundles … -- --locked`) : pas d'action tierce supplémentaire, aucun secret. Installeurs publiés en artefacts du workflow (14 jours), pas de release tant que le dépôt est privé.

### Dependabot
Hebdomadaire pour cargo, npm et github-actions, mises à jour mineures groupées, **délai de 7 jours** (`cooldown`) avant d'adopter une nouvelle version, pour laisser le temps de détecter une publication compromise.
