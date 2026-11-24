# Feuille de route

Chaque étape produit une application **testable**. On ne passe à l'étape suivante qu'après les retours de test du porteur.
Les identifiants (SSH-01, VLT-02…) renvoient au `docs/CAHIER_DES_CHARGES.md`.

**Légende** : ⬜ à faire · 🟨 en cours · ✅ terminée (testée et validée par le porteur)

| # | Étape | Statut |
|---|---|---|
| 0 | Fondations | ✅ |
| 1 | Terminal SSH multi-onglets | ✅ |
| 2 | Coffre chiffré et carnet d'hôtes | ✅ |
| 3 | 2FA et générateur de clés | ✅ |
| 4 | SFTP | ✅ |
| 5 | Split, snippets, diffusion multi-sessions | 🟨 |
| 6 | Tunnels SSH | ⬜ |
| 7 | Terminal local | ⬜ |
| 8 | RDP v1 (client système) | ⬜ |
| 9 | Import OpenSSH, export/import chiffré | ⬜ |
| 10 | Déverrouillage par le système | ⬜ |
| 11 | Clés FIDO2 | ⬜ |
| 12 | RDP natif (IronRDP) | ⬜ |
| 13 | Finitions et publication | ⬜ |
| **Phase 2** | **Younji, le client qui protège** | |
| 14 | Observer et tracer | ⬜ |
| 15 | Modification sûre, durcissement SSH et Proxmox | ⬜ |
| 16 | Garde-fous de session | ⬜ |
| 17 | Hygiène des accès | ⬜ |
| 18 | Confort et démo | ⬜ |
| 19 | Audit système | ⬜ |
| 20 | Suivi dans le temps | ⬜ |
| 21 | Édition et comparaison de configs | ⬜ |
| 22 | Export d'inventaire Ansible | ⬜ |

