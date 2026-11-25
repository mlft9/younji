# Plan — Étape 7 : terminal local

> Rédigé en mode autonome (25/11/2026). Branche `etape/7-terminal-local`, partie de `main` (étape 5 validée et fusionnée, étape 6 retirée par ADR-038).
> Les étapes 7, 8 et 9 ont été demandées ensemble : les branches sont empilées (`etape/8-*` part de `etape/7-*`, `etape/9-*` de `etape/8-*`).

Exigences : LOC-01 (Windows : PowerShell, PowerShell 7, CMD, distributions WSL détectées automatiquement), LOC-02 (Linux : shell par défaut, bash, zsh).
Références : `SECURITE.md` §5 (processus externes lancés par le cœur), §6 ; ADR-024 (flux et acquittement), ADR-026 (verrouillage), ADR-028 (application derrière le déverrouillage).

## 1. Architecture retenue

- **Crate `core-term`** (sans Tauri) : détection des profils (`profiles.rs`), session PTY (`session.rs`) avec `portable-pty` (ConPTY sous Windows, `openpty` sous Linux). Une session = un fil de lecture (lots bornés, contrôle de flux par acquittement comme SSH, 2 Mio non acquittés au plus), un fil d'écriture (file bornée), un fil qui attend la fin du processus.
- **Profils désignés par identifiant** : l'interface demande `local_profiles`, reçoit `{ id, name, kind }` et ouvre `local_open { profileId }`. Le programme et ses arguments sont **toujours** reconstruits par le cœur à partir de la détection : aucun chemin ni argument ne vient de l'interface.
- **src-tauri** : `LocalTerminals` (registre, 32 sessions au plus), commandes `local_profiles`, `local_open`, `local_write`, `local_resize`, `local_ack`, `local_close`. Mêmes `Channel` que SSH (octets bruts + événements JSON `connected` / `disconnected`).
- **Interface** : un onglet de terminal porte soit une cible SSH, soit un profil local. `TerminalSession` utilise un « transport » (écrire, redimensionner, acquitter, fermer) choisi à l'ouverture ; xterm, recherche, collage gardé, liens et split restent communs. Menu « Terminal local » dans la barre latérale et bouton dans la barre d'onglets.

## 2. Sous-tâches

| # | Sous-tâche | Fichiers | Commit |
|---|---|---|---|
| 1 | Crate `core-term` : profils (Windows : `powershell.exe`, `pwsh.exe`, `cmd.exe`, distributions WSL via `wsl.exe -l -q` ; Linux : `$SHELL` s'il figure dans `/etc/shells`, puis bash, zsh, fish, sh de `/etc/shells`), session PTY, erreurs typées, tests | `crates/core-term/` | `feat(term): …` |
| 2 | IPC : registre, commandes, capability, isolation, codes d'erreur, fermeture au verrouillage et au rechargement | `src-tauri/` , `isolation/index.js` | `feat(ipc): …` |
| 3 | Interface : transport, onglet local, choix du profil, relance, FR/EN, tests | `src/` | `feat(ui): …` |
| 4 | Contrôles et documentation | — | `docs: …` |

## 3. Dépendances

| Dépendance | Rôle | Licence | Version |
|---|---|---|---|
| `portable-pty` | PTY multiplateforme (ConPTY / openpty), projet wezterm | MIT | 0.9.0 |

`cargo deny check` lancé après l'ajout.

## 4. Risques de sécurité et parades

| Risque | Parade |
|---|---|
| Interface compromise qui lance un programme arbitraire | Seuls les profils détectés par le cœur sont lançables, désignés par un identifiant ; programme et arguments reconstruits côté Rust ; aucun shell intermédiaire ; nom de distribution WSL relu de `wsl.exe` et validé (caractères admis, longueur) avant d'être passé en argument séparé |
| `$SHELL` piégé (variable d'environnement) | Retenu seulement s'il est absolu, existe et figure dans `/etc/shells` |
| Sortie massive (`yes`, `cat` d'un gros fichier) | Lots de 32 Kio, acquittement, lecture suspendue au-delà de 2 Mio non acquittés |
| Terminal local accessible coffre verrouillé | `local_open` refusé coffre verrouillé ; toutes les sessions locales fermées (processus tués) au verrouillage et au rechargement, tampons détruits par l'interface (ADR-026) |
| Processus orphelins | Processus tué à la fermeture de l'onglet et à la destruction de la session (`Drop`) |
| Fuite d'informations | Aucun contenu du terminal ni commande journalisés ; seuls l'identifiant de session et le type de profil |
| Variables d'environnement de Younji transmises au shell | Environnement hérité de l'utilisateur (c'est son propre shell), plus `TERM=xterm-256color` et `COLORTERM=truecolor` ; aucune variable propre à Younji ajoutée |

## 5. Tests

- `core-term` : détection Linux (`/etc/shells` factice, `$SHELL` hors liste refusé), analyse de la sortie UTF-16 de `wsl.exe`, validation des noms de distribution, session réelle `/bin/sh` (commande exécutée, redimensionnement, fin de processus, fermeture qui tue le processus, contrôle de flux).
- `src-tauri` : identifiants de profil inconnus refusés, cohérence `build.rs` / capability / isolation (test existant).
- Interface : ouverture d'un onglet local (IPC simulée), transport local utilisé pour la saisie, pas de formulaire SSH ni de panneau SFTP sur un onglet local, locales FR/EN.

## 6. Choix tranchés en autonomie

1. **Commandes IPC séparées (`local_*`)** plutôt que de réutiliser `ssh_*` : une commande SFTP ou de clé d'hôte ne peut jamais viser une session locale par erreur.
2. **Terminal local derrière le coffre** (ADR-028) et fermé au verrouillage, comme les sessions SSH : cohérent avec ADR-026, et un poste verrouillé n'offre pas de shell.
3. **Pas de diffusion ni de snippet vers un terminal local** dans cette étape : la diffusion vise des serveurs listés par adresse ; mélanger shell local et serveurs augmente le risque d'erreur (M7). Réversible.
4. **Profils non modifiables** (pas de programme personnalisé) : un champ « programme » libre reviendrait à exposer l'exécution arbitraire à l'interface.
5. **Dossier de départ** : dossier personnel de l'utilisateur.
