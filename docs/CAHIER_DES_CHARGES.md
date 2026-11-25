# Cahier des charges — Younji

> Client SSH / SFTP / RDP de bureau, dans l'esprit de MobaXterm et Termius.
> Younji (prononcé « younji ») est un nom inventé, libre en tant que marque dans le domaine logiciel. Il remplace « Vigie », déjà déposé (voir `docs/DECISIONS.md`, ADR-017).

| | |
|---|---|
| **Porteur du projet** | mlft9, étudiant en 3ᵉ année de coordination de projets informatiques, orientation cybersécurité |
| **Type de projet** | Projet personnel / portfolio |
| **Statut** | Cadrage validé, développement non commencé |
| **Dernière mise à jour** | 17/11/2026 |

---

## 1. Contexte et objectifs

### 1.1 Contexte
Les administrateurs systèmes et les profils cyber jonglent tous les jours avec des connexions SSH, des transferts de fichiers et des bureaux à distance. Les outils actuels ont chacun leurs limites :
- **MobaXterm** est très complet mais réservé à Windows, avec une interface datée. La version gratuite est limitée.
- **Termius** est moderne, mais les fonctions avancées sont payantes et la synchronisation passe par un cloud tiers.
- **PuTTY / KiTTY** sont austères : pas de coffre, pas de SFTP intégré.

### 1.2 Objectifs
1. Livrer une **application de bureau Windows et Linux** qui réunit SSH, SFTP, terminal local et RDP dans une interface moderne.
2. Faire de la **sécurité un argument central** : coffre chiffré, vérification stricte des clés d'hôte, durcissement de l'application, chaîne d'approvisionnement maîtrisée.
3. Produire un **projet de portfolio crédible** : code propre et testé, CI, documentation d'architecture, README illustré, publication open source (GPL-3.0).

### 1.3 Utilisateurs cibles
- **Principal** : le porteur du projet (admin de son lab Proxmox, étudiant cyber).
- **Secondaire** : administrateurs systèmes et réseau, pentesteurs, étudiants en informatique.

---

## 2. Périmètre

### 2.1 Plateformes
- **Windows 10 (22H2+) et Windows 11**, x64.
- **Linux** x64 : Ubuntu 22.04+, Debian 12+ et dérivées (paquets `.deb` et `AppImage`).
- Hors périmètre : macOS, ARM.

### 2.2 Langues
Interface **bilingue français / anglais**, détection de la langue du système, sélecteur dans les paramètres.

### 2.3 Hors périmètre (v1)
Ces éléments ont été écartés lors du cadrage. On pourra les reconsidérer plus tard :
- macOS, architectures ARM
- Serveur X11 intégré (fonction phare de MobaXterm)
- Telnet, port série, VNC, Mosh
- Synchronisation cloud ou serveur. Seul l'export/import chiffré est prévu.
- Import de sessions MobaXterm, PuTTY ou CSV. Seul l'import OpenSSH est prévu.
- Rebond par bastion (ProxyJump), agent forwarding, certificats SSH
- Tunnels SSH (redirections `-L`, `-R`, `-D`/SOCKS5) : prévus à l'origine à l'étape 6 (TUN-01 à TUN-03), retirés le 25/11/2026 car jugés non nécessaires et trop complexes pour le début (ADR-038)
- Agent SSH (Pageant, ssh-agent), sauf comme plan B pour FIDO2 (voir AUTH-04)
- Enregistrement des sessions, monitoring serveur (CPU/RAM)
- Thèmes et polices entièrement personnalisables. La v1 propose un thème clair et un thème sombre qui suivent le système, plus la taille de police.
- Lien avec le projet web `gestionnaire-ssh` : les deux projets sont indépendants.

---

## 3. Exigences fonctionnelles

Chaque exigence porte un identifiant et l'étape de la feuille de route (`docs/ROADMAP.md`) qui la livre.
**Priorité** : M = indispensable, S = souhaitable.

