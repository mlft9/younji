# AGENTS.md — Younji

Client SSH / SFTP / RDP de bureau pour Windows et Linux, dans l'esprit de MobaXterm et Termius.
Projet personnel / portfolio orienté **cybersécurité** : la sécurité est une exigence de premier rang, pas une finition.

## Documents de référence (à lire avant de coder une étape)
- `docs/CAHIER_DES_CHARGES.md` : exigences numérotées (SSH-01, VLT-02…) et hors périmètre
- `docs/ROADMAP.md` : étapes, critères d'acceptation, définition de « terminé »
- `docs/ARCHITECTURE.md` : découpage en crates, flux IPC, stockage
- `docs/SECURITE.md` : règles et schéma cryptographique (**obligatoire** dès qu'on touche aux secrets, au réseau, à l'IPC ou à la config Tauri)
- `docs/DECISIONS.md` : décisions d'architecture (ADR). Ne pas revenir sur une décision sans en ajouter une nouvelle.
- `docs/ENVIRONNEMENT_DE_TEST.md` : lab Proxmox et serveurs de test

## Stack
- **Tauri 2 stable** (jamais Tauri 3 alpha). Backend Rust, edition 2021+, toolchain stable.
- SSH : `russh`, `russh-sftp`. Terminal local : `portable-pty`. RDP natif : `ironrdp`.
- Crypto : `argon2`, `chacha20poly1305` (RustCrypto), `zeroize`, `secrecy`, `rand` (OsRng). Trousseau : `keyring`.
- Frontend : React + TypeScript strict + Vite + Tailwind CSS + shadcn/ui, `@xterm/xterm` (+ addons fit, search, web-links), `zustand`, `i18next` / `react-i18next`.
- Tests : `cargo test`, Vitest + Testing Library.

## Organisation du dépôt
```
src/                 Interface React (composants, stores zustand, locales fr/en, lib/ipc.ts)
src-tauri/           Application Tauri : commandes fines, état, capabilities, config
crates/core-ssh/     SSH, clés d'hôte, SFTP (sans dépendance à Tauri)
crates/core-vault/   Coffre chiffré et modèle de données
crates/core-term/    Terminal local (PTY)
crates/core-import/  Import ~/.ssh/config, known_hosts
crates/core-rdp/     RDP (étapes 8 et 12)
docs/                Documentation projet
.github/workflows/   CI (ci.yml) et compilation des installeurs (build.yml)
```
Règle d'or : **la logique métier vit dans `crates/`**, testable sans Tauri. `src-tauri` ne fait que relier les commandes IPC aux crates.

## Commandes (créées à l'étape 0)
| Commande | Rôle |
|---|---|
| `npm run tauri dev` | Lancer l'application en développement |
| `npm run check` | **Tout vérifier avant de livrer** : lint + types + tests front + fmt/clippy/tests Rust |
| `npm run lint` | ESLint + Prettier (vérification) + `tsc --noEmit` |
| `npm run test` | Tests frontend (Vitest) |
| `npm run check:rust` | `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` |
| `npm run test:rust` | `cargo test --workspace` |
| `npm run test:integration` | Tests d'intégration SSH (nécessitent un serveur de test, voir `docs/ENVIRONNEMENT_DE_TEST.md`) |
| `npm run audit` | `cargo deny check` + `npm audit --audit-level=high` |
| `npm run tauri build` | Construire les installeurs en local |

## Conventions
- **Langue** : identifiants de code en anglais ; commentaires, documentation et messages de commit en français.
- **Commits** : Conventional Commits, avec le type en anglais et la description en français, par exemple `feat(ssh): ajoute la vérification des clés d'hôte`. Commits petits et cohérents.
- **Branches** : `etape/<n>-<slug>` (ex. `etape/1-terminal-ssh`), fusionnées dans `main` après validation par le porteur.
- **Rust** : erreurs typées (`thiserror` dans les crates, conversion en codes d'erreur sérialisables pour l'IPC) ; pas de `unwrap()` / `expect()` hors tests ; async avec `tokio` ; `tracing` pour les journaux.
- **TypeScript** : `strict`, pas de `any`, appels IPC uniquement via `src/lib/ipc.ts` (typés).
- **Interface** : aucune chaîne visible en dur. Tout passe par i18n, avec les clés FR et EN ajoutées ensemble. Le backend renvoie des **codes d'erreur**, traduits côté interface.
- **Dépendances** : justifier chaque ajout (utilité, maintenance, licence compatible GPL-3.0) dans la description de l'étape. Préférer les crates RustCrypto et l'écosystème Tauri officiel.

## Règles de sécurité non négociables
1. Aucun secret (mot de passe, passphrase, clé privée, DEK, code OTP) dans : les journaux, les messages d'erreur, les arguments de processus, le disque en clair, les commits.
2. Secrets en mémoire dans `secrecy::SecretString` / `SecretBox` ou `Zeroizing<…>`. Jamais de `Debug` ni de `Display` qui exposent un secret.
3. Les secrets restent dans le backend Rust. L'interface les envoie une seule fois à la saisie, puis les désigne par **identifiant**. Jamais de secret renvoyé vers l'interface.
4. Clés d'hôte toujours vérifiées (TOFU) ; changement de clé = connexion refusée. Aucune option « ignorer ».
5. Toute entrée venant de l'IPC ou du réseau est validée côté Rust : chemins, ports, tailles, noms de fichiers distants.
6. Config Tauri : CSP stricte, capabilities minimales par fenêtre, aucun contenu distant, aucun plugin qui ouvre un shell arbitraire.
7. Crypto : uniquement les primitives et paramètres de `docs/SECURITE.md`. Aléa via `OsRng`. Aucune crypto « maison ».
8. Fichiers sensibles écrits de façon atomique, avec des permissions restrictives (0600 sous Linux).
9. Pas de télémétrie, pas d'appel réseau en dehors des connexions demandées par l'utilisateur (et, plus tard, de la vérification de mise à jour).

## Définition de « terminé » pour une étape
`npm run check` passe ; tests ajoutés ; chaînes FR + EN ; revue de sécurité sans problème bloquant ; docs à jour (`ARCHITECTURE.md`, `DECISIONS.md` si besoin, `CHANGELOG.md`, statut dans `ROADMAP.md`) ; plan de test manuel fourni au porteur.
