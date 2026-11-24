# Plan — Étape 6 : tunnels SSH

> Rédigé en mode autonome (24/11/2026). Branche `etape/6-tunnels`, partie de `main` (étape 5 fusionnée, PR #13).

Exigences : TUN-01 (`-L`, `-R`, `-D` SOCKS5), TUN-02 (gestionnaire : rattachement à un hôte, démarrage/arrêt, état, démarrage automatique en option), TUN-03 (`127.0.0.1` par défaut, avertissement explicite pour `0.0.0.0`).
Références : `SECURITE.md` §2 (M7 : tunnel exposé), §4.4 (tunnels), §5 (validation IPC) ; ADR-002 (russh), ADR-024 (Channel), ADR-026 (verrouillage), ADR-030 (identités), ADR-037 (ajout pur au schéma du coffre).

## 1. Architecture retenue

- **Une connexion SSH dédiée par tunnel** (équivalent de `ssh -N -L …`). Le tunnel est une session `core-ssh` d'un troisième type (`Purpose::Tunnel`), à côté du shell et du déploiement de clé : même vérification TOFU, même authentification (identité de l'hôte, 2FA keyboard-interactive), même pilotage (`ssh_host_key_decision`, `ssh_auth_answer`, `ssh_close`), même fermeture au verrouillage (ADR-026). Arrêter un tunnel ferme sa connexion, son écoute et toutes ses connexions relayées : le port est libéré.
- **Définitions dans le coffre** (`core-vault`, champ `tunnels` ajouté sans changer `SCHEMA_VERSION`, comme ADR-037) : nom, hôte du carnet, type, portée d'écoute (`loopback` / `allInterfaces`), port d'écoute, destination (hôte, port), démarrage automatique. Elles révèlent l'infrastructure : chiffrées au repos.
- **Relais** dans `core-ssh/src/tunnel/` :
  - `-L` : écoute TCP locale → `direct-tcpip` vers la destination vue du serveur.
  - `-D` : écoute TCP locale, mini-serveur **SOCKS5** (RFC 1928, `CONNECT` seul, sans authentification) écrit à la main → `direct-tcpip`.
  - `-R` : `tcpip-forward` demandé au serveur ; chaque canal `forwarded-tcpip` reçu est **vérifié** (port demandé) puis relié à la destination locale **configurée** (jamais à une adresse choisie par le serveur).
  - Compteurs (connexions actives, total) remontés par le `Channel` d'événements, au plus toutes les 250 ms.
- **Interface** : section Tunnels (liste, formulaire, démarrage/arrêt, état, compteurs, indicateur permanent « exposé »), store `tunnels` qui garde l'état des tunnels démarrés hors de la section, dialogues de clé d'hôte et de 2FA montés au niveau de l'application. Démarrage automatique : après le déverrouillage du coffre.

## 2. Sous-tâches (dans l'ordre)

| # | Sous-tâche | Fichiers / crates | Commit(s) |
|---|---|---|---|
| 1 | **Modèle** : `Tunnel`, `TunnelKind`, `BindScope`, CRUD validé (nom, ports 1–65535, destination, hôte existant), 256 tunnels max, suppression en cascade avec l'hôte, revérification au chargement | `crates/core-vault/src/model.rs`, `error.rs`, `lib.rs` | `feat(vault): …` |
| 2 | **SOCKS5** : analyse bornée de la négociation (versions, méthodes, `CONNECT`, IPv4/IPv6/domaine), réponses d'erreur, délai | `crates/core-ssh/src/tunnel/socks.rs` | `feat(ssh): …` |
| 3 | **Tunnels** : `TunnelSpec` validé, écoute, relais bidirectionnel, `-R` dans le `ClientHandler` (refus des canaux non demandés, y compris pour les sessions shell), compteurs, arrêt propre ; événements `TunnelReady` / `TunnelStats` ; codes d'erreur | `crates/core-ssh/src/tunnel/`, `session.rs`, `error.rs` | `feat(ssh): …` |
| 4 | **IPC** : `tunnel_save`, `tunnel_delete`, `tunnel_start` ; tunnels dans le snapshot ; `build.rs`, capability, isolation | `src-tauri/src/commands/tunnels.rs`, `ssh.rs`, `book.rs`, `lib.rs`, `build.rs`, `capabilities/main.json`, `isolation/index.js` | `feat(ipc): …` |
| 5 | **Interface** : section Tunnels, formulaire avec confirmation pour `0.0.0.0`, store, dialogues globaux, démarrage auto, arrêt au verrouillage ; FR/EN | `src/features/tunnels/`, `src/stores/tunnels.ts`, `src/lib/{ipc,types}.ts`, locales | `feat(ui): …` |
| 6 | Tests d'intégration (serveur russh local du dépôt) : `-L`, `-D`, `-R`, arrêt qui libère le port | `crates/core-ssh/tests/` | `test(ssh): …` |
| 7 | `npm run check`, revue de sécurité, QA, documentation | — | `fix(security): …`, `docs: …` |