### 3.1 Application et interface (APP)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| APP-01 | Mise en page type Termius : barre latérale par sections (Hôtes, Clés, Snippets, SFTP, Paramètres) et zone principale à onglets | M | 0 |
| APP-02 | Interface FR/EN, langue détectée automatiquement et modifiable à chaud | M | 0 |
| APP-03 | Thème clair / sombre qui suit le système, avec forçage possible | M | 0 |
| APP-04 | Raccourcis clavier documentés : nouvel onglet, fermer, onglet suivant/précédent, verrouiller, rechercher | M | 1 |
| APP-05 | Paramètres persistants (langue, thème, police, délai de verrouillage, comportements) | M | 2 |
| APP-06 | Version portable : données stockées à côté de l'exécutable si un fichier marqueur est présent | M | 13 |
| APP-07 | Mises à jour automatiques signées, actives une fois le dépôt public | M | 13 |
| APP-08 | Journal applicatif rotatif, niveau configurable, sans aucun secret | M | 1 |

### 3.2 Terminal SSH (SSH)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| SSH-01 | Connexion rapide : hôte, port, utilisateur, mot de passe ou clé privée (avec passphrase) | M | 1 |
| SSH-02 | Terminal complet (xterm-256color, couleurs, Unicode, applications plein écran : vim, htop, tmux) | M | 1 |
| SSH-03 | Plusieurs sessions en onglets : ouvrir, fermer, renommer, réordonner | M | 1 |
| SSH-04 | Redimensionnement du terminal propagé au serveur | M | 1 |
| SSH-05 | Recherche dans le terminal, copier/coller (Ctrl+Maj+C/V, clic droit), liens cliquables | M | 1 |
| SSH-06 | Keepalive, délais d'attente, détection de déconnexion avec bouton « Reconnecter » | M | 1 |
| SSH-07 | Écran partagé (split) horizontal et vertical dans un onglet, redimensionnable | M | 5 |
| SSH-08 | Diffusion de la saisie vers plusieurs sessions sélectionnées, avec indicateur visuel fort | M | 5 |

### 3.3 Clés d'hôte (HKY)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| HKY-01 | Confiance à la première connexion (TOFU) : affichage du type de clé et de l'empreinte SHA-256, acceptation explicite | M | 1 |
| HKY-02 | Si la clé d'un hôte connu change : **connexion bloquée**, alerte claire (attaque de l'homme du milieu possible). Pour débloquer, il faut supprimer volontairement l'ancienne clé. | M | 1 |
| HKY-03 | Fichier `known_hosts` propre à l'application, au format OpenSSH | M | 1 |
| HKY-04 | Gestion des clés d'hôte connues dans l'interface : liste, suppression | S | 2 |

### 3.4 Coffre et carnet d'hôtes (VLT / HST)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| VLT-01 | Création du coffre avec un mot de passe maître et un indicateur de robustesse | M | 2 |
| VLT-02 | Chiffrement de toutes les données sensibles (identités, mots de passe, clés, snippets). Détails dans `docs/SECURITE.md`. | M | 2 |
| VLT-03 | Verrouillage manuel (raccourci) et automatique après une inactivité configurable | M | 2 |
| VLT-04 | Changement du mot de passe maître | M | 2 |
| VLT-05 | Déverrouillage rapide par Windows Hello (Windows) ou le trousseau du système (Linux), en option | M | 10 |
| VLT-06 | Export / import d'une sauvegarde chiffrée avec une phrase de passe distincte (fusion ou remplacement) | M | 9 |
| HST-01 | Carnet d'hôtes : nom, adresse, port, utilisateur, identité, groupe, tags, notes | M | 2 |
| HST-02 | Groupes (dossiers), recherche instantanée, double-clic pour se connecter | M | 2 |
| HST-03 | Identités réutilisables (utilisateur + mot de passe et/ou clé) partagées entre hôtes | M | 2 |