**Définition de « terminé », valable pour toutes les étapes :**
- `npm run check` passe en local et la CI est verte.
- Les nouvelles fonctions ont des tests (unitaires et, côté SSH, d'intégration).
- Toutes les chaînes de l'interface existent en FR et en EN.
- L'agent `security-reviewer` n'a relevé aucun problème bloquant.
- La documentation est à jour : `ARCHITECTURE.md`, `DECISIONS.md` si une décision a été prise, `CHANGELOG.md`, le statut dans ce fichier.
- Un plan de test manuel a été fourni et le porteur a validé l'étape.

---

## Étape 0 — Fondations
**But** : un squelette solide, durci et outillé, sur lequel tout le reste viendra se greffer.

Contenu :
- Projet Tauri 2 (stable) + React + TypeScript strict + Vite + Tailwind + shadcn/ui + Zustand.
- Workspace Cargo : `src-tauri/` (application, fine) et `crates/` (logique métier). Voir `docs/ARCHITECTURE.md`.
- i18n FR/EN (i18next) : détection de la langue du système, sélecteur, fichiers `src/locales/{fr,en}.json`. APP-02.
- Mise en page vide type Termius : barre latérale, zone d'onglets, thème clair/sombre. APP-01, APP-03.
- Durcissement Tauri : CSP stricte, capabilities minimales, pas d'API globale `window.__TAURI__`.
- Journalisation (`tracing`) vers un fichier rotatif. APP-08 (base).
- Outillage : rustfmt, clippy (`-D warnings`), `cargo-deny` (licences compatibles GPL-3.0, vulnérabilités, sources), ESLint, Prettier, `tsc --noEmit`, Vitest.
- Scripts npm : `check`, `lint`, `test`, `check:rust`, `test:rust`, `audit` (voir `AGENTS.md`).
- CI :
  - `ci.yml` : à chaque push et PR, sous Linux : formatage, lint, types, tests, `cargo-deny`, `npm audit`, gitleaks.
  - `build.yml` : manuel (`workflow_dispatch`) et sur tag `v*` : installeurs Windows (NSIS + MSI) et Linux (deb + AppImage) en artefacts.
  - Dependabot (cargo, npm, actions). Actions épinglées par SHA, permissions minimales.

**Critères d'acceptation**
- [x] `npm run tauri dev` ouvre la fenêtre sous Windows.
- [x] La langue bascule FR ↔ EN à chaud.
- [x] Le thème suit le système et peut être forcé.
- [x] `npm run check` passe et la CI est verte.
- [x] Le workflow `build.yml` lancé à la main produit des installeurs Windows et Linux qui s'installent et se lancent.

---

## Étape 1 — Terminal SSH multi-onglets
**But** : se connecter en SSH et travailler confortablement dans plusieurs onglets.

Contenu :
- Crate `core-ssh` : connexion russh, authentification par mot de passe ou par clé (Ed25519/ECDSA/RSA-SHA2, avec passphrase), PTY `xterm-256color`, shell, flux d'entrée/sortie, redimensionnement, fermeture propre, keepalive, délais d'attente. SSH-01, SSH-02, SSH-04, SSH-06, AUTH-01, AUTH-02.
- Vérification des clés d'hôte TOFU avec `known_hosts` propre à l'application. HKY-01, HKY-02, HKY-03.
- Commandes Tauri fines + flux de données via `tauri::ipc::Channel` (pas d'événements globaux pour les données du terminal).
- Interface : formulaire de connexion rapide, onglets (ouvrir, fermer, renommer, réordonner), xterm.js (fit, search, web-links), copier/coller, raccourcis. SSH-03, SSH-05, APP-04.
- Dialogue de clé d'hôte inconnue (type + empreinte SHA-256) et alerte bloquante en cas de changement.
- Tests d'intégration contre un `sshd` en conteneur (CI) ou une VM du lab (en local).

**Critères d'acceptation**
- [x] Connexion à la VM Debian du lab, par mot de passe puis par clé avec passphrase.
- [x] vim, htop, tmux et les couleurs s'affichent correctement. Le redimensionnement de la fenêtre est répercuté.
- [x] Trois onglets ouverts en parallèle sur des hôtes différents.
- [x] Première connexion : l'empreinte est affichée et doit être acceptée.
- [x] Après régénération des clés d'hôte de la VM : connexion **bloquée** avec une alerte claire.
- [x] Couper le réseau de la VM : l'onglet passe à « déconnecté » et propose de se reconnecter, sans planter.
- [x] `cat` d'un gros fichier : l'interface reste réactive.

---

## Étape 2 — Coffre chiffré et carnet d'hôtes
**But** : enregistrer ses hôtes et identités en sécurité.

Contenu :
- Crate `core-vault` : schéma cryptographique de `docs/SECURITE.md` (Argon2id → KEK, DEK aléatoire, XChaCha20-Poly1305, en-tête authentifié), écriture atomique + `.bak`, format versionné. VLT-01, VLT-02, VLT-04.
- Modèle : hôtes, groupes, identités, clés, clés d'hôte connues. HST-01 à HST-03, HKY-04, KEY-02 (import).
- Interface : création/déverrouillage du coffre, carnet d'hôtes (groupes, recherche, CRUD, double-clic pour se connecter), section Clés, paramètres. APP-05.
- Verrouillage manuel et automatique. VLT-03. Décision à consigner : que deviennent les sessions ouvertes au verrouillage ?

**Critères d'acceptation**
- [x] Au premier lancement, création du coffre. Aux lancements suivants, il est verrouillé.
- [x] Un mauvais mot de passe est refusé, sans indice sur la cause.
- [x] Le fichier du coffre ouvert dans un éditeur ne montre aucune donnée lisible (ni nom d'hôte, ni utilisateur).
- [x] Le verrouillage automatique fonctionne avec le délai configuré.
- [x] Le changement de mot de passe maître fonctionne, et l'ancien mot de passe ne marche plus.
- [x] Une connexion depuis le carnet utilise l'identité enregistrée sans rien redemander.

---

## Étape 3 — 2FA et générateur de clés
Contenu : keyboard-interactive (AUTH-03), générateur de clés (KEY-01), export de la clé publique, déploiement sur un hôte (KEY-03).

**Critères d'acceptation**
- [x] Connexion à une VM configurée avec mot de passe + TOTP (PAM google-authenticator).
- [x] Générer une clé Ed25519, la déployer sur la VM, puis se connecter par clé. Un second déploiement ne duplique pas la ligne.
- [x] Les permissions de `~/.ssh` (700) et de `authorized_keys` (600) sont correctes après le déploiement.

---

## Étape 4 — SFTP
Contenu : `russh-sftp` ; panneau latéral qui suit le shell (OSC 7, intégration au prompt en option) ; onglet double panneau ; opérations, file de transferts, conflits ; édition d'un fichier distant avec l'éditeur local. SFTP-01 à SFTP-05.
Point de sécurité : noms de fichiers distants malveillants (`../`, caractères de contrôle), liens symboliques, écrasements.

**Critères d'acceptation**
- [x] Un `cd` dans le terminal fait suivre le panneau (avec un shell compatible OSC 7 ou l'intégration activée).
- [x] Envoi et téléchargement d'un dossier récursif contenant des accents, des espaces et des fichiers vides.
- [x] Un fichier de 1 Go se transfère avec progression, peut être annulé, et l'interface reste fluide.
- [x] Un fichier distant nommé de façon malveillante ne peut pas écrire en dehors du dossier local choisi.

---

## Étape 5 — Split, snippets, diffusion multi-sessions
Contenu : split horizontal/vertical (SSH-07), snippets avec variables (SNP-01 à SNP-03), diffusion de la saisie (SSH-08) avec bordure d'alerte et confirmation à l'activation.

**Critères d'acceptation**
- [ ] Quatre terminaux en grille dans un onglet, redimensionnables, avec navigation au clavier.
- [ ] Un snippet avec `{{service}}` demande la valeur avant envoi.
- [ ] La diffusion vers trois sessions envoie les mêmes frappes, le mode est impossible à manquer visuellement, et il se désactive en un clic.

---

## Étape 6 — Tunnels SSH
Contenu : `-L`, `-R`, `-D` (SOCKS5), gestionnaire de tunnels, `127.0.0.1` par défaut avec avertissement pour `0.0.0.0`. TUN-01 à TUN-03.

**Critères d'acceptation**
- [ ] `-L` : un service web qui écoute sur `127.0.0.1:8080` dans la VM est joignable depuis le PC.
- [ ] `-D` : Firefox configuré avec le proxy SOCKS navigue via la VM.
- [ ] `-R` : un service du PC est joignable depuis la VM.
- [ ] L'état et le nombre de connexions sont visibles. L'arrêt libère le port.

---

## Étape 7 — Terminal local
Contenu : crate `core-term` (`portable-pty`, ConPTY sous Windows), profils détectés (PowerShell, pwsh, CMD, WSL ; bash/zsh sous Linux). LOC-01, LOC-02.

**Critères d'acceptation**
- [ ] Les profils installés sont détectés. PowerShell et une distribution WSL s'ouvrent dans un onglet.
- [ ] Redimensionnement et couleurs corrects. Les applications plein écran fonctionnent.

---

## Étape 8 — RDP v1 (client système)
Contenu : hôtes RDP dans le carnet (RDP-01). Sous Windows : fichier `.rdp` temporaire + identifiants via le gestionnaire d'identifiants Windows, supprimés après usage → `mstsc`. Sous Linux : détection de `xfreerdp`/`xfreerdp3`, mot de passe transmis par l'entrée standard. RDP-02.

**Critères d'acceptation**
- [ ] Connexion à la VM Windows du lab depuis Windows et depuis Linux.
- [ ] Le mot de passe n'apparaît **jamais** dans la liste des processus (vérifier avec `ps aux` ou le Gestionnaire des tâches).
- [ ] Les fichiers et identifiants temporaires sont supprimés après le lancement.

---

## Étape 9 — Import OpenSSH, export/import chiffré
Contenu : crate `core-import`, import de `~/.ssh/config` avec rapport (IMP-01), de `known_hosts` (IMP-02), sauvegarde chiffrée avec une phrase de passe distincte (VLT-06).

**Critères d'acceptation**
- [ ] Un `~/.ssh/config` réel (avec `Include` et des jokers) est importé. Les entrées ignorées sont listées avec leur raison.
- [ ] Export, puis import sur une autre machine ou un profil vierge : données identiques.
- [ ] Une sauvegarde modifiée d'un seul octet est rejetée (intégrité).

---

## Étape 10 — Déverrouillage par le système
Contenu : VLT-05. Windows Hello + protection de la DEK par le système (gestionnaire d'identifiants / DPAPI), trousseau Secret Service sous Linux (`keyring`). Opt-in. Le modèle de menace est à documenter dans `SECURITE.md`. Attention : un simple consentement Windows Hello n'est qu'une barrière d'interface, la protection réelle vient du stockage protégé par le système.

**Critères d'acceptation**
- [ ] Une fois l'option activée, le coffre se déverrouille avec Windows Hello (PIN, empreinte, visage).
- [ ] Désactiver l'option efface la DEK du stockage système.
- [ ] Le mot de passe maître reste toujours utilisable en secours.

---

## Étape 11 — Clés FIDO2
Contenu : **étude de faisabilité d'abord** (support `sk-*` dans russh ou via une crate CTAP2, compatibilité Windows), à consigner dans `DECISIONS.md`. Ensuite, implémentation native ou plan B via l'agent OpenSSH. AUTH-04.

**Critères d'acceptation**
- [ ] Génération d'une clé `sk-ssh-ed25519` sur une YubiKey (ou utilisation d'une clé existante).
- [ ] Connexion qui demande le toucher de la clé.

---

## Étape 12 — RDP natif (IronRDP)
Contenu : crate `core-rdp`, rendu dans un onglet (canvas, envoi de régions modifiées), clavier/souris, presse-papiers texte, redimensionnement, NLA/CredSSP, vérification TOFU du certificat. RDP-03, RDP-04.

**Critères d'acceptation**
- [ ] Session Windows utilisable dans un onglet : fluidité acceptable sur le réseau local, clavier AZERTY correct.
- [ ] Copier-coller de texte dans les deux sens.
- [ ] Un changement de certificat du serveur déclenche une alerte.

---

## Étape 13 — Finitions et publication
> Avancés à la fin de l'étape 0 (ADR-016) : `LICENSE` GPL-3.0, `SECURITY.md` et passage du dépôt en public.

Contenu : version portable (APP-06), mises à jour signées (APP-07, clés dans les secrets GitHub), README illustré (captures, GIF), `ARCHITECTURE.md` finalisé, demande de signature à la SignPath Foundation, vérification que le nom « Younji » n'est pas une marque déposée dans le domaine logiciel (INPI, EUIPO) avant la publication.

**Critères d'acceptation**
- [ ] La version portable fonctionne depuis une clé USB, sans rien écrire dans le profil utilisateur.
- [ ] Une mise à jour publiée est détectée, vérifiée (signature) et installée.
- [ ] Le README permet à un inconnu d'installer et de comprendre le projet en 5 minutes.

---

# Phase 2 — Younji, le client qui protège

**Ambition** : se démarquer des autres clients SSH en devenant l'outil qui **audite, durcit et prévient**, pas seulement celui qui connecte. Ces étapes viennent du tri des idées du 18/11/2026 : 28 idées retenues, 8 écartées (Wake-on-LAN, expiration des certificats TLS, runbooks, découverte réseau, mini autorité de certification SSH, SSHFP, carnet de lab chiffré, écouteur de reverse shell). Les codes entre parenthèses (A1, G2…) renvoient à ce tri.

**Quand les faire** : la numérotation ne fixe pas l'ordre de réalisation. Ces étapes peuvent être intercalées dès le **jalon présentable** (étapes 0 à 4 terminées), à condition que les étapes indiquées dans « Dépend de » soient validées. L'ordre de la phase 2 suit la priorité recommandée : ce qui est visible et sans risque d'abord, puis ce qui modifie les serveurs.

**Règle commune à toute fonction qui modifie un serveur** (étapes 15, 17, 21, 22) : elle passe par le **processus de modification sûre** livré à l'étape 15 (aperçu, confirmation, sauvegarde, application, test dans une nouvelle connexion, retour arrière automatique). Aucune exception.

---

## Étape 14 — Observer et tracer
**But** : montrer dès la connexion ce qui est faible, et garder une trace fiable de tout ce que Younji fait.
**Dépend de** : 1, 2.

Contenu :
- **Audit SSH passif (A1)** : pendant la négociation, relever les algorithmes proposés par le serveur et signaler ceux qui sont faibles (SHA-1, CBC, groupes Diffie-Hellman courts, clés RSA courtes). Aucune commande exécutée côté serveur. Résultat visible dans l'onglet et sur la fiche de l'hôte.
- **Journal d'actions infalsifiable (S6)** : chaque action de Younji qui touche un serveur ou un secret (connexion, clé déployée, configuration modifiée…) est consignée dans un journal chaîné par hachage. Chaque entrée inclut le hachage de la précédente, et la chaîne est authentifiée par une clé tirée du coffre.

Points de vigilance :
- L'audit passif est purement informatif : aucune action automatique.
- Le journal ne contient aucun secret ni contenu de terminal. Un hachage chaîné seul se recalcule entièrement : c'est l'authentification par la clé du coffre qui rend une falsification détectable. Ce choix est à consigner dans `DECISIONS.md` et `SECURITE.md`.

**Critères d'acceptation**
- [ ] Connexion à une VM configurée volontairement avec des algorithmes faibles : ils sont listés avec une explication. Sur une VM saine, aucun avertissement.
- [ ] Les résultats correspondent à ceux de l'outil `ssh-audit` sur la même VM.
- [ ] Modifier une ligne du journal à la main : Younji signale l'altération au prochain démarrage.

---

## Étape 15 — Modification sûre, durcissement SSH et Proxmox
**But** : durcir un serveur en un clic, sans jamais risquer de s'en couper l'accès.
**Dépend de** : 2, 14.

Contenu :
- **Processus de modification sûre** (socle réutilisé par les étapes 17, 21 et 22) : aperçu des changements → confirmation explicite → sauvegarde de la configuration (et snapshot si Proxmox) → application avec un minuteur de retour arrière → test dans une **nouvelle** connexion, la session actuelle restant ouverte → validation, ou retour arrière automatique si le test échoue ou si le minuteur expire. Chaque étape est inscrite au journal (S6).
- **Audit + durcissement SSH (A2)** : lecture de la configuration effective (`sshd -T`), score, explication de chaque point (`PermitRootLogin`, `PasswordAuthentication`, `MaxAuthTries`…), puis application d'un profil durci, validé par `sshd -t` avant tout redémarrage du service.
- **Import des VM via l'API Proxmox (P1)** : ajout des VM au carnet d'hôtes, démarrage d'une VM éteinte avant connexion.
- **Snapshot automatique avant durcissement (P2)** : snapshot Proxmox avant toute modification, avec retour au snapshot en un clic.

Points de vigilance :
- Jeton d'API Proxmox dédié, aux droits minimaux, stocké dans le coffre. Certificat TLS de Proxmox vérifié en TOFU, comme les clés d'hôte.
- Ne jamais désactiver l'authentification par mot de passe sans avoir vérifié qu'une connexion par clé fonctionne.

**Critères d'acceptation**
- [ ] Durcissement d'une VM Debian : score avant/après affiché, nouvelle connexion réussie, journal à jour.
- [ ] Profil volontairement cassé (port SSH erroné, par exemple) : le test échoue et la configuration d'origine revient seule avant la fin du minuteur.
- [ ] Les VM du Proxmox apparaissent dans le carnet. Un snapshot est créé avant le durcissement et peut être restauré depuis Younji.
- [ ] Fermer Younji en plein durcissement : le minuteur côté serveur restaure quand même la configuration.

---

## Étape 16 — Garde-fous de session
**But** : réduire les erreurs humaines (mauvais serveur, commande de trop, secret collé au mauvais endroit).
**Dépend de** : 2. La coupure des tunnels du bouton panique s'active dès que l'étape 6 est livrée.

Contenu :
- **Étiquettes d'environnement (G1)** : prod, préprod, lab… avec une couleur visible sur l'onglet, la bordure du terminal et la barre de titre.
- **Confirmation des commandes dangereuses (G2)** : `rm -rf`, `shutdown`, `reboot`, `iptables -F`, `DROP TABLE`… demandent une confirmation sur un hôte étiqueté prod. Liste modifiable.
- **Alerte anti-fuite au collage (G3)** : avertissement quand le texte collé ressemble à un mot de passe, une clé privée ou un jeton d'API.
- **Bouton panique (S7)** : un raccourci ferme toutes les sessions, coupe les tunnels et verrouille le coffre.
- **Verrouillage lié à la session Windows (S8)** : le coffre se verrouille quand Windows est verrouillé ou que le PC se met en veille (et l'équivalent sous Linux quand il est disponible).

Points de vigilance :
- G2 est une **aide**, pas une barrière : un alias, un script ou l'historique du shell la contournent. La documentation doit le dire clairement.
- G3 analyse le texte en local, sans jamais le journaliser ni le conserver.

**Critères d'acceptation**
- [ ] Un hôte étiqueté prod est identifiable d'un coup d'œil, dans tous les onglets et panneaux.
- [ ] `sudo reboot` tapé sur un hôte prod demande une confirmation. Sur un hôte lab, rien.
- [ ] Coller une clé privée de test dans le terminal déclenche l'alerte avant l'envoi.
- [ ] Le bouton panique laisse l'application verrouillée, sans session ni tunnel ouvert.
- [ ] Verrouiller Windows (Win+L) verrouille le coffre.

---

## Étape 17 — Hygiène des accès
**But** : savoir qui peut entrer où, et garder des clés saines.
**Dépend de** : 3, 15.

Contenu :
- **Inventaire des accès (H1)** : lecture des `authorized_keys` de tous les serveurs, croisée avec les clés du coffre : clés inconnues, orphelines ou présentes sur trop de machines.
- **Rotation de clé en masse (H2)** : générer une nouvelle clé, la déployer, vérifier la connexion avec elle, puis seulement retirer l'ancienne, serveur par serveur, via le processus de modification sûre.
- **Âge des clés et des mots de passe (H3)** : date de création ou de dernier changement de chaque secret, avec un rappel au-delà d'un seuil réglable. Le rappel propose directement la rotation (H2).
- **Clés à durée limitée (S1)** : déploiement d'une clé qui expire d'elle-même (`expiry-time` dans `authorized_keys`, OpenSSH 8.2 et plus).

Points de vigilance :
- Une clé inconnue peut être légitime (collègue, outil d'automatisation) : on la signale, on ne la supprime jamais automatiquement.
- L'ancienne clé n'est retirée d'un serveur qu'après une connexion réussie avec la nouvelle **sur ce même serveur**.

**Critères d'acceptation**
- [ ] Ajouter à la main une clé inconnue sur une VM : l'inventaire la signale.
- [ ] Rotation sur trois VM : nouvelle clé fonctionnelle partout, ancienne clé retirée, journal à jour. Si une VM est injoignable, l'ancienne clé y reste et l'échec est signalé.
- [ ] Une clé déployée avec une expiration à J+1 est refusée par le serveur le surlendemain.

---

## Étape 18 — Confort et démo
**But** : les détails qui font gagner du temps tous les jours, et un mode pensé pour les démos.
**Dépend de** : 1, 2. À faire de préférence **avant l'étape 13**, pour les captures du README.

Contenu :
- **Mode présentation (C1)** : masquage automatique des IP, noms d'hôtes et utilisateurs à l'écran, y compris dans le contenu du terminal, avec un indicateur visible quand le mode est actif.
- **Notification de fin de commande (E3)** : notification système quand une commande longue se termine dans un onglet inactif.
- **Sessions qui survivent aux coupures (E4)** : intégration de tmux en option, pour retrouver son travail après une perte de connexion.
- **Palette de commandes (E5)** : `Ctrl+K` pour tout faire au clavier (se connecter, ouvrir un tunnel, lancer un snippet, changer de thème).
- **Menus contextuels de l'application** (retour de recette de l'étape 5) : un menu clic droit propre à Younji sur les éléments de l'interface (hôte du carnet : se connecter, ouvrir en SFTP, modifier ; onglet : renommer, fermer, diviser ; champs de saisie : couper, copier, coller), à la place du menu natif de la WebView, bloqué depuis l'étape 5 hors des champs de saisie.

Points de vigilance :
- Le mode présentation masque au mieux, sans garantie absolue (un serveur peut afficher une IP sous une forme inattendue). Il faut le dire dans l'interface.

**Critères d'acceptation**
- [ ] En mode présentation, une capture d'écran d'une session complète ne laisse voir aucune IP ni aucun nom d'hôte du carnet.
- [ ] Un `sleep 30` dans un onglet inactif déclenche une notification à la fin.
- [ ] Couper le réseau puis le rétablir : la session tmux est retrouvée intacte.
- [ ] Toutes les actions principales sont accessibles depuis `Ctrl+K`.

---

## Étape 19 — Audit système
**But** : évaluer la sécurité d'un serveur au-delà de SSH, et repérer les signes d'attaque.
**Dépend de** : 2, 14. L'installation éventuelle de Lynis passe par l'étape 15.

Contenu :
- **Audit système léger (A3)** : mises à jour en attente, redémarrage nécessaire, pare-feu actif ou non, ports en écoute expliqués, services exposés sur toutes les interfaces, `sudo` sans mot de passe, fail2ban ou CrowdSec présent. **Lynis** est lancé à la demande s'il est présent, et ses résultats sont affichés.
- **Référentiel ANSSI (A4)** : chaque contrôle est rattaché aux recommandations ANSSI de configuration d'un système GNU/Linux.
- **Tentatives de connexion échouées (S4)** : tentatives récentes et IP les plus insistantes, lues dans les journaux SSH du serveur.
- **Contrôle d'intégrité des binaires (S5)** : vérification de `sshd`, `sudo`, `ls`… par rapport à leurs paquets (`debsums`, `rpm -Va`).

Points de vigilance :
- Les audits sont en lecture seule. Toute correction proposée passe par l'étape 15.
- S5 est un signal, pas une preuve d'innocence : un attaquant root peut aussi falsifier l'outil de vérification.
- Les résultats d'audit décrivent les faiblesses des serveurs : ils sont stockés dans le coffre chiffré.

**Critères d'acceptation**
- [ ] Sur une VM avec un port ouvert inutile et un `sudo` sans mot de passe, les deux sont signalés avec la recommandation ANSSI correspondante.
- [ ] Après une série de connexions ratées depuis une autre VM, son IP apparaît en tête de liste.
- [ ] Remplacer un binaire de test par une copie modifiée : le contrôle d'intégrité le signale.

---

## Étape 20 — Suivi dans le temps
**But** : voir l'état de toute la flotte et ce qui a changé depuis la dernière fois.
**Dépend de** : 19.

Contenu :
- **Détection de dérive (A5)** : à chaque connexion ou audit, comparaison avec le précédent (« nouveau port ouvert depuis le 12/11 », « authentification par mot de passe réactivée »).
- **Rapport exportable (A6)** : rapport Markdown ou PDF par hôte (constats, gravité, corrections appliquées).
- **Tableau de bord de la flotte (Q1)** : OS, noyau, uptime, espace disque, redémarrage nécessaire, score d'audit, avec un badge par hôte dans le carnet. Collecte à la connexion ou à la demande, jamais en tâche de fond cachée.

Points de vigilance :
- Un rapport exporté est un document sensible : avertissement à l'export, et nom de fichier sans données identifiantes par défaut.

**Critères d'acceptation**
- [ ] Ouvrir un port sur une VM entre deux audits : la dérive est signalée avec sa date.
- [ ] Le rapport d'une VM s'ouvre correctement et reprend les constats de l'audit.
- [ ] Le tableau de bord affiche l'état de toutes les VM du lab en un écran.

---

## Étape 21 — Édition et comparaison de configs
**But** : modifier un fichier de configuration distant sans pouvoir casser le service.
**Dépend de** : 4, 15.

Contenu :
- **Édition sûre des configs (E1)** : éditeur intégré (coloration syntaxique) pour les fichiers distants. La validation (`nginx -t`, `sshd -t`, `apachectl configtest`…) est lancée avant l'enregistrement, et une configuration cassée est refusée. Les fichiers critiques passent par le processus de modification sûre.
- **Diff entre serveurs (E2)** : comparaison du même fichier sur deux machines, différences mises en évidence.

**Critères d'acceptation**
- [ ] Une erreur de syntaxe volontaire dans `nginx.conf` est refusée à l'enregistrement, avec le message du validateur, et le fichier distant reste intact.
- [ ] Le diff de `sshd_config` entre une VM durcie et une VM d'origine montre exactement les lignes modifiées.

---

## Étape 22 — Export d'inventaire Ansible
**But** : préparer les serveurs pour Ansible et générer des fichiers prêts à l'emploi, pour faire le pont avec les outils du métier (Q4).
**Dépend de** : 3, 15. Profite de l'étape 17 (expiration des clés).

Contenu :
- **Choix des hôtes** : hôtes ou groupes du carnet, qui deviennent des groupes Ansible.
- **Compte de service** : création d'un utilisateur `ansible` sans mot de passe (accès par clé uniquement), droits via `/etc/sudoers.d/ansible` validé par `visudo -c`, le tout via le processus de modification sûre.
- **Clé dédiée et bridée** : Ed25519 générée pour Ansible seulement et stockée dans le coffre, déployée avec `from="<IP de la machine de contrôle>"`, sans redirection de ports ni d'agent, avec une date d'expiration en option.
- **Fichiers générés** : `inventory.yml` (groupes, `ansible_host`, `ansible_port`, `ansible_user: ansible`), un `known_hosts` contenant les clés d'hôte déjà vérifiées par Younji, un `ansible.cfg` qui garde `host_key_checking = True`, et la commande de test `ansible all -m ping`.
- **Révocation** : la fonction inverse supprime le compte, la clé et le fichier sudoers sur chaque serveur.

À trancher au début de l'étape (ADR) :
- `sudo` sans mot de passe (pratique, mais le compte devient l'équivalent de root) ou mot de passe demandé au lancement (`--ask-become-pass`).
- Export de la clé privée : protégée par une passphrase et chargée dans un agent SSH, ou fichier `0600` avec un avertissement.

Points de vigilance :
- Ansible ne tourne pas nativement sous Windows. La machine de contrôle sera WSL ou une VM Linux du lab, et l'export doit la cibler (son IP pour `from=`, des chemins Linux).

**Critères d'acceptation**
- [ ] Depuis la machine de contrôle, `ansible all -m ping` réussit sur les VM exportées, sans jamais désactiver la vérification des clés d'hôte.
- [ ] Une connexion avec la clé Ansible depuis une autre IP que la machine de contrôle est refusée.
- [ ] Après une révocation, le compte `ansible`, sa clé et son fichier sudoers ont disparu de chaque VM.
