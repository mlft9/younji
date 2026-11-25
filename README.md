# Younji

Client **SSH / SFTP / RDP** de bureau pour **Windows et Linux**, dans l'esprit de MobaXterm et Termius, avec la sécurité comme priorité.

> 🚧 **Statut** : étapes 1 à 5 (terminal SSH multi-onglets, coffre chiffré et carnet d'hôtes, 2FA et générateur de clés, SFTP, écran partagé, snippets et diffusion multi-sessions) validées. Disponible : connexion SSH par mot de passe, par clé ou avec 2FA (code TOTP), vérification stricte des clés d'hôte, terminal en onglets, coffre chiffré avec verrouillage manuel et automatique, carnet d'hôtes, identités, clés importées ou générées, export et déploiement de la clé publique, explorateur SFTP, écran partagé, snippets avec variables, diffusion de la saisie vers plusieurs serveurs. Prêtes à tester : étape 7 (terminal local : PowerShell, CMD, WSL, bash, zsh) et étape 8 (ouverture d'un hôte RDP du carnet dans mstsc sous Windows ou FreeRDP 3 sous Linux, sans exposer le mot de passe). Avancement détaillé dans la [feuille de route](docs/ROADMAP.md).

## Fonctionnalités prévues
- Terminal SSH multi-onglets, écran partagé, diffusion de commandes vers plusieurs serveurs
- Coffre chiffré (Argon2id + XChaCha20-Poly1305), déverrouillage par Windows Hello ou le trousseau Linux
- Vérification stricte des clés d'hôte (protection contre l'homme du milieu)
- 2FA (TOTP), clés matérielles FIDO2, générateur et déploiement de clés SSH
- Explorateur SFTP (panneau qui suit le terminal + vue double panneau)
- Snippets de commandes
- Terminal local (PowerShell, CMD, WSL, bash)
- RDP (client du système, puis natif dans un onglet)
- Import de `~/.ssh/config`, export/import chiffré
- Interface en français et en anglais, version portable, mises à jour signées

## Stack
Tauri 2 · Rust (russh, IronRDP, RustCrypto) · React · TypeScript · Tailwind · shadcn/ui · xterm.js

## Documentation
| Document | Contenu |
|---|---|
| [Cahier des charges](docs/CAHIER_DES_CHARGES.md) | Exigences, périmètre, contraintes |
| [Feuille de route](docs/ROADMAP.md) | Étapes et critères d'acceptation |
| [Architecture](docs/ARCHITECTURE.md) | Découpage, IPC, stockage |
| [Sécurité](docs/SECURITE.md) | Modèle de menace, cryptographie, durcissement |
| [Décisions](docs/DECISIONS.md) | Journal des choix d'architecture (ADR) |
| [Environnement de test](docs/ENVIRONNEMENT_DE_TEST.md) | Lab Proxmox |

---

## Démarrer le développement avec Claude Code (Windows)

### 1. Installer les prérequis
Développe **nativement sous Windows**, pas dans WSL : c'est indispensable pour produire les binaires Windows.
- [Git pour Windows](https://git-scm.com/download/win)
- [Node.js](https://nodejs.org/) 22 ou plus récent
- [Rust](https://rustup.rs/) avec la toolchain par défaut `stable-x86_64-pc-windows-msvc` (la version et les composants `rustfmt` / `clippy` sont fixés par `rust-toolchain.toml`)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/), charge de travail « Développement Desktop en C++ »
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny), pour `npm run audit` : `cargo install --locked cargo-deny`
- WebView2 : déjà présent sur Windows 11 et sur les Windows 10 à jour
- Liste officielle et à jour : [prérequis Tauri](https://v2.tauri.app/start/prerequisites/)
- [Claude Code](https://code.claude.com/docs/en/setup)
- *(Optionnel)* [GitHub CLI](https://cli.github.com/) (`gh auth login`) pour que Claude Code puisse ouvrir des PR et lancer les compilations, toujours après ton accord

### 2. Créer le dépôt
Sur GitHub, crée le dépôt **vide** `younji` (sans README, licence ni .gitignore). Ensuite, dans ce dossier :
```powershell
git init -b main
git add .
git commit -m "docs: cadrage initial du projet"
git remote add origin https://github.com/mlft9/younji.git
git push -u origin main
```

### 3. Personnaliser (optionnel)
Copie `CLAUDE.local.example.md` en `CLAUDE.local.md` et complète-le : adresses des VM du lab, préférences. Ce fichier n'est pas commité.

### 4. Lancer Claude Code
```powershell
claude
```
Vérifie que tout est chargé :
- `/context` : `CLAUDE.md` et `AGENTS.md` apparaissent dans les fichiers de mémoire.
- `/agents` : `security-reviewer`, `qa-validator` et `docs-keeper` sont listés.
- `/` : les commandes `etape`, `retour`, `release` et `revue-securite` sont proposées.

Puis démarre :
```
/etape 0
```

### 5. Lancer l'application
```powershell
npm ci --ignore-scripts
npm run tauri dev
```
`--ignore-scripts` : aucun script d'installation npm n'est exécuté, comme en CI. Avant de livrer : `npm run check` (lint, types, tests front et Rust). Liste complète des commandes : [AGENTS.md](AGENTS.md).

### 6. Le cycle de travail
```
/etape N  →  Claude propose un plan  →  tu valides (ou corriges)
          →  Claude code, teste, fait relire la sécurité, documente
          →  il te donne un plan de test avec des cases à cocher
          →  tu testes sur ton lab
/retour "1 ok, 2 ok, 3 ko : l'onglet ne se ferme pas…"
          →  corrections + tests de non-régression
/retour "tout est validé"  →  l'étape passe à ✅, fusion dans main (après ton accord)
/etape N+1
```
Autres commandes : `/revue-securite [cible]` pour un audit ciblé, `/release 0.x.y` pour préparer une version.

### Garde-fous déjà configurés (`.claude/settings.json`)
- Autorisés sans confirmation : compilation, tests, lint, commandes git en lecture, commits locaux.
- **Confirmation demandée** : `git push`, fusion, tags, ajout de dépendances, modification de `docs/SECURITE.md`, des capabilities Tauri et des workflows CI.
- **Interdits** : lire `~/.ssh/`, les fichiers `.env`, les clés `*.key` ; `push --force` ; suppressions récursives.

---

## Sécurité
Pour signaler une vulnérabilité, **n'ouvrez pas d'issue publique** : suivez la [politique de sécurité](SECURITY.md).

## Licence
Distribué sous licence **GPL-3.0** : voir [LICENSE](LICENSE).

*Développé avec l'assistance de Claude Code. Le processus est décrit dans l'[ADR-013](docs/DECISIONS.md#adr-013--développement-assisté-par-claude-code).*