### 3.5 Authentification et clés (AUTH / KEY)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| AUTH-01 | Mot de passe | M | 1 |
| AUTH-02 | Clé publique : Ed25519, ECDSA, RSA (SHA-2), avec passphrase | M | 1 |
| AUTH-03 | Keyboard-interactive / 2FA : affichage des questions du serveur (code TOTP…) | M | 3 |
| AUTH-04 | Clés matérielles FIDO2 (`sk-ssh-ed25519`, `sk-ecdsa`), comme les YubiKey. Plan B : passer par l'agent OpenSSH. | M | 11 |
| KEY-01 | Générateur de clés : Ed25519 par défaut, ECDSA, RSA 4096, avec passphrase optionnelle | M | 3 |
| KEY-02 | Import d'une clé privée existante dans le coffre, export de la clé publique | M | 2–3 |
| KEY-03 | Déploiement d'une clé publique sur un hôte (`authorized_keys`), idempotent, permissions correctes | M | 3 |

### 3.6 SFTP (SFTP)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| SFTP-01 | Panneau SFTP à côté du terminal, qui suit le dossier courant du shell | M | 4 |
| SFTP-02 | Onglet SFTP double panneau local / distant avec glisser-déposer | M | 4 |
| SFTP-03 | Opérations : lister, envoyer, télécharger (fichiers et dossiers), renommer, supprimer, créer un dossier, chmod | M | 4 |
| SFTP-04 | File de transferts avec progression, annulation et gestion des conflits | M | 4 |
| SFTP-05 | Ouvrir un fichier distant dans l'éditeur local et le renvoyer automatiquement à l'enregistrement | S | 4 |

### 3.7 Snippets (SNP)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| SNP-01 | Bibliothèque de snippets (CRUD), stockée dans le coffre | M | 5 |
| SNP-02 | Variables `{{nom}}` demandées au moment de l'exécution | S | 5 |
| SNP-03 | Envoi vers la session active ou vers plusieurs sessions | M | 5 |

### 3.8 Tunnels SSH (TUN) — retirés
Les exigences TUN-01 à TUN-03 sont retirées du périmètre (ADR-038, voir §2.3). Leurs identifiants ne sont pas réattribués.

### 3.9 Terminal local (LOC)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| LOC-01 | Windows : PowerShell, PowerShell 7, CMD, distributions WSL détectées automatiquement | M | 7 |
| LOC-02 | Linux : shell par défaut de l'utilisateur, bash, zsh | M | 7 |

### 3.10 RDP (RDP)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| RDP-01 | Hôtes RDP dans le carnet (adresse, port, utilisateur, domaine, résolution) | M | 8 |
| RDP-02 | v1 : lancement du client du système (mstsc sous Windows, xfreerdp sous Linux) **sans jamais passer le mot de passe en ligne de commande** | M | 8 |
| RDP-03 | v2 : RDP natif dans un onglet (IronRDP) : affichage, clavier/souris, presse-papiers texte, redimensionnement, NLA | M | 12 |
| RDP-04 | Vérification du certificat du serveur RDP (TOFU, même logique que pour SSH) | M | 12 |

### 3.11 Import (IMP)
| ID | Exigence | Prio | Étape |
|---|---|---|---|
| IMP-01 | Import de `~/.ssh/config` (Host, HostName, User, Port, IdentityFile, Include), avec un rapport des entrées ignorées | M | 9 |
| IMP-02 | Import des clés d'hôte depuis `~/.ssh/known_hosts` | S | 9 |

---

## 4. Exigences non fonctionnelles

### 4.1 Sécurité
Le détail et la justification sont dans `docs/SECURITE.md`. Ces règles ne sont pas négociables :
- Aucun secret en clair sur le disque, dans les journaux, dans les arguments de processus ou dans les messages d'erreur.
- Les secrets transitent le moins possible par l'interface web. Le backend Rust les détient et l'interface les désigne par identifiant.
- Clés d'hôte vérifiées systématiquement (HKY-01/02).
- Application Tauri durcie : CSP stricte, permissions minimales, aucun contenu distant.
- Chaîne d'approvisionnement contrôlée : lockfiles, `cargo-deny`, `npm audit`, actions CI épinglées par SHA, détection de secrets.
- Aucune télémétrie.

