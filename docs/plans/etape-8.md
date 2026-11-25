# Plan — Étape 8 : RDP v1 (client du système)

> Rédigé en mode autonome (25/11/2026). Branche `etape/8-rdp-systeme`, partie de `etape/7-terminal-local` (étape 7 pas encore validée : la PR de l'étape 8 dépend de celle de l'étape 7).

Exigences : RDP-01 (hôtes RDP dans le carnet : adresse, port, utilisateur, domaine, résolution), RDP-02 (lancement de mstsc / xfreerdp **sans jamais passer le mot de passe en ligne de commande**).
Références : `SECURITE.md` §4.3, §6 (processus externes) ; ADR-007 (RDP en deux temps), ADR-026 (verrouillage), ADR-030 (identités).

## 1. Architecture retenue

- **Modèle** (`core-vault`) : un hôte porte un protocole, `ssh` (défaut, coffres existants inchangés) ou `rdp { domain, resolution }` avec `resolution` = plein écran ou largeur × hauteur bornées. Le mot de passe vient de l'**identité** de l'hôte, comme pour SSH (ADR-030) ; une clé d'identité est ignorée en RDP.
- **Crate `core-rdp`** (sans Tauri) :
  - `target.rs` : cible RDP validée (adresse, port, utilisateur, domaine, résolution), sans caractère de contrôle ni séparateur de ligne ;
  - `rdpfile.rs` : contenu du fichier `.rdp` (aucun mot de passe dedans) ;
  - `freerdp.rs` : arguments de `xfreerdp3`/`xfreerdp` (un argument par option, aucun shell) et contenu de l'entrée standard (`/from-stdin:force`) ;
  - `launch.rs` : détection du client, lancement, supervision et nettoyage ;
  - `wincred.rs` (Windows) : `CredWriteW` / `CredReadW` / `CredDeleteW`, type `CRED_TYPE_DOMAIN_PASSWORD`, cible `TERMSRV/<hôte>`, persistance `CRED_PERSIST_SESSION`.