## 3. Dépendances à ajouter

Aucune. SOCKS5 (`CONNECT` seul) tient en une centaine de lignes ; une crate SOCKS complète (`fast-socks5`…) apporterait `BIND`/`UDP ASSOCIATE` et de l'authentification inutiles, donc de la surface. Le relais utilise `tokio::io::copy_bidirectional` (déjà présent).

## 4. Risques de sécurité et parades

| Risque | Parade |
|---|---|
| Tunnel exposé au réseau par erreur (M7) : un `-D` sur `0.0.0.0` fait du poste un proxy ouvert vers le réseau du serveur | Portée **fermée** (`loopback` → `127.0.0.1`, `allInterfaces` → `0.0.0.0`) : aucune adresse libre. `loopback` par défaut. `allInterfaces` exige une **confirmation explicite** dans l'interface **et** un drapeau `exposureConfirmed` vérifié par le cœur à l'enregistrement. Indicateur permanent dans la liste et sur l'état du tunnel |
| Serveur malveillant qui ouvre des canaux `forwarded-tcpip` non demandés (pour joindre le réseau du poste) | Le `ClientHandler` **refuse** tout `forwarded-tcpip` qui ne correspond pas au port demandé par un tunnel `-R` de cette connexion (sessions shell comprises : russh les acceptait par défaut). La destination locale est celle de la définition, jamais celle annoncée par le serveur |
| Épuisement de ressources (connexions en rafale, négociation SOCKS lente) | 128 connexions simultanées par tunnel (au-delà : refus), négociation SOCKS bornée en taille et en temps (10 s), connexion locale `-R` bornée (10 s), canaux bornés |
| Entrées IPC / coffre malformées (ports, hôtes, caractères de contrôle) | Validation dans `core-vault` à l'enregistrement **et** au chargement, revalidation dans `core-ssh` (`Target`) au démarrage ; désérialisation stricte (`deny_unknown_fields`) |
| SOCKS : demandes autres que `CONNECT`, adresses piégées | `BIND` et `UDP ASSOCIATE` refusés (code 7) ; nom de domaine 1–255 octets sans caractère de contrôle ; port 0 refusé |
| Tunnel qui survit au verrouillage ou à la fermeture | Tunnels enregistrés comme sessions : fermés par `close_all_sessions` ; tâche propriétaire avec `CancellationToken`, connexions relayées annulées avec elle ; écoute fermée = port libéré |
| Secrets | Aucun nouveau secret : identité de l'hôte lue dans le coffre par le cœur ; sans identité, le serveur pose ses questions (keyboard-interactive). Aucun journal des destinations ni des adresses des clients |
| Démarrage automatique non voulu | Option désactivée par défaut ; ne démarre qu'après un déverrouillage explicite ; une clé d'hôte inconnue demande toujours la décision |