### 4.2 Performance
- Démarrage à froid en moins de 2 s sur un PC récent (indicatif).
- Un `cat` d'un gros fichier ou un `yes` ne doit pas figer l'interface. Le flux du terminal est régulé.
- Transfert SFTP de 1 Go sans blocage de l'interface ni explosion de la mémoire (traitement en flux).
- Mémoire raisonnable avec 10 onglets ouverts (indicatif : moins de 400 Mo).

### 4.3 Robustesse
- Une coupure réseau ou un serveur qui ferme la connexion ne doit jamais faire planter l'application. La session passe à l'état « déconnectée ».
- Écritures du coffre atomiques (fichier temporaire puis renommage) et sauvegarde de la version précédente.
- Erreurs typées côté Rust et traduites côté interface (codes d'erreur, pas de texte en dur).

### 4.4 Ergonomie et accessibilité
- Tout est utilisable au clavier, avec un focus visible.
- Contrastes conformes WCAG AA pour l'interface (hors contenu du terminal).
- Messages d'erreur compréhensibles, avec la cause probable et une action possible.

### 4.5 Maintenabilité et qualité
- Logique métier en crates Rust indépendantes de Tauri, donc testables seules.
- Tests unitaires et tests d'intégration contre un vrai serveur SSH (conteneur en CI).
- CI à chaque push : formatage, lint, typage, tests, audit des dépendances.
- Documentation tenue à jour à chaque étape : architecture, décisions, journal des modifications.

### 4.6 Licence
- GPL-3.0 à la publication. Les dépendances doivent avoir une licence compatible, vérifiée par `cargo-deny` et par la CI.

---

## 5. Contraintes techniques
- **Tauri 2** (version stable). Pas de Tauri 3 alpha.
- Backend **Rust** : `russh` (SSH), `russh-sftp`, `portable-pty` (terminal local), `ironrdp` (RDP natif), crates RustCrypto (`argon2`, `chacha20poly1305`), `zeroize` / `secrecy`, `keyring`.
- Frontend : **React + TypeScript (strict) + Vite + Tailwind CSS + shadcn/ui**, `@xterm/xterm`, `zustand`, `i18next`.
- Dépôt **GitHub privé**, CI **GitHub Actions**. Les compilations Windows se font à la demande pour économiser les minutes.
- Développement natif sous Windows (pas sous WSL) pour produire les binaires Windows.

## 6. Livrables
1. Code source sur GitHub, avec un historique de commits propre.
2. Installeurs Windows (`.exe` NSIS, `.msi`) et Linux (`.deb`, `.AppImage`), plus une version portable.
3. Documentation : README illustré, `docs/ARCHITECTURE.md`, `docs/SECURITE.md`, `docs/DECISIONS.md`, `CHANGELOG.md`.
4. CI qualité et compilation.

## 7. Méthode de travail
- Développement par **étapes courtes** (voir `docs/ROADMAP.md`). Chaque étape aboutit à une application testable.
- À la fin de chaque étape, Claude Code fournit un **plan de test manuel**. Le porteur teste sur son lab Proxmox (voir `docs/ENVIRONNEMENT_DE_TEST.md`) et fait ses retours avant l'étape suivante.
- Toute décision structurante est consignée dans `docs/DECISIONS.md`.

## 8. Risques identifiés
| Risque | Impact | Parade |
|---|---|---|
| RDP natif (IronRDP) complexe à intégrer | Fort | RDP en deux temps : client système d'abord (étape 8), natif en fin de parcours (étape 12) |
| Support FIDO2 incomplet dans l'écosystème Rust | Moyen | Étude de faisabilité au début de l'étape 11, plan B via l'agent OpenSSH |
| Minutes CI limitées en dépôt privé (Windows compte double) | Faible | Compilations Windows à la demande, tests sur Linux à chaque push |
| Avertissement SmartScreen (binaire non signé) | Faible | Accepté en privé ; demande à la SignPath Foundation à la publication |
| Suivi du dossier courant du shell pour le SFTP | Moyen | Séquence OSC 7 si le shell l'émet, intégration au prompt en option |
| Fuite de secrets via l'interface web ou les journaux | Fort | Secrets confinés au backend, types `Secret`, revue de sécurité à chaque étape |