- **src-tauri** : commande `rdp_launch(hostId)` qui lit l'hôte et l'identité dans le coffre (aucun secret ne vient de l'interface), registre des clients lancés (16 au plus), fermés au verrouillage. `ssh_connect_host` et `key_deploy` refusent un hôte RDP.
- **Interface** : choix du protocole dans le formulaire d'hôte (domaine, résolution), bouton « Ouvrir (RDP) » dans le carnet, message d'état (client lancé, client absent, mot de passe manquant).

## 2. Sous-tâches

| # | Sous-tâche | Fichiers | Commit |
|---|---|---|---|
| 1 | Protocole des hôtes dans le coffre, validation, compatibilité de lecture | `crates/core-vault/src/model.rs`, `error.rs` | `feat(vault): …` |
| 2 | Crate `core-rdp` (cible, `.rdp`, arguments FreeRDP, client Windows via le gestionnaire d'identifiants, lancement supervisé) et tests | `crates/core-rdp/` | `feat(rdp): …` |
| 3 | IPC : `rdp_launch`, registre, verrouillage, refus SSH d'un hôte RDP, DTO d'hôte | `src-tauri/`, `isolation/index.js`, `tests/security/` | `feat(ipc): …` |
| 4 | Interface : formulaire, carnet, FR/EN, tests | `src/features/hosts/` | `feat(ui): …` |
| 5 | Contrôles et documentation | — | `docs: …` |

## 3. Dépendances

| Dépendance | Rôle | Licence | Version |
|---|---|---|---|
| `windows-sys` (Windows seulement, déjà présent dans l'arbre via Tauri) | `CredWriteW`, `CredReadW`, `CredDeleteW`, `CredFree` | MIT OR Apache-2.0 | 0.61 |
| `tempfile` (déjà utilisé) | fichier `.rdp` au nom aléatoire | MIT OR Apache-2.0 | 3 |

## 4. Risques de sécurité et parades

| Risque | Parade |
|---|---|
| Mot de passe visible dans la liste des processus | Jamais en argument. Windows : identifiant `TERMSRV/<hôte>` écrit par l'API (pas `cmdkey /pass:`). Linux : entrée standard (`/from-stdin:force`), tube fermé aussitôt après l'écriture |
| Mot de passe dans l'environnement ou les journaux | Aucune variable d'environnement ; tampon de l'entrée standard dans `Zeroizing` ; journaux sans hôte ni utilisateur au-delà de `debug`, jamais le mot de passe |
| Identifiant Windows laissé derrière | Supprimé dès la fin de `mstsc`, au plus tard 60 s après le lancement, au verrouillage et à la fermeture de l'application (`Drop`) ; persistance `CRED_PERSIST_SESSION` (jamais sur disque ni itinérant) |
| Écrasement d'un identifiant enregistré par l'utilisateur pour ce serveur | Si `TERMSRV/<hôte>` existe déjà, Younji ne l'écrase pas et ne le supprime pas : mstsc utilise celui de l'utilisateur (signalé dans l'interface) |
| Type d'identifiant lisible par d'autres programmes | `CRED_TYPE_DOMAIN_PASSWORD` : le mot de passe n'est pas relisible par `CredRead` (seul LSA l'utilise), contrairement à un identifiant générique |
| Injection dans le fichier `.rdp` (ligne supplémentaire `alternate shell:…`) | Tous les champs validés : aucun caractère de contrôle, domaine et utilisateur limités ; le fichier ne contient que des clés fixes |
| Injection d'options FreeRDP (`/u:x /p:secret`, argument commençant par `/` ou `-`) | Chaque option est un argument distinct construit par le cœur (`/v:hôte:port`, `/u:…`, `/d:…`) ; adresse validée comme pour SSH, utilisateur et domaine sans espace ni caractère de contrôle |
| Fichier `.rdp` temporaire prévisible ou lisible | Dossier `<données locales>/rdp` (0700 sous Unix), nom aléatoire (`tempfile`), supprimé avec l'identifiant ; dossier vidé au démarrage |
| Programme lancé usurpé | Chemins absolus : `%SystemRoot%\System32\mstsc.exe` ; sous Linux, recherche de `xfreerdp3` puis `xfreerdp` dans les seuls dossiers absolus du `PATH` |
| Certificat du serveur | Windows : vérification par mstsc. Linux : `/cert:tofu` (premier certificat accepté, changement refusé), cohérent avec le TOFU des clés SSH ; jamais `/cert:ignore` |
| Client resté ouvert après le verrouillage | Les clients lancés par Younji sont arrêtés au verrouillage (ADR-026) |
| Lancement depuis une interface compromise | `rdp_launch` ne prend qu'un identifiant d'hôte ; le coffre doit être déverrouillé |

## 5. Tests

- `core-vault` : protocole par défaut `ssh` pour un coffre existant, validation du domaine et de la résolution.
- `core-rdp` : contenu `.rdp` (aucun mot de passe, champs validés), arguments FreeRDP (un argument par option, pas de mot de passe), entrée standard (ligne de domaine vide si absent), refus des caractères de contrôle, recherche du client dans le `PATH` (dossiers relatifs ignorés), lancement d'un faux client (script de test) qui lit l'entrée standard et vérifie que le mot de passe n'est pas dans ses arguments, nettoyage du fichier.
- `src-tauri` : refus de `ssh_connect_host` pour un hôte RDP, DTO sans secret.
- Interface : formulaire RDP (champs, envoi), bouton RDP dans le carnet.
- Manuel : connexion à la VM Windows du lab depuis Windows et Linux, `ps aux` / Gestionnaire des tâches, `cmdkey /list` après lancement.

## 6. Choix tranchés en autonomie

1. **Identifiant Windows de type « domaine »** (`CRED_TYPE_DOMAIN_PASSWORD`) écrit par l'API Win32 plutôt qu'un identifiant générique via la crate `keyring` : le mot de passe n'est pas relisible par les autres programmes de la session. Cela demande quelques lignes `unsafe` (FFI) : la règle `unsafe_code = "forbid"` du workspace devient `deny` pour la seule crate `core-rdp`, avec une autorisation limitée au module `wincred` (ADR-040).
2. **Identifiant existant de l'utilisateur jamais écrasé** ni supprimé.
3. **Suppression au plus tard 60 s après le lancement** : mstsc lit l'identifiant au début de la connexion. Point à vérifier en recette (acceptation d'un certificat inconnu plus longue que 60 s).
4. **Linux : mot de passe obligatoire** dans l'identité (sans terminal, xfreerdp ne peut pas le demander). Windows : sans mot de passe, mstsc le demande lui-même.
5. **Linux : `/cert:tofu`** plutôt que la question interactive (impossible sans terminal) ou `/cert:ignore` (refusé).
6. **Clients RDP fermés au verrouillage**, comme les sessions SSH (ADR-026).

## 7. Écarts en cours d'étape (revue de sécurité)

- **Identifiant Windows laissé à la fermeture de Younji (Moyenne)** : Tauri termine le processus sans détruire l'état géré, donc le nettoyage par `Drop` annoncé au §4 n'avait jamais lieu. Corrigé : à `RunEvent::Exit`, les clients lancés sont arrêtés et leurs identifiants supprimés ; chaque identifiant écrit par Younji porte un commentaire marqueur non secret (`Younji (temporaire)`), et au démarrage les `TERMSRV/*` portant ce marqueur sont supprimés (arrêt brutal). Un reste marqué n'est plus pris pour un identifiant de l'utilisateur : il est réécrit puis supprimé.
- **Question de certificat lisant l'entrée standard (Faible)** : `/cert:tofu,deny` refuse un certificat modifié sans aucune question, indépendamment de l'entrée standard.
- **Domaine et `utilisateur@domaine` (Faible)** : FreeRDP 3 ne déduit pas le domaine d'un UPN ; la ligne de domaine vide est envoyée sauf pour `DOMAINE\utilisateur` ou un domaine renseigné. Seul `xfreerdp3` (FreeRDP 3) est recherché : FreeRDP 2 ne connaît ni `/cert:tofu,deny` ni le même dialogue. À vérifier en recette avec `alice`, `alice@lab` et `LAB\alice`.
- **Deux lancements simultanés vers le même serveur (Faible)** : refusé (`rdp.busy`) tant que l'identifiant temporaire du premier existe.
- **Nom d'utilisateur `LAB\LAB\alice` (Faible)** : le domaine n'est ajouté que si l'utilisateur n'en porte pas déjà un.
- **Numéro d'ADR** : l'ADR annoncé au §6 (unsafe dans `core-rdp`) prend le numéro ADR-040, l'ADR-039 étant pris par l'étape 7.
- **Plusieurs instances de Younji (Faible, contre-revue)** : un identifiant marqué trouvé au lancement appartient forcément à une autre instance en cours (les restes d'un arrêt brutal sont supprimés au démarrage) : le lancement est refusé (`rdp.busy`) au lieu de l'écraser avec un autre compte.