## 5. Tests prévus
- **core-vault** : CRUD des tunnels, validation (ports, destination, portée, hôte inexistant), cascade à la suppression de l'hôte, lecture d'un coffre sans tunnels, revérification au chargement.
- **core-ssh** : SOCKS5 (négociation valide IPv4/IPv6/domaine, méthodes sans « pas d'authentification », commandes refusées, tailles, versions), `TunnelSpec` (validation), codes d'erreur.
- **Intégration** (serveur russh local, sans réseau externe) : `-L` vers un écho TCP, `-D` via SOCKS5, `-R` (le serveur ouvre un `forwarded-tcpip`), refus d'un `forwarded-tcpip` non demandé, arrêt qui libère le port.
- **src-tauri** : désérialisation stricte, confirmation d'exposition exigée, snapshot, événements en camelCase, cohérence `build.rs` / capabilities / isolation (test existant).
- **Interface (Vitest)** : formulaire (valeurs par défaut sûres, confirmation `0.0.0.0`, champs selon le type), store (démarrage, compteurs, arrêt, verrouillage), liste (indicateur d'exposition), locales FR/EN.

## 6. Choix tranchés en autonomie (règle « option la plus sûre »)
1. **Connexion dédiée par tunnel** plutôt que greffée sur un terminal : cycle de vie indépendant, arrêt qui libère tout, aucun effet sur les terminaux ; coût : une authentification de plus (2FA redemandée).
2. **Portée d'écoute fermée** (`127.0.0.1` ou `0.0.0.0`), pas d'adresse libre ni d'IPv6 : on ne peut pas exposer « un peu » par erreur ; à rouvrir si le besoin d'écouter sur une interface précise apparaît.
3. **Double confirmation de l'exposition** (interface + drapeau vérifié par le cœur).
4. **SOCKS5 sans authentification, `CONNECT` seul** : suffisant pour un navigateur ; la protection est l'écoute sur `127.0.0.1`.
5. **Refus des `forwarded-tcpip` non demandés** pour toutes les connexions.
6. **Démarrage automatique** = au déverrouillage du coffre, jamais au lancement de l'application sans déverrouillage.
7. **Suppression d'un hôte** = suppression de ses tunnels (pas de tunnel orphelin).
8. **Pas de dépendance SOCKS** : code maison minimal et testé.
9. **Destination `-L`/`-D` résolue par le serveur** (comme OpenSSH) : le poste ne fait aucune requête DNS pour la destination.

## 7. Écarts en cours d'étape
1. **Confirmation native de l'exposition** (revue de sécurité, 1re passe, Moyenne) : le drapeau `exposureConfirmed` venant de l'IPC ne protège pas d'une WebView compromise (M4). Ajout d'une boîte de dialogue **native, ouverte par le cœur**, à chaque démarrage d'un tunnel sur `0.0.0.0` (démarrage automatique compris). Le drapeau reste comme garde-fou contre un bogue de l'interface.
2. **Course entre confirmation et démarrage** (2e passe, Moyenne) : le cœur garde la définition et l'hôte affichés (sans secret), relit le coffre après la confirmation et **refuse** le démarrage s'ils ont changé (`tunnels.changed`). Une seule confirmation ouverte à la fois (`tunnels.confirmation_pending`). Noms de tunnel sans caractères invisibles, texte de la boîte nettoyé.
3. **Refus de tous les canaux ouverts par le serveur** (1re passe, Faible) : `session`, `direct-tcpip`, `x11`, agent, sockets Unix, en plus des `forwarded-tcpip` non demandés ; pour toutes les connexions (shells et déploiements compris).
4. **Divers** : canal `-R` déjà accepté au-delà de 128 connexions fermé explicitement ; journaux de refus et de saturation écrits une fois ; adresse du client local non transmise au serveur (`127.0.0.1:0`) ; garde `Drop` sur l'emplacement `-R`.
5. **Section provisoire supprimée** : `SectionPlaceholder` et les chaînes `sections.*` n'ont plus d'usage (la dernière section provisoire était Tunnels).
6. **Tests OpenSSH** : `-L` et `-D` ajoutés (ignorés sans `YOUNJI_TEST_*`) ; `-R` n'est couvert que contre le serveur russh local, d'où les tests manuels 9 à 11.
