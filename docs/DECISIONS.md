# Journal des décisions d'architecture (ADR)

Chaque décision structurante est consignée ici. On ne réécrit jamais un ADR : si une décision change, on en ajoute un nouveau qui remplace l'ancien.
Format : statut, contexte, décision, alternatives écartées, conséquences.

| ADR | Sujet | Statut |
|---|---|---|
| 001 | Tauri 2 comme socle applicatif | Acceptée |
| 002 | russh pour SSH, SFTP et tunnels | Acceptée (tunnels retirés : ADR-038) |
| 003 | Stack frontend | Acceptée |
| 004 | Logique métier dans des crates indépendantes de Tauri | Acceptée |
| 005 | Schéma cryptographique du coffre | Acceptée |
| 006 | Clés d'hôte : TOFU strict | Acceptée ; point ouvert tranché par ADR-027 |
| 007 | RDP en deux temps | Acceptée |
| 008 | Dépôt GitHub privé et stratégie CI | Partiellement remplacée par ADR-016 (visibilité) |
| 009 | Licence GPL-3.0 | Acceptée |
| 010 | Nom de code « Relais » et centralisation du nom | Remplacée par ADR-014 |
| 011 | Pas de synchronisation, export/import chiffré uniquement | Acceptée |
| 012 | Déverrouillage : mot de passe maître + système en option | Acceptée |
| 013 | Développement assisté par Claude Code | Acceptée |
| 014 | Nom définitif : Vigie | Remplacée par ADR-017 |
| 015 | Modèle d'isolation Tauri et commandes soumises aux capabilities | Acceptée |
| 016 | Dépôt public dès l'étape 0 | Acceptée |
| 017 | Nouveau nom : Younji | Acceptée |
| 018 | CSP : balises `<style>` en ligne autorisées pour xterm.js | Acceptée |
| 019 | russh 0.65.2, moteur `ring`, algorithmes explicites | Acceptée |
| 020 | Clé privée choisie par la boîte de dialogue du système, désignée par identifiant | Acceptée |
| 021 | Reconnexion sans secret mémorisé (provisoire jusqu'à l'étape 2) | Remplacée par ADR-030 pour les hôtes du carnet ; reste valable pour la connexion rapide |
| 022 | Liens du terminal : http(s), Ctrl+clic, revalidation côté Rust | Acceptée |
| 023 | TOFU strict : anti-rétrogradation et `known_hosts` en clair | Acceptée ; partie « `known_hosts` en clair » remplacée par ADR-027 |
| 024 | Flux du terminal : acquittement et deux `Channel` | Acceptée |
| 025 | xterm.js 5.5 plutôt que 6, pour garder `freezePrototype` | Acceptée |
| 026 | Verrouillage : sessions fermées et terminaux détruits | Acceptée, prise en autonomie, à relire |
| 027 | Clés d'hôte connues stockées dans le coffre | Acceptée, prise en autonomie, à relire |
| 028 | Application derrière le déverrouillage ; minuteur, tentatives et sauvegarde gérés par le cœur | Acceptée, prise en autonomie, à relire |
| 029 | Robustesse du mot de passe maître : zxcvbn (Rust) fait foi, zxcvbn-ts pour l'indicateur | Acceptée, prise en autonomie, à relire |
| 030 | Identités et clés du coffre | Acceptée, prise en autonomie, à relire |
| 031 | Authentification en plusieurs méthodes et keyboard-interactive | Acceptée, prise en autonomie, à relire |
| 032 | Clés générées dans le coffre, sans passphrase ni export de la clé privée | Acceptée, prise en autonomie, à relire ; écart à KEY-01 à trancher |
| 033 | Déploiement de clé par commande `sh` constante et entrée standard | Acceptée, prise en autonomie, à relire |
| 034 | SFTP sur la connexion du terminal, dossiers locaux désignés par identifiant, éditeur imposé | Acceptée, prise en autonomie, à relire |
| 035 | Panneaux (split) : une session par panneau, arbre de disposition par onglet | Acceptée, prise en autonomie, à relire |
| 036 | Diffusion de la saisie côté interface, cibles explicites | Acceptée, prise en autonomie, à relire |
| 037 | Snippets dans le coffre, variables sans échappement automatique | Acceptée, prise en autonomie, à relire |
| 038 | Retrait des tunnels SSH (étape 6) | Acceptée |
| 039 | Terminal local : profils détectés par le cœur, commandes `term_*` séparées | Acceptée, prise en autonomie, à relire |
| 040 | RDP v1 : identifiants Windows de type domaine écrits par l'API, seul module `unsafe` du projet | Acceptée, prise en autonomie, à relire |
| 041 | Import OpenSSH et sauvegarde chiffrée | Acceptée, prise en autonomie, à relire |

---

## ADR-001 — Tauri 2 comme socle applicatif
**Statut** : Acceptée (17/11/2026)
**Contexte** : il faut une vraie application de bureau Windows + Linux, avec un backend capable de gérer finement SSH, la crypto et la mémoire, et une interface moderne.
**Décision** : Tauri 2 (version stable) : cœur en Rust, interface web dans la WebView du système.
**Alternatives écartées** :
- Electron : très répandu (Tabby), mais lourd (Chromium embarqué), backend Node moins adapté à la gestion des secrets en mémoire.
- Qt / Slint (100 % natif) : interface plus coûteuse à rendre moderne, écosystème terminal moins riche que xterm.js.
- Tauri 3 (alpha) : instable, documentation incomplète.
**Conséquences** : binaires légers ; sécurité renforcée par le modèle capabilities/CSP ; dépendance à WebView2 (Windows) et WebKitGTK (Linux), qui rendent un peu différemment, d'où des tests sur les deux.

## ADR-002 — russh pour SSH, SFTP et tunnels
**Statut** : Acceptée (17/11/2026)
**Contexte** : il faut une bibliothèque SSH client asynchrone, multiplateforme, qui gère PTY, SFTP, redirection de ports et keyboard-interactive.
**Décision** : `russh` (+ `russh-sftp`), en pur Rust et asynchrone (tokio).
**Alternatives écartées** :
- `ssh2` (bindings libssh2) : synchrone, dépend d'OpenSSL/libssh2 en C, compilation Windows plus pénible, algorithmes modernes en retard.
- Lancer le binaire `ssh` du système : peu de contrôle (clés d'hôte, coffre), dépend de l'installation locale.
**Conséquences** : pas de dépendance C ; API qui évolue souvent (toujours vérifier la version) ; le support FIDO2 (`sk-*`) est à étudier (étape 11, plan B : agent OpenSSH).

## ADR-003 — Stack frontend
**Statut** : Acceptée (17/11/2026)
**Contexte** : le porteur connaît déjà React + TypeScript + Tailwind (projet `gestionnaire-ssh`), et l'interface doit être moderne (style Termius) et bilingue.
**Décision** : React + TypeScript strict + Vite + Tailwind CSS + shadcn/ui, zustand pour l'état, i18next pour les traductions, xterm.js pour le terminal.
**Alternatives écartées** : Svelte (plus léger mais moins répandu), Vue.
**Conséquences** : écosystème riche ; attention à la surface de dépendances npm (voir SECURITE.md, M4).

## ADR-004 — Logique métier dans des crates indépendantes de Tauri
**Statut** : Acceptée (17/11/2026)
**Contexte** : il faut pouvoir tester SSH, le coffre et les imports sans lancer d'interface ni installer WebKitGTK en CI.
**Décision** : workspace Cargo avec `crates/core-*` sans dépendance à Tauri ; `src-tauri` ne fait que relier les commandes IPC aux crates.
**Conséquences** : tests rapides et ciblés ; frontière claire ; il faut un peu de code de conversion (erreurs, types) dans `src-tauri`.

## ADR-005 — Schéma cryptographique du coffre
**Statut** : Acceptée (17/11/2026)
**Contexte** : protéger identifiants et cartographie contre le vol du fichier (attaque hors ligne).
**Décision** : Argon2id (64 Mio, t=3, p=4) → KEK ; DEK aléatoire chiffrée par la KEK ; données chiffrées par la DEK en XChaCha20-Poly1305 ; en-tête authentifié en AAD. Détails : `SECURITE.md` §3.
**Alternatives écartées** :
- AES-256-GCM : sûr, mais ses nonces de 96 bits rendent les nonces aléatoires plus risqués à long terme ; XChaCha20 (192 bits) supprime ce souci.
- SQLCipher : dépendance C, et chiffrement au niveau de la base plutôt que des données.
- Trousseau du système seul : pas de portabilité, pas d'export.
**Conséquences** : changer le mot de passe maître renouvelle la DEK et rechiffre tout le coffre (voir `SECURITE.md` §3.1) ; la DEK permet le déverrouillage par le système sans y stocker le mot de passe ; les paramètres Argon2 peuvent évoluer, car ils sont stockés dans l'en-tête.

## ADR-006 — Clés d'hôte : TOFU strict
**Statut** : Acceptée (17/11/2026). Point ouvert (emplacement de `known_hosts`) tranché par ADR-027 : dans le coffre.
**Décision** : acceptation explicite à la première connexion (empreinte SHA-256) ; si la clé change, la connexion est refusée, sans contournement. Fichier `known_hosts` propre à l'application, au format OpenSSH.
**Alternatives écartées** : mode strict seul (trop contraignant au quotidien) ; configuration par hôte (complexité jugée inutile pour l'instant).
**Conséquences** : comportement familier pour un utilisateur d'OpenSSH ; l'emplacement final de `known_hosts` (fichier ou coffre) sera tranché à l'étape 2.

## ADR-007 — RDP en deux temps
**Statut** : Acceptée (17/11/2026)
**Décision** : étape 8, lancement du client du système (mstsc / xfreerdp) sans exposer le mot de passe ; étape 12, RDP natif dans un onglet avec IronRDP.
**Conséquences** : RDP utilisable tôt ; le plus gros risque technique est repoussé en fin de parcours, sans bloquer le reste.

## ADR-008 — Dépôt GitHub privé et stratégie CI
**Statut** : Acceptée (17/11/2026). Visibilité du dépôt remplacée par ADR-016 ; la stratégie CI reste valable.
**Contexte** : le dépôt reste privé au début. Les minutes GitHub Actions sont limitées et les runners Windows comptent double.
**Décision** : `ci.yml` sous Linux à chaque push ; `build.yml` (Windows + Linux) en manuel ou sur tag. Passage en public à l'étape 13. Gitea abandonné, car le Gitea du porteur n'est pas joignable depuis les environnements cloud.
**Conséquences** : coûts maîtrisés ; les installeurs se demandent explicitement quand il y a une version à tester.

## ADR-009 — Licence GPL-3.0
**Statut** : Acceptée (17/11/2026)
**Décision** : GPL-3.0 à la publication. `cargo-deny` vérifie que les dépendances sont compatibles (MIT, Apache-2.0, BSD, ISC, MPL-2.0…).
**Conséquences** : les forks restent open source ; licence approuvée par l'OSI, donc éligible à la signature gratuite de la SignPath Foundation.

## ADR-010 — Nom de code « Relais » et centralisation du nom
**Statut** : Remplacée par ADR-014 (17/11/2026)
**Décision** : le nom provisoire « Relais » (identifiant de bundle `io.github.mlft9.relais`) est défini en un seul endroit par couche : `productName` / `identifier` dans `tauri.conf.json`, clé i18n `app.name`, constante Rust `APP_NAME`. Les crates portent des noms neutres (`core-*`).
**Conséquences** : le changement de nom à l'étape 13 se limite à quelques fichiers, plus le nom du fichier de coffre. **Attention** : changer l'`identifier` déplace le dossier de données, donc il faudra prévoir une migration.

## ADR-011 — Pas de synchronisation, export/import chiffré uniquement
**Statut** : Acceptée (17/11/2026)
**Décision** : aucune synchronisation automatique. Un export/import chiffré avec une phrase de passe distincte (étape 9).
**Conséquences** : aucune infrastructure à maintenir, aucune surface d'attaque réseau supplémentaire.

## ADR-012 — Déverrouillage : mot de passe maître + système en option
**Statut** : Acceptée (17/11/2026)
**Décision** : mot de passe maître obligatoire à la création et toujours valable. En option (étape 10), déverrouillage rapide par Windows Hello ou le trousseau Linux. Verrouillage automatique après inactivité.
**Point ouvert (étape 10)** : la façon de lier Windows Hello à la DEK. Un simple consentement (`UserConsentVerifier`) n'est qu'une barrière d'interface. Une clé `KeyCredentialManager` dont la signature sert à dériver une clé de chiffrement offre une vraie liaison cryptographique. À trancher dans un nouvel ADR.

## ADR-013 — Développement assisté par Claude Code
**Statut** : Acceptée (17/11/2026)
**Décision** : le code est écrit par Claude Code, étape par étape, selon `CLAUDE.md` / `AGENTS.md`, avec trois agents spécialisés (sécurité, QA, documentation) et des commandes de processus (`/etape`, `/retour`, `/release`, `/revue-securite`). Le porteur valide chaque plan, teste chaque étape et prend les décisions.
**Conséquences** : rythme soutenu ; la revue de sécurité systématique et les tests compensent le risque d'erreurs plausibles mais fausses ; le processus est documenté et assumé dans le README.

## ADR-014 — Nom définitif : Vigie
**Statut** : Remplacée par ADR-017 (17/11/2026). Acceptée le 17/11/2026, remplaçait ADR-010.
**Contexte** : le nom de code « Relais » devait être remplacé avant la publication. Le choisir avant d'écrire la moindre ligne de code évite toute migration du dossier de données.
**Décision** : le projet s'appelle **Vigie** (le guetteur en haut du mât, qui surveille l'horizon), en cohérence avec l'orientation sécurité. Dépôt `mlft9/vigie`, identifiant de bundle `io.github.mlft9.vigie`. La règle de centralisation d'ADR-010 est conservée : `productName` / `identifier` dans `tauri.conf.json`, clé i18n `app.name`, constante Rust `APP_NAME`. Fichiers de données : `vault.vigie`, sauvegardes `.vigie-backup`. Variables de test : `VIGIE_TEST_*`.
**Alternatives écartées** : Poterne, Amarre, Escale, Nacre (libres mais moins parlants) ; Conque/Conch, Pagure, Carapace, Bastion (déjà utilisés par des logiciels proches).
**Vérification faite** : aucun client SSH ni outil d'accès distant connu ne porte ce nom (recherche web du 17/11/2026).
**Reste à faire (étape 13)** : vérifier les marques déposées dans le domaine logiciel (INPI, EUIPO) avant la publication.
**Conséquences** : aucune migration à prévoir. Le mot est le même en français et proche en anglais (*vigil*), facile à prononcer.

## ADR-015 — Modèle d'isolation Tauri et commandes soumises aux capabilities
**Statut** : Acceptée (17/11/2026), décision du porteur
**Contexte** : `SECURITE.md` §5 demandait d'évaluer le modèle d'isolation de Tauri à l'étape 0. La menace visée est M4 : une dépendance npm compromise qui s'exécute dans la WebView et tente d'appeler le cœur Rust.
**Décision** :
- **Isolation activée** : `app.security.pattern` en mode `isolation`, application d'isolation dans `isolation/`. Le script est classique (pas de module ES), car les modules ne se chargent pas dans l'iframe sandboxée sous WebView2. Il applique une **liste blanche de commandes**. Une commande refusée est remplacée par `vigie_isolation_denied` (commande inexistante), sans sa charge utile : l'appel échoue avec une erreur. Lever une exception dans le hook laisserait la promesse de l'appel en attente indéfiniment.
- **Commandes applicatives soumises aux capabilities** : chaque commande est déclarée dans `src-tauri/build.rs` (`AppManifest`). Tauri génère une permission `allow-<commande>`, qu'une capability doit accorder explicitement (`allow-app-info` dans `capabilities/main.json`).
- **Garde-fou automatique** : `tests/security/tauri-config.test.ts` impose la cohérence entre `build.rs`, la liste blanche de l'isolation et les capabilities, et compare les permissions accordées à une liste exacte.

**Limite constatée** (test sur un build réel piloté par CDP, 17/11/2026) : un script de la fenêtre principale qui intercepte `fetch` récupère l'en-tête `Tauri-Invoke-Key`, puis appelle `http://ipc.localhost/<commande>` directement avec un corps vide, sans passer par l'iframe. L'appel `plugin:app|version` n'a été arrêté que par l'ACL (capabilities). L'isolation est donc une défense en profondeur « au mieux », pas une barrière : les vraies barrières sont les **capabilities** et la **validation côté Rust**.
**Règles qui en découlent** : ne jamais accorder une permission en comptant sur le filtrage de l'isolation ; ne jamais confier à l'isolation la validation d'une charge utile.
**Alternatives écartées** :
- Pas d'isolation (CSP + capabilities seules) : écarté, l'isolation reste une couche de plus à coût faible.
- Ajouter à la CSP le hash de la balise `<style>` injectée par Tauri : écarté, on préfère ne pas élargir la CSP pour un besoin purement visuel.

**Conséquences** :
- La balise `<style>` que Tauri injecte pour masquer l'iframe d'isolation est bloquée par notre CSP (`style-src 'self'`). L'iframe est masquée depuis `src/index.css`, sans assouplir la CSP. Deux avertissements CSP apparaissent dans la console : ils sont attendus.
- Chaque message IPC est chiffré (AES-GCM) entre l'iframe et le cœur : coût négligeable.
- Toute nouvelle commande doit être ajoutée à trois endroits : `src-tauri/build.rs`, `isolation/index.js`, une capability (et la liste attendue du test). Le test échoue si l'un d'eux est oublié.

## ADR-016 — Dépôt public dès l'étape 0
**Statut** : Acceptée (17/11/2026), décision du porteur. Remplace la partie « visibilité » d'ADR-008.
**Contexte** : ADR-008 prévoyait un dépôt privé jusqu'à l'étape 13. Le porteur a rendu le dépôt public à la fin de l'étape 0, notamment pour pouvoir consulter la CI sans authentification et pour la valeur portfolio du projet.
**Décision** : le dépôt `mlft9/vigie` est public. Les éléments que l'étape 13 prévoyait « avant publication » sont avancés :
- `LICENSE` : texte officiel de la GPL-3.0 (gnu.org), conformément à ADR-009 ;
- `SECURITY.md` : politique de signalement (FR/EN) via le signalement privé de GitHub, avec des délais de réponse indicatifs.

**Restent à faire par le porteur** :
- activer le signalement privé (*Settings > Code security > Private vulnerability reporting*) ;
- vérifier que « Vigie » n'est pas une marque déposée dans le domaine logiciel (INPI, EUIPO), comme prévu par ADR-014.

**Alternatives écartées** : revenir en privé jusqu'à l'étape 13 (aucun bénéfice de sécurité : aucun secret n'est dans le dépôt, gitleaks le vérifie à chaque push).
**Conséquences** :
- Les minutes GitHub Actions sont gratuites pour un dépôt public : le coût des builds Windows n'est plus une contrainte. La stratégie d'ADR-008 (CI Linux à chaque push, `build.yml` manuel ou sur tag) est conservée par sobriété.
- Tout ce qui est poussé devient public et durable (historique, artefacts, journaux de CI) : la règle « aucun secret dans le dépôt ni dans les journaux » prend encore plus d'importance.
- Les contributions externes deviennent possibles : `pull_request_target` reste interdit, et les workflows n'exposent aucun secret aux PR venant de forks.
- La signature de code via la SignPath Foundation (projets open source) pourra être demandée plus tôt, sans attendre l'étape 13.

## ADR-017 — Nouveau nom : Younji
**Statut** : Acceptée (17/11/2026), décision du porteur. Remplace ADR-014.
**Contexte** : la vérification de marque prévue par ADR-014 et ADR-016 a été faite le 17/11/2026 (TMview, recherche « Contient : VIGIE »). Le signe « VIGIE » est enregistré à l'identique en **classe 9** (logiciels) et/ou **42** (services informatiques) par plusieurs titulaires actifs : en France (n° 3652191, Lacroix Traffic, classe 9 ; n° 4726295, Sellor, classes 9 et 42), dans l'Union européenne (n° 019097956, ViGIE Solutions, classes 9 et 42, en cours d'enregistrement) et en Allemagne. La marque française « VIGIE CYBER » vise en outre le domaine de la cybersécurité. Pour un logiciel public et diffusé, le risque de contrefaçon n'est pas acceptable.
**Décision** : le projet s'appelle **Younji** (prononcé « younji »), un nom inventé.
- Vérifications faites le 17/11/2026 : TMview « Contient : younji » ne renvoie que deux marques coréennes expirées en classe 6 ; aucun logiciel, application ni dépôt homonyme trouvé par recherche web.
- Règle de centralisation d'ADR-010 et ADR-014 conservée : `productName` / `identifier` dans `tauri.conf.json`, clé i18n `app.name`, constante Rust `APP_NAME`.
- Nouveaux identifiants : dépôt `mlft9/younji`, identifiant de bundle `io.github.mlft9.younji`, crate `younji` (bibliothèque `younji_lib`), variables `YOUNJI_LOG` et `YOUNJI_TEST_*`, fichiers `vault.younji` et `.younji-backup`, octets magiques `YNJV` (coffre) et `YNJB` (sauvegarde) dans `SECURITE.md` §3.
- Icône provisoire : un « Y » (le mât et sa hune) surmonté d'un point (le guetteur).

**Alternatives écartées** :
- Garder « Vigie » : risque juridique (voir le contexte).
- « Hungie » : libre comme marque, mais l'orthographe ne correspond pas à la prononciation voulue et évoque *hungry* en anglais.
- Hune, Gabier, Poterne, Amarre : non retenus par le porteur.

**Conséquences** :
- Aucune migration : aucune version publiée, aucun utilisateur. Le changement d'identifiant déplace seulement le dossier de données de développement (`%LOCALAPPDATA%\io.github.mlft9.vigie`), qui peut être supprimé.
- Les ADR et entrées du CHANGELOG antérieurs gardent le nom « Vigie » : c'est l'historique.
- `.gitignore` et les règles `deny` de Claude Code gardent aussi les anciens motifs `vigie_test_*` / `~/.vigie-lab/`, pour protéger d'éventuelles clés de test déjà générées.
- Le dépôt GitHub est renommé par le porteur ; GitHub redirige automatiquement l'ancienne adresse.
- Avant toute diffusion commerciale ou tout gain de notoriété, envisager le dépôt de la marque « Younji » en classes 9 et 42.

## ADR-018 — CSP : balises `<style>` en ligne autorisées pour xterm.js
**Statut** : Acceptée (18/11/2026), décision du porteur
**Contexte** : xterm.js 6 injecte des balises `<style>` dynamiques (barre de défilement, rendu DOM de repli). La CSP de production (`style-src 'self'`) les bloquait, alors que la CSP de développement autorisait déjà les styles en ligne : le terminal aurait fonctionné en `npm run tauri dev` et cassé dans l'installeur.
**Décision** : ajouter la seule directive `style-src-elem 'self' 'unsafe-inline'`, en production comme en développement. `style-src 'self'` reste inchangé en production : les attributs `style="…"` restent interdits. Scripts et sources distantes inchangés. `tests/security/tauri-config.test.ts` vérifie que `'unsafe-inline'` n'apparaît que dans `style-src` et `style-src-elem`.
**Alternatives écartées** :
- Rendu WebGL obligatoire, sans repli DOM : le terminal deviendrait inutilisable sur une VM sans accélération graphique ou sous WebKitGTK.
- `style-src 'unsafe-inline'` global : ouvrirait aussi les attributs `style`, sans besoin.

**Conséquences** : risque jugé faible. Une feuille de style injectée pourrait modifier l'affichage, mais pas exfiltrer de données : `img-src`, `font-src` et `connect-src` n'autorisent aucune origine distante. La CSP de développement doit rester aussi proche que possible de celle de production, pour éviter un nouveau « marche en dev, casse dans l'installeur ».

## ADR-019 — russh 0.65.2, moteur `ring`, algorithmes explicites
**Statut** : Acceptée (18/11/2026)
**Contexte** : ADR-002 retient russh. Il faut choisir sa version, son moteur cryptographique et les algorithmes négociés.
**Décision** :
- `russh` **0.65.2**, sans les fonctionnalités par défaut, avec `ring` et `rsa`. Pas de compression.
- La 0.65.2 est adoptée **avant la fin du délai de 7 jours** prévu pour les nouvelles versions (Dependabot, `cooldown`) : la 0.65.1 imposait `rsa 0.10.0-rc.18`, touchée par RUSTSEC-2023-0071 (attaque temporelle Marvin) ; la 0.65.2 utilise `rsa 0.10.0`, corrigée.
- Algorithmes listés explicitement dans `crates/core-ssh/src/config.rs`, sans dépendre des défauts de russh : échange de clés hybride `mlkem768x25519-sha256` en tête, puis curve25519, ECDH NIST, DH groupes 14/16/18 en SHA-2, strict kex (parade à Terrapin) ; chiffrements ChaCha20-Poly1305 et AES-GCM, AES-CTR en repli avec MAC `*-etm` SHA-2 ; clés d'hôte Ed25519, ECDSA, `rsa-sha2-512/256`. Exclus : SHA-1, CBC, 3DES, DSA, DH group exchange, `none`, compression, certificats d'hôte. Un test échoue si un algorithme faible réapparaît.

**Alternatives écartées** :
- Moteur `aws-lc-rs` (défaut de russh) : exige CMake et la compilation de code C sous Windows, ce qui alourdit le poste de développement et la CI.
- Rester sur 0.65.1 jusqu'à la fin du délai : dépendance vulnérable connue.

**Conséquences** : aucune dépendance C pour SSH. La liste explicite peut refuser un vieil équipement ; une option « algorithmes hérités » par hôte passerait par un nouvel ADR (`SECURITE.md` §4.1).

## ADR-020 — Clé privée choisie par la boîte de dialogue du système, désignée par identifiant
**Statut** : Acceptée (18/11/2026), décision du porteur
**Contexte** : AUTH-02 demande l'authentification par clé. La WebView est non fiable (`SECURITE.md` §5) : si elle pouvait transmettre un chemin, un script compromis ferait lire au cœur n'importe quel fichier.
**Décision** : la commande `ssh_pick_key_file` ouvre la boîte de dialogue de fichiers du système **depuis Rust** (`tauri-plugin-dialog`, initialisé côté Rust, **aucune permission du plugin accordée à la WebView**). Le chemin choisi reste dans `SessionManager` ; l'interface reçoit un identifiant opaque et le nom du fichier, sans le dossier. `ssh_connect` n'accepte qu'un identifiant connu. 32 fichiers au plus sont mémorisés pendant l'exécution ; les plus anciens sont oubliés.
**Alternatives écartées** :
- Chemin saisi dans un champ texte : le cœur lirait un chemin fourni par la WebView.
- Contenu de la clé collé dans l'interface : la clé privée transiterait par la WebView.

**Conséquences** : la WebView ne connaît ni l'arborescence ni le contenu de la clé. Le fichier est relu à chaque connexion (64 Kio au plus). L'import de clés dans le coffre arrive à l'étape 2.

## ADR-021 — Reconnexion sans secret mémorisé (provisoire jusqu'à l'étape 2)
**Statut** : Acceptée (18/11/2026), décision du porteur. Provisoire : à revoir avec le coffre (étape 2). Remplacée par ADR-030 pour les hôtes du carnet ; reste valable pour la connexion rapide.
**Contexte** : SSH-06 demande un bouton « Reconnecter ». Sans coffre, garder le mot de passe ou la passphrase pour la reconnexion les laisserait en mémoire pendant toute la vie de l'onglet.
**Décision** : « Reconnecter » rouvre le formulaire pré-rempli (hôte, port, utilisateur, fichier de clé) et redemande le mot de passe ou la passphrase. Le cœur détruit les identifiants dès la fin de l'authentification ; l'interface vide le champ dès l'envoi et ne garde aucun secret dans un store.
**Alternatives écartées** : mémoriser le secret côté Rust pendant la vie de l'onglet (plus confortable, mais secret en mémoire sans la protection du coffre).
**Conséquences** : une saisie de plus à chaque reconnexion. Avec le coffre, la reconnexion pourra réutiliser une identité désignée par son identifiant.

## ADR-022 — Liens du terminal : http(s), Ctrl+clic, revalidation côté Rust
**Statut** : Acceptée (18/11/2026), décision du porteur sur `http:` + `https:`
**Contexte** : SSH-05 demande des liens cliquables. Le serveur contrôle le texte affiché, y compris les liens OSC 8, dont le texte peut masquer la vraie cible.
**Décision** :
- Seuls `http:` et `https:` sont ouverts (`http:` pour les services du lab), avec un hôte et sans identifiants intégrés (`user:pass@`).
- Ouverture au **Ctrl+clic** uniquement. Un lien OSC 8 passe par une confirmation qui affiche l'adresse réelle.
- La commande `open_external_url` **revalide** l'adresse côté Rust (2048 caractères au plus, aucun caractère de contrôle) puis la transmet au système via la crate `open`, en argument de processus, jamais à un shell.

**Alternatives écartées** :
- `tauri-plugin-opener` : tirait `zbus` 5, signalé non maintenu par cargo-deny.
- `https:` seul : bloquerait les interfaces web du lab.

**Conséquences** : aucune permission de plugin d'ouverture dans les capabilities. `.claude/rules/tauri-config.md` est aligné sur cette règle.

## ADR-023 — TOFU strict : anti-rétrogradation et `known_hosts` en clair
**Statut** : Acceptée (18/11/2026). Précise ADR-006. Partie « `known_hosts` en clair » (fichier dans `app_data_dir`, déblocage manuel) remplacée par ADR-027 ; les règles de vérification (anti-rétrogradation, `@revoked`, enregistrement après vérification de la signature) restent valables.
**Contexte** : mise en œuvre d'HKY-01 à HKY-03. Avec un TOFU naïf, un homme du milieu qui présente une clé d'un **autre type** que celle enregistrée retomberait sur une simple demande de confiance.
**Décision** :
- Hôte connu (au moins une ligne pour `hôte` ou `[hôte]:port`) et clé présentée absente de la liste : **connexion bloquée**, même si le type de clé diffère. Une ligne illisible qui nomme l'hôte compte comme une clé connue : elle bloque au lieu de redemander confiance. Une clé `@revoked` est refusée.
- La clé acceptée est enregistrée dans `check_server_key`, que russh appelle après avoir vérifié la signature du serveur sur l'échange de clés : le serveur a prouvé qu'il détient la clé privée correspondante.
- Fichier `known_hosts` dans `app_data_dir`, au format OpenSSH, **en clair et sans hachage des noms**, écrit de façon atomique (0600 sous Linux). Les lignes hachées, les motifs et `@cert-authority` sont ignorés.
- Déblocage manuel en attendant HKY-04 (étape 2) : l'alerte affiche le chemin du fichier et le début exact de la ligne à supprimer, fourni par le cœur.

**Alternatives écartées** :
- Accepter un nouveau type de clé pour un hôte connu : ouvre la rétrogradation.
- Hacher les noms d'hôtes (`HashKnownHosts`) : protège peu un fichier local et empêcherait d'afficher la liste dans la future gestion des clés d'hôte.

**Conséquences** : un serveur qui ajoute un type de clé d'hôte est bloqué jusqu'à la suppression manuelle de ses lignes. L'emplacement définitif de `known_hosts` (fichier ou coffre) reste à trancher à l'étape 2 (ADR-006).

## ADR-024 — Flux du terminal : acquittement et deux `Channel`
**Statut** : Acceptée (18/11/2026)
**Contexte** : un `cat` de plusieurs gigaoctets ou `yes` ne doit ni figer l'interface ni faire grossir la mémoire sans limite. Deux appels IPC peuvent aussi s'exécuter dans le désordre côté Rust.
**Décision** :
- Sortie regroupée par lots (8 ms ou 32 Kio). L'interface acquitte les octets affichés (`ssh_ack`) ; au-delà de **2 Mio non acquittés**, la session cesse de lire le canal SSH, la fenêtre SSH se remplit et le serveur s'arrête d'émettre. Un acquittement excessif ramène le compteur à zéro sans déborder.
- Deux `Channel` par session : **octets bruts** pour la sortie (sans encodage JSON), **JSON** pour les événements d'état.
- Saisie sérialisée côté interface : un seul `ssh_write` en cours, découpage à 64 Kio (limite revérifiée par le cœur).
- Les messages de `Channel` de plus de 1 Kio sont récupérés par la commande interne `plugin:__TAURI_CHANNEL__|fetch`, ajoutée à la liste des commandes internes de l'isolation (`isolation/index.js`). Tauri l'exempte d'ACL ; elle ne renvoie qu'un message déjà destiné à la fenêtre, une seule fois, désigné par un identifiant aléatoire.

**Alternatives écartées** :
- Événements globaux Tauri : non ciblés par session et sérialisés en JSON.
- Pas de contrôle de flux : mémoire non bornée, interface submergée.

**Conséquences** : mémoire bornée par session. Toute autre commande interne de Tauri reste refusée par l'isolation ; un test le vérifie.

## ADR-025 — xterm.js 5.5 plutôt que 6, pour garder `freezePrototype`
**Statut** : Acceptée (18/11/2026), décision du porteur. Précise ADR-003 (choix de xterm.js) ; ADR-018 reste valable, xterm.js 5.5 injectant aussi des balises `<style>`.
**Contexte** : au premier lancement réel (`npm run tauri dev`), la fenêtre restait blanche. Tauri gèle `Object.prototype` avant tout script (`freezePrototype: true`, durcissement contre la pollution de prototype). Le code de VS Code embarqué dans xterm.js 6 (espace de noms `KeyCodeUtils`) affecte `toString` à un objet ordinaire dès le chargement du module : en mode strict, l'affectation d'une propriété héritée en lecture seule lève une exception, et tout le JavaScript de l'application échoue. Les tests ne le voyaient pas, car ils tournaient sans ce gel.
**Décision** : revenir à la génération xterm.js 5.5 (`@xterm/xterm` 5.5.0, addons fit 0.10, search 0.15, web-links 0.11, unicode11 0.8, webgl 0.18), vérifiée compatible avec le prototype gelé, et garder `freezePrototype`. Un test de non-régression (`src/frozen-prototype.test.tsx`) gèle `Object.prototype`, puis charge xterm.js et ses addons, affiche l'application et ouvre un terminal.
**Alternatives écartées** :
- Désactiver `freezePrototype` : retire une défense en profondeur contre une dépendance npm compromise (menace M4).
- Réécrire l'affectation fautive de xterm.js 6 à la compilation (plugin Vite) : rustine fragile, à revérifier à chaque mise à jour.

**Conséquences** : les fonctions utilisées sont identiques en 5.5. Une mise à jour vers xterm.js 6 (Dependabot) fera échouer le test de non-régression tant que xterm.js n'aura pas corrigé ce point ; à réexaminer à chaque version majeure.

## ADR-026 — Verrouillage : sessions fermées et terminaux détruits
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur
**Contexte** : l'étape 2 (VLT-03) demande de décider du sort des sessions ouvertes au verrouillage (`SECURITE.md` §3.5). Un poste laissé sans surveillance ne doit exposer ni les secrets du coffre, ni un shell ouvert, ni l'historique affiché.
**Décision** :
- Tout verrouillage (manuel par le bouton ou Ctrl+Maj+L, inactivité, rechargement de l'interface) passe par `lock_everything` : le cœur **verrouille d'abord le coffre**, puis **ferme toutes les sessions SSH**.
- Une connexion en cours de préparation revérifie l'état du coffre une fois sa session enregistrée : si un verrouillage est survenu entre-temps, la session est **refermée aussitôt** (`vault.locked`). Aucune session ne peut donc survivre au verrouillage.
- L'interface détruit les onglets et les terminaux, tampon de défilement compris, et oublie le contenu du coffre affiché.

**Alternatives écartées** :
- Garder les sessions derrière l'écran de verrouillage : plus confortable, mais shell ouvert et historique en mémoire de l'interface sur un poste sans surveillance.
- Option laissée à l'utilisateur : complexité supplémentaire, reportée.

**Conséquences** : un verrouillage automatique interrompt les tâches longues lancées dans un terminal (délai par défaut 15 min, réglable de 1 à 60 min, sans option « jamais », voir ADR-028). Réversible : une option « garder les sessions » pourrait être ajoutée par un nouvel ADR.

## ADR-027 — Clés d'hôte connues stockées dans le coffre
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur. Remplace la partie « `known_hosts` en clair » d'ADR-023 ; tranche le point ouvert d'ADR-006.
**Contexte** : à l'étape 1, les clés d'hôte acceptées étaient dans un fichier `known_hosts` en clair (ADR-023). Ce fichier révèle la liste des serveurs (cartographie, menace M1) et, surtout, un logiciel tiers pouvait le modifier pour y glisser la clé d'un attaquant, qui serait alors acceptée sans alerte.
**Décision** :
- Le contenu, **au format OpenSSH**, est stocké dans le coffre (champ `knownHosts`, 4 Mio au plus), donc chiffré et authentifié.
- `core-ssh` lit et écrit ce contenu via le trait `KnownHostsStorage`, implémenté par `src-tauri` pour le coffre (`VaultKnownHosts`). `core-ssh` ne dépend pas de `core-vault` (ADR-004) ; le stockage fichier (`FileStorage`) reste disponible pour les tests.
- Toute connexion exige le coffre déverrouillé (`vault.locked` sinon).
- L'ancien fichier `known_hosts` en clair est repris à la création du coffre, puis supprimé.
- Déblocage dans Clés > Clés d'hôte connues (HKY-04), après une confirmation qui rappelle de vérifier l'empreinte. La suppression désigne la **position exacte de la ligne** et n'agit que si cette ligne n'a pas changé depuis l'affichage : une liste périmée ne supprime jamais une autre clé.

**Alternatives écartées** :
- Garder le fichier en clair : ni confidentialité ni intégrité.
- Hacher les noms d'hôtes dans le fichier : protège la confidentialité, pas l'intégrité, et empêche d'afficher la liste.

**Conséquences** : coût nul pour l'utilisateur, puisque toute l'application est déjà derrière le déverrouillage (ADR-028). L'alerte de clé modifiée n'indique plus de chemin de fichier : elle renvoie vers la gestion des clés d'hôte. HKY-03 (« fichier au format OpenSSH ») : le format est conservé, mais à l'intérieur du coffre ; l'export arrive à l'étape 9. Écart à relire par le porteur.

## ADR-028 — Application derrière le déverrouillage ; minuteur, tentatives et sauvegarde gérés par le cœur
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur
**Contexte** : VLT-01 à VLT-04. La WebView n'est pas fiable (`SECURITE.md` §5) : une interface compromise ne doit pouvoir ni empêcher le verrouillage, ni enchaîner les essais de mot de passe, ni changer le délai de verrouillage hors de l'application.
**Décision** :
- **Rien n'est affiché avant le déverrouillage** : carnet, terminaux et connexion rapide (qui a besoin des clés d'hôte, ADR-027).
- **Minuteur d'inactivité dans le cœur** : tâche tokio qui vérifie toutes les 5 s. L'interface ne fait que signaler l'activité (`vault_touch`, au plus toutes les 15 s). Le délai est comparé à **deux horloges** : monotone (insensible aux changements d'heure) et murale (qui avance pendant la mise en veille). Le verrouillage a lieu dès que l'une des deux dépasse le délai : un portable rouvert après des heures se verrouille tout de suite, et une horloge murale qui recule n'empêche jamais le verrouillage.
- **Événement de verrouillage par `Channel`** (`vault_watch`) plutôt que par un événement global, pour ne pas accorder `core:event:*` à la fenêtre.
- **Rechargement de l'interface = verrouillage.**
- **Délai de verrouillage stocké dans le coffre** (authentifié), pas dans `settings.json`, modifiable en clair. Valeurs 1, 5, 10, 15, 30, 60 min ; 15 par défaut.
- **Délai progressif côté cœur** : tentatives de déverrouillage sérialisées ; après 3 échecs consécutifs, 1 s, 2 s, 4 s… jusqu'à 30 s. La vérification du mot de passe actuel lors d'un changement partage ce compteur, pour ne pas servir d'oracle sans ralentissement.
- **Copie de sauvegarde (`vault.younji.bak`)** :
  - au changement de mot de passe maître, le `.bak` est recopié depuis le nouveau fichier ; si la copie échoue, il est supprimé. Une copie lisible avec l'ancien mot de passe ne doit pas subsister ;
  - la création est refusée si seul le `.bak` existe (`vault.backup_only`) : sinon, la première sauvegarde écraserait la seule copie restante. L'utilisateur la restaure en la renommant.

**Alternatives écartées** :
- Minuteur dans l'interface : une interface compromise pourrait ne jamais verrouiller.
- Délai progressif dans l'interface : contournable par un appel IPC direct.
- Option « jamais » pour le verrouillage : un poste oublié resterait ouvert indéfiniment.
- Garder l'ancien `.bak` après un changement de mot de passe : il s'ouvrirait avec l'ancien mot de passe, éventuellement compromis.

**Conséquences** : le cœur est seul juge du verrouillage. Une interface compromise peut seulement simuler une activité (hors modèle de menace). Limite connue : écraser un fichier n'efface pas forcément ses anciennes versions sur le disque (`SECURITE.md` §3.6).

## ADR-029 — Robustesse du mot de passe maître : zxcvbn (Rust) fait foi, zxcvbn-ts pour l'indicateur
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur
**Contexte** : `SECURITE.md` §3.4 exige au moins 12 caractères et un score zxcvbn d'au moins 3. L'utilisateur doit voir la robustesse en direct pendant la saisie, mais un secret ne doit traverser l'IPC qu'une fois.
**Décision** :
- **Cœur** : crate `zxcvbn` 3.1 (sans la fonctionnalité `builder`), dans `core-vault/src/policy.rs`. Contrôle appliqué à la création comme au changement de mot de passe ; c'est lui qui fait foi.
- **Interface** : `@zxcvbn-ts/core` 4.2.0, `@zxcvbn-ts/language-common` 4.1.3 et `@zxcvbn-ts/language-en` 4.1.1, versions épinglées, chargées à la demande (dictionnaires dans un fichier séparé), pour un indicateur en direct calculé localement.

**Alternatives écartées** :
- Appel IPC à chaque frappe : le mot de passe traverserait l'IPC de nombreuses fois.
- Heuristique maison dans l'interface : incohérente avec le contrôle du cœur.

**Conséquences** : les deux implémentations peuvent rarement diverger ; le cœur l'emporte et son refus est affiché. Le test `freezePrototype` (ADR-025) charge aussi zxcvbn-ts. Limite connue : la crate `zxcvbn` copie le mot de passe dans des chaînes internes non effacées (`SECURITE.md` §6).

## ADR-030 — Identités et clés du coffre
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur. Remplace ADR-021 pour les hôtes du carnet ; ADR-021 reste valable pour la connexion rapide.
**Contexte** : HST-03 et KEY-02. Une connexion depuis le carnet doit utiliser l'identité enregistrée sans rien redemander, sans qu'aucun secret ne passe par l'interface.
**Décision** :
- **Import de clé** : la clé choisie par `ssh_pick_key_file` (ADR-020) est déchiffrée une fois avec sa passphrase, puis ré-encodée au format OpenSSH **sans passphrase** dans le coffre, qui la protège. La passphrase n'est pas conservée. La clé publique est stockée **sans commentaire** (il peut contenir un nom d'utilisateur ou de machine).
- **Identité clé + mot de passe** : la clé est essayée d'abord, le mot de passe ensuite si le serveur refuse la clé.
- **Suppressions** : une clé utilisée par une identité, ou une identité utilisée par un hôte, ne peut pas être supprimée (`vault.key_in_use`, `vault.identity_in_use`). Supprimer un groupe laisse ses hôtes « sans groupe ». Groupes à un seul niveau.
- **Connexion depuis le carnet** (`ssh_connect_host`) : l'interface désigne l'hôte par son identifiant ; le cœur lit l'identité dans le coffre. La reconnexion fait de même. Un hôte sans identité ouvre le formulaire de connexion rapide pré-rempli.
- Le cœur ne renvoie jamais un mot de passe d'identité (seulement `hasPassword`) ni une clé privée (seulement la clé publique et l'empreinte).

**Alternatives écartées** :
- Stocker la passphrase à côté de la clé chiffrée : aucune protection de plus que le coffre, et une saisie à chaque connexion.
- Supprimer en cascade les identités ou hôtes liés : casserait des entrées sans prévenir.
- Groupes imbriqués : plus complexes à valider, sans besoin exprimé par HST-02.

**Conséquences** : aucune saisie à la connexion ni à la reconnexion depuis le carnet. La sécurité des clés importées repose entièrement sur le mot de passe maître et Argon2id (ADR-005). L'export d'une clé privée n'est pas proposé.

## ADR-031 — Authentification en plusieurs méthodes et keyboard-interactive
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur. Précise l'ordre « clé puis mot de passe » d'ADR-030.
**Contexte** : AUTH-03 demande d'afficher les questions du serveur (code TOTP…). Un serveur peut exiger plusieurs méthodes à la suite (`AuthenticationMethods publickey,keyboard-interactive`) et poser des questions arbitraires : leur texte n'est pas fiable (menace M3), et un serveur malveillant peut tenter de se faire passer pour Younji.
**Décision** :
- **Enchaînement** : la méthode `none` révèle d'abord les méthodes acceptées ; ensuite la clé, le mot de passe et keyboard-interactive sont essayés dans cet ordre, **selon les méthodes restantes** annoncées par le serveur, succès partiels compris. Chaque moyen n'est utilisé qu'une fois.
- **Nouveau moyen `Credentials::Interactive`** : aucun secret fourni d'avance. C'est le mode « Questions du serveur (2FA) » de la connexion rapide.
- **Remplissage automatique du mot de passe enregistré**, limité : une seule fois par connexion, pour une demande d'**une seule** question **sans écho** dont le texte est exactement « Password » ou « Mot de passe » (casse, espaces et « : » ignorés), et seulement si la méthode `password` ne l'a pas déjà refusé. Toute autre question est posée à l'utilisateur.
- **Questions nettoyées et bornées** (caractères de contrôle et invisibles retirés, 8 questions par demande, 16 demandes au plus) ; au-delà, `ssh.auth_prompt_invalid`.
- **Réponses liées au numéro de tour** (`ssh_auth_answer`) : une réponse en retard ou envoyée deux fois ne peut pas servir à la question suivante. Le nombre de réponses et leur longueur sont vérifiés par le cœur.
- **Délais** : 30 s par échange réseau, 60 s de temps réseau cumulé pour toute l'authentification (hors saisie), 2 min de saisie par demande.
- **Anti-hameçonnage** : le dialogue indique l'hôte qui pose les questions, encadre le texte du serveur comme « Message envoyé par le serveur » et rappelle que Younji ne demande jamais le mot de passe maître à cet endroit.

**Alternatives écartées** :
- Ne jamais remplir automatiquement : un hôte du carnet « mot de passe + TOTP » (PAM) redemanderait le mot de passe déjà enregistré à chaque connexion, contrairement à HST-03.
- Remplir toute question qui contient « password » : le mot de passe partirait aussi vers « New password », « Password for root »… choisies par le serveur.

**Conséquences** : une identité « clé seule » refusée par le serveur peut désormais aboutir à des questions keyboard-interactive, comme avec OpenSSH. Le destinataire du mot de passe rempli automatiquement est le même serveur, déjà authentifié par sa clé d'hôte (TOFU), que pour la méthode `password`. Limite mémoire : russh reçoit les réponses en `String` non effacées (`SECURITE.md` §6).

## ADR-032 — Clés générées dans le coffre, sans passphrase ni export de la clé privée
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur. Prolonge ADR-030. **Écart assumé à KEY-01, à trancher par le porteur.**
**Contexte** : KEY-01 demande un générateur de clés « avec passphrase optionnelle ». Dans Younji, une clé importée est déjà stockée sans passphrase dans le coffre, qui la protège (ADR-030).
**Décision** :
- **Algorithmes** : Ed25519 (par défaut), ECDSA P-256, P-384 et P-521, RSA 4096. Ni RSA plus court, ni DSA.
- **Aléa** : générateur du système uniquement (`getrandom::SysRng`, équivalent d'`OsRng`). S'il échouait, la génération s'arrêterait plutôt que de produire une clé avec un aléa dégradé.
- **La clé naît dans le cœur et va directement dans le coffre**, sans passphrase, comme une clé importée. Seules la clé publique (sans commentaire) et l'empreinte reviennent à l'interface.
- **Pas de passphrase, pas d'export de la clé privée** : une passphrase ne protège qu'un fichier de clé, or la clé ne quitte jamais le cœur. Le coffre (Argon2id, ADR-005) la protège mieux que le chiffrement des fichiers OpenSSH (bcrypt-pbkdf).
- **Export de la clé publique** (`key_export_public`) : fichier choisi dans la boîte de dialogue du système ouverte par le cœur (ADR-020), extension `.pub` imposée (un nom tapé sans extension ne peut pas écraser la clé privée voisine), écriture atomique, aucun écrasement d'un fichier que la boîte de dialogue n'a pas confirmé.

**Alternatives écartées** :
- Passphrase optionnelle sans export : elle ne protégerait rien de plus que le coffre et imposerait une saisie à chaque connexion.
- Export de la clé privée : ouvrirait un chemin d'exfiltration hors du coffre.

**Conséquences** : une clé générée n'est utilisable que depuis Younji. **Question pour le porteur** : faut-il un export de la clé privée, chiffrée par une passphrase **obligatoire** ? Ce serait un nouvel ADR.

## ADR-033 — Déploiement de clé par commande `sh` constante et entrée standard
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur
**Contexte** : KEY-03 demande un déploiement idempotent, avec des permissions correctes. `SECURITE.md` §4.2 impose SFTP ou l'entrée standard d'une commande fixe ; SFTP n'arrive qu'à l'étape 4.
**Décision** :
- **Commande constante** `sh -c '…'` (`crates/core-ssh/src/deploy.rs`), sur une ligne, sans apostrophe ni « ! » : tout shell de connexion (bash, zsh, fish…) la lit de la même façon. Aucune donnée n'y est concaténée.
- **Ligne de clé par l'entrée standard** : `algorithme base64`, **sans commentaire** (aucun nom d'utilisateur ni de machine envoyé au serveur). Elle est **re-dérivée de la clé privée du coffre** puis revalidée ; l'interface ne fournit que des identifiants (hôte, clé).
- **Idempotence** : la clé n'est « déjà présente » que si une ligne de `authorized_keys` commence exactement par `algorithme base64`. Une ligne commentée ou portant des options (`from=`, `command=`) ne compte pas. Le script fait `umask 077`, `chmod 700 ~/.ssh`, `chmod 600 authorized_keys`, et ajoute un retour à la ligne si le fichier n'en a pas. Seuls un code de retour nul **et** un marqueur exact (`YOUNJI_ADDED` / `YOUNJI_PRESENT`) valent succès.
- **Même chemin qu'une session** (`SessionHandle::spawn_deploy`) : TOFU strict, 2FA (ADR-031), limite de sessions, fermeture au verrouillage (ADR-026). Hôte cible désigné dans le carnet ; s'il n'a pas d'identité, un mot de passe peut être saisi pour ce seul déploiement, sinon les questions du serveur prennent le relais.
- **Pas d'association automatique** de la clé déployée à l'identité de l'hôte : l'utilisateur garde la main (message d'aide après le déploiement).

**Alternatives écartées** :
- Insérer la clé dans la ligne de commande : injection possible, interdite par `SECURITE.md` §4.2.
- Attendre SFTP (étape 4) : retarde KEY-03 sans gain de sécurité, l'entrée standard étant déjà sûre.

**Conséquences** :
- Serveurs Unix uniquement. Un serveur Windows (shell `cmd` ou PowerShell) devrait échouer avec `ssh.deploy_failed` : à confirmer sur le lab.
- Cas limite : un Windows avec `sh` dans le PATH (Git for Windows) pourrait réussir le déploiement dans `%USERPROFILE%\.ssh\authorized_keys`, fichier ignoré par OpenSSH pour les comptes administrateurs (`administrators_authorized_keys`). La connexion par clé échouerait alors sans erreur au déploiement.
- Le passage à SFTP (étape 4) pourra être étudié dans un nouvel ADR.

## ADR-034 — SFTP sur la connexion du terminal, dossiers locaux désignés par identifiant, éditeur imposé
**Statut** : Acceptée (18/11/2026) — prise en autonomie, à relire par le porteur. Prolonge ADR-020 (chemins locaux gardés par le cœur) et ADR-026 (fermeture au verrouillage).
**Contexte** : SFTP-01 à SFTP-05. Le SFTP écrit sur le disque local des noms choisis par le serveur (menace M3), et l'interface, non fiable (M4), pilote des lectures et écritures de fichiers locaux. Ouvrir un fichier distant « pour l'éditer » peut aussi l'exécuter si l'on passe par l'application associée à son extension.
**Décision** :
- **SFTP sur la connexion du terminal** : un canal `sftp` est ouvert sur la session déjà authentifiée (`SessionHandle::open_sftp`), sans nouvelle authentification ni 2FA redemandée, avec la même clé d'hôte vérifiée. Le client SFTP se ferme avec la session, au verrouillage et au rechargement de l'interface. L'onglet double panneau dépend donc d'un terminal connecté.
- **Dossiers locaux (« racines ») choisis par le cœur** : la boîte de dialogue du système est ouverte par le cœur (`local_pick_root`), qui garde le chemin et renvoie un identifiant et un nom. **Aucun chemin local absolu n'est accepté de l'IPC.** Les chemins relatifs à une racine sont canonicalisés et confinés (`LocalRoot::resolve`). Les racines sont oubliées au verrouillage. `RootPolicy` refuse comme racine :
  - la racine d'un disque ou du système de fichiers ;
  - le dossier personnel entier et ses ancêtres ;
  - `~/.ssh` et ce qu'il contient ;
  - les dossiers de données de l'application (data, local data, cache, config, log), coffre et copies d'édition compris.
- **Panneau local sans suppression ni renommage**, et aucune politique de conflit acceptée de l'IPC : tout transfert démarre en « demander », l'écrasement ne se décide qu'à la question posée pendant le transfert.
- **Éditeur imposé**, jamais l'application associée ni une détection du type par le contenu (`src-tauri/src/editor.rs`) ; le fichier est passé en argument de processus, sans shell :
  - Windows : Bloc-notes, depuis `System32` ;
  - Linux : liste fixe d'éditeurs graphiques (`gnome-text-editor`, `gedit`, `kate`…), cherchés par chemin absolu dans `/usr/bin`, `/usr/local/bin` et `/bin`, jamais dans le `PATH` ; aucun trouvé : `editor.open_failed`. Le plan prévoyait `xdg-open` sur une copie `.txt` : écarté, car `xdg-open` suit les associations de l'utilisateur et peut détecter le type par le contenu ;
  - macOS : `open -e` (TextEdit).
  Un éditeur choisi par l'utilisateur est reporté.
- **Gestionnaire natif de glisser-déposer de Tauri désactivé** (`disable_drag_drop_handler`) : il bloquait le glisser-déposer HTML5 entre les panneaux sous Windows et aurait exposé à l'interface les chemins des fichiers déposés. Seul le glisser-déposer interne (types MIME propres à Younji) est pris en charge ; les fichiers déposés depuis le système sont ignorés.
- **Dépendance `russh-sftp` 3.0.1** (Apache-2.0), prévue par ADR-002. Son `SftpSession::read_dir` accumule toutes les entrées sans limite : les listages passent par `RawSftpSession`, avec nos bornes (`SECURITE.md` §4.2). Le contenu des fichiers passe par `SftpSession` (lectures et écritures en pipeline).
- **Exception cargo-deny RUSTSEC-2026-0543** : `chrono` n'est plus maintenue (aucune vulnérabilité connue). Elle est transitive de `russh-sftp`, qui ne s'en sert que pour fabriquer des attributs côté serveur, code jamais appelé par le client. À retirer dès que `russh-sftp` s'en passe ; à réexaminer à chaque mise à jour.

**Alternatives écartées** :
- Connexion SFTP séparée : nouvelle authentification (2FA redemandée), seconde vérification de clé d'hôte, durée de vie distincte de la session.
- Chemins locaux absolus fournis par l'interface : une interface compromise lirait ou écrirait n'importe quel fichier de l'utilisateur.
- Suppression et renommage locaux : une interface compromise pourrait détruire des fichiers locaux.
- Application associée ou `xdg-open` : un fichier distant `x.bat`, `x.js`, `x.desktop` ou un `README` contenant du HTML serait exécuté ou ouvert ailleurs qu'en édition.
- `SftpSession::read_dir` : mémoire non bornée face à un serveur malveillant.

**Conséquences et risques résiduels** :
- Une interface compromise peut lire, créer et écraser des fichiers dans une racine autorisée (en répondant « écraser » à un conflit) tant que le coffre est déverrouillé. Une confirmation native de l'écrasement local, affichée par le cœur, est envisagée plus tard.
- Un fichier distant écrasé par un envoi puis annulé n'est pas restaurable (pas de renommage atomique côté serveur ; seul un fichier **créé** par le transfert est supprimé à l'annulation).
- Un téléchargement n'est pas plafonné par la taille annoncée : un fichier qui grossit pendant le transfert est légitime (journaux) ; l'annulation reste possible.
- La détection de conflit d'édition repose sur la taille et la date distantes, précises à la seconde : deux modifications de même taille dans la même seconde passent inaperçues.
- L'éditeur externe n'est pas fermé au verrouillage : la copie locale est supprimée et plus rien n'est renvoyé, mais la fenêtre de l'éditeur peut encore afficher le contenu.
- Les comptes « SFTP seul » (`ForceCommand internal-sftp`) ne sont probablement pas utilisables proprement, faute de terminal : à vérifier sur le lab (`testsftp`).
- Fichiers téléchargés en 0600 sous Linux, sans reprendre les droits distants (exécution comprise).
- Sous Windows, les fichiers téléchargés portent la Mark-of-the-Web (`Zone.Identifier`, `ZoneId=3`) : SmartScreen et le mode protégé d'Office s'appliquent.

## ADR-035 — Panneaux (split) : une session par panneau, arbre de disposition par onglet
**Statut** : Acceptée (24/11/2026) — prise en autonomie, à relire par le porteur
**Contexte** : SSH-07 demande un écran partagé horizontal et vertical, redimensionnable. Il faut décider si un panneau partage la connexion de l'onglet ou en ouvre une, comment le nouveau panneau s'authentifie, et comment dessiner les séparateurs.
**Décision** :
- **Chaque panneau est une session complète**, avec sa propre connexion SSH, son `TerminalSession`, son état et son panneau SFTP. Aucune nouvelle commande IPC : chaque panneau passe par `ssh_connect` ou `ssh_connect_host` (TOFU strict, 2FA, limite de sessions, fermeture au verrouillage inchangés).
- **Arbre de disposition par onglet** (`src/features/terminal/layout.ts`, fonctions pures) : feuilles = panneaux, nœuds = séparations `row` / `column` avec un ratio (10 % minimum de chaque côté). Le store `sessions` garde `layouts` (par onglet), `focusedPane` (par onglet) et, pour chaque panneau secondaire, `paneOf` (son onglet). Seul le panneau racine apparaît dans la barre d'onglets.
- **8 panneaux au plus par onglet** : au-delà, les terminaux deviennent illisibles et chaque panneau ouvre une connexion.
- **Nouveau panneau** : il reprend la cible du panneau d'origine. Hôte du carnet : connexion directe avec l'identité du coffre, aucun secret dans l'interface. Connexion rapide : formulaire pré-rempli, mot de passe ou passphrase redemandé (comme ADR-021).
- **Fermeture du panneau racine** : le panneau suivant est promu et devient l'onglet ; les autres panneaux restent ouverts. Fermer un panneau secondaire laisse sa place à son voisin.
- **Séparateurs écrits à la main** (`SplitView.tsx`) : souris et clavier (Tab puis flèches, Début, Fin), sans dépendance.

**Alternatives écartées** :
- Plusieurs canaux sur une seule connexion : couplerait la durée de vie des panneaux et ne permettrait pas de viser un autre serveur ; gain faible.
- `react-resizable-panels` : une dépendance de plus dans la chaîne d'approvisionnement pour quelques dizaines de lignes.
- Mémoriser le mot de passe d'une connexion rapide pour ouvrir le panneau sans le redemander : contraire à la règle « aucun secret dans l'interface ».

**Conséquences** :
- Une grille de 4 panneaux ouvre 4 connexions et compte 4 sessions dans la limite du cœur (64).
- Chaque panneau a sa propre clé d'hôte vérifiée et sa propre 2FA : un hôte « mot de passe + code » demande un code par panneau.
- La disposition n'est pas persistée : elle disparaît à la fermeture de l'onglet et au verrouillage.

## ADR-036 — Diffusion de la saisie côté interface, cibles explicites
**Statut** : Acceptée (24/11/2026) — prise en autonomie, à relire par le porteur
**Contexte** : SSH-08 demande d'envoyer la même saisie à plusieurs sessions, avec un indicateur visuel fort. La menace M7 (`SECURITE.md` §2) est d'envoyer des commandes au mauvais serveur, ou d'oublier le mode actif. Un serveur ne doit pas pouvoir écrire dans les autres sessions par les réponses automatiques du terminal.
**Décision** :
- **Entièrement côté interface**, au-dessus de `ssh_write` (store `broadcast`, `TerminalSession.handleInput`) : **aucune nouvelle commande IPC**, donc aucun nouveau privilège. Une interface compromise pouvait déjà écrire dans chaque session. La saisie d'une cible est recopiée dans la file d'entrée (`InputQueue`, ADR-024) de chaque autre cible, qui garde son contrôle de flux.
- **Cibles cochées explicitement**, dans tous les onglets, après un dialogue qui liste chaque session connectée par **nom et adresse** (`utilisateur@hôte:port`). Seule la session active est cochée d'office. Il faut **au moins 2 cibles**. Rien n'est jamais persisté : la diffusion repart inactive à chaque démarrage.
- **Arrêt** : une cible déconnectée ou fermée est retirée **définitivement** (une reconnexion, qui peut viser un autre serveur, ne la réintègre pas) ; arrêt automatique sous 2 cibles et au verrouillage ; arrêt manuel en un clic (bandeau) ou par Ctrl+Maj+I. L'activation passe toujours par le dialogue.
- **Indicateurs** : bandeau rouge en haut de la fenêtre, visible depuis toutes les sections ; bordure rouge épaisse et en-tête « DIFFUSION » sur chaque panneau cible ; pastille sur les onglets concernés.
- **Collage diffusé toujours confirmé**, même sur une ligne : le dialogue liste les serveurs. Chaque cible reçoit le texte encadré selon **son propre** mode « bracketed paste ».
- **Seule la saisie de l'utilisateur est diffusée** : des données d'xterm ne sont recopiées que dans les 100 ms qui suivent une frappe, et jamais si elles ressemblent à une réponse automatique (position du curseur, état, modes, identification, focus, souris, chaînes OSC et DCS). Les rapports souris binaires ne sont jamais diffusés. Les envois de snippets et l'intégration au prompt restent dans leur session.

**Alternatives écartées** :
- Commande IPC de diffusion côté cœur : nouveau privilège sans gain, le cœur ne sait pas mieux que l'interface quelles frappes viennent de l'utilisateur.
- « Tous les panneaux de l'onglet » par défaut : trop facile de viser un serveur sans le voir.
- Réintégrer une cible reconnectée : la reconnexion peut aboutir sur un autre serveur ou un autre compte.

**Conséquences et limites** :
- Maj+F3 produit la même séquence qu'une réponse de position du curseur : il n'est pas diffusé.
- L'attribution « frappe de l'utilisateur » repose sur une fenêtre de 100 ms : à surveiller avec les méthodes de saisie (IME) et AltGr. Une réponse automatique qui tombe dans cette fenêtre et échappe au filtre serait diffusée.

## ADR-037 — Snippets dans le coffre, variables sans échappement automatique
**Statut** : Acceptée (24/11/2026) — prise en autonomie, à relire par le porteur. Prolonge ADR-027 et ADR-030 (contenu du coffre).
**Contexte** : SNP-01 à SNP-03 : bibliothèque de commandes réutilisables, variables `{{nom}}` demandées à l'envoi, envoi vers une ou plusieurs sessions. Un snippet ou une valeur de variable peut contenir des séquences d'échappement qui font sortir du collage encadré, ou des caractères invisibles qui cachent une commande.
**Décision** :
- **Stockage dans le coffre** (`core-vault`, `model.rs`), champ `snippets` avec `#[serde(default)]`, **sans changer `SCHEMA_VERSION`** : un coffre sans snippets se lit toujours. Une ancienne version de Younji refuse le fichier (`deny_unknown_fields`) au lieu d'ignorer le champ : pas de perte silencieuse. Deux commandes : `snippet_save`, `snippet_delete` ; la liste arrive avec `vault_snapshot`.
- **Pas des secrets** : les snippets sont affichés et envoyés en clair au terminal ; le formulaire le rappelle. Ils restent chiffrés au repos.
- **Contenu validé par le cœur** et revalidé dans l'interface : 1 à 16 Kio, aucun caractère de contrôle hors `\n` et `\t`, aucun caractère invisible (contrôles bidirectionnels, largeur nulle, BOM), 32 variables au plus, noms `[A-Za-z_][A-Za-z0-9_-]{0,31}`, 1 000 snippets au plus. Le contenu est **revérifié au chargement du coffre** (`check_loaded`).
- **Valeurs des variables** : une seule ligne, sans caractère de contrôle ni invisible. **Aucun échappement shell automatique** : il changerait la commande de façon peu lisible. L'aperçu, obligatoire, montre exactement ce qui part.
- **Envoi** : « Insérer » par défaut (sans Entrée), « Exécuter » sur clic explicite. Cibles cochées une à une, par nom et adresse. L'envoi **ignore la diffusion** : seules les sessions cochées reçoivent le texte, encadré comme un collage si leur shell l'a demandé.

**Alternatives écartées** :
- Fichier de snippets séparé, en clair : exposerait des commandes qui révèlent l'infrastructure (noms de services, chemins).
- Nouvelle version de schéma avec migration : inutile pour un ajout pur.
- Échappement shell des valeurs : dépend du shell distant (bash, fish, PowerShell) et rend l'aperçu trompeur.

**Conséquences** :
- Le contenu étant revérifié au chargement, **durcir une règle plus tard** (taille, caractères admis) exigera une migration, ou d'écarter les snippets devenus invalides, pour ne pas rendre un coffre existant illisible.
- Une valeur de variable peut contenir des métacaractères du shell (`;`, `$( )`) : c'est voulu, l'aperçu les montre.
- Le champ `snippets` est toujours écrit, même vide : un coffre enregistré une fois par cette version n'est plus lisible par une version antérieure à l'étape 5 (refus propre, sans perte).

## ADR-038 — Retrait des tunnels SSH (étape 6)
**Statut** : Acceptée (25/11/2026) — décision du porteur. Retire TUN-01 à TUN-03 et l'étape 6 ; ADR-002 reste valable pour SSH et SFTP.
**Contexte** : l'étape 6 (redirections `-L`, `-R`, `-D`/SOCKS5, gestionnaire de tunnels) a été réalisée sur la branche `etape/6-tunnels` (PR mlft9/younji#14). Le porteur la juge non nécessaire et trop complexe pour le début du projet : nouvelle surface réseau (écoutes sur le poste, proxy SOCKS, canaux ouverts par le serveur), confirmation native de l'exposition, recette lourde.
**Décision** :
- La PR est fermée sans fusion ; la branche `etape/6-tunnels` est conservée comme archive.
- TUN-01 à TUN-03 passent dans le hors périmètre du cahier des charges (§2.3). La section « Tunnels » de la barre latérale, qui n'affichait qu'un emplacement réservé, est supprimée.
- L'étape 6 est marquée ❌ dans la feuille de route ; son numéro n'est pas réattribué, pour ne pas décaler les références aux étapes suivantes (16, 18…).
- Les règles de `SECURITE.md` §4.4 sont conservées, signalées comme sans objet tant que la fonction est absente : elles s'appliqueraient telles quelles à son retour.

**Alternatives écartées** :
- Fusionner puis masquer la fonction : du code et des permissions IPC inutilisés, donc de la surface d'attaque sans bénéfice.
- Renuméroter les étapes : casse les références croisées (ROADMAP, ADR, plans) pour un gain nul.

**Conséquences** :
- Moins de surface d'attaque : aucune écoute réseau ouverte par l'application, aucune permission IPC de tunnel.
- Le bouton panique (étape 16) et la palette de commandes (étape 18) ne mentionnent plus les tunnels.
- Pour rouvrir le sujet, un nouvel ADR remplacera celui-ci ; la branche archivée sert de point de départ (revue de sécurité déjà passée, verdict OK).

## ADR-039 — Terminal local : profils détectés par le cœur, commandes `term_*` séparées
**Statut** : Acceptée (25/11/2026) — prise en autonomie, à relire par le porteur. Prolonge ADR-024 (flux et acquittement), ADR-026 (verrouillage) et ADR-028 (application derrière le déverrouillage).
**Contexte** : LOC-01 et LOC-02 demandent d'ouvrir les shells du poste dans un onglet (PowerShell, PowerShell 7, CMD et distributions WSL sous Windows ; shell par défaut, bash, zsh sous Linux). C'est la première fonction qui **lance un programme local** à la demande de l'interface, que l'on considère comme non fiable : une interface compromise ne doit pas pouvoir s'en servir pour exécuter un programme de son choix.
**Décision** :
- **Profils détectés par le cœur, désignés par identifiant** (`core-term`, `profiles.rs`) : `term_profiles` lance la détection et la mémorise ; `term_open` n'accepte qu'un identifiant de cette liste. Programme et arguments sont reconstruits côté Rust et ne sont jamais envoyés à l'interface. Le programme est lancé directement, arguments séparés, **sans shell intermédiaire**. Sous Linux, `$SHELL` n'est retenu comme défaut que s'il est absolu et figure dans `/etc/shells` ; seuls les noms connus (bash, zsh, fish, ksh, tcsh, dash, sh, nu) sont proposés. Sous Windows, programmes pris dans `%SystemRoot%\System32` et `%ProgramFiles%\PowerShell`, distributions lues par `wsl.exe -l -q` (5 s, 64 Kio au plus) et validées (lettres, chiffres, `.`, `_`, `-`, 64 caractères, sans tiret initial), `docker-desktop*` écartées.
- **Profils non modifiables** : pas de programme ni d'argument personnalisé. Un champ libre reviendrait à exposer l'exécution arbitraire à l'interface.
- **Commandes IPC séparées** (`term_profiles`, `term_open`, `term_write`, `term_resize`, `term_ack`, `term_close`) plutôt que de réutiliser les `ssh_*` : registre distinct (`LocalTerminals`, 32 terminaux), si bien qu'une commande SFTP ou de clé d'hôte ne peut jamais viser un terminal local. Préfixe `term_`, car `local_pick_root` et `local_list` désignent déjà les dossiers locaux du SFTP. Côté interface, `TerminalSession` choisit un transport (SSH ou local) à l'ouverture ; le reste du terminal est commun.
- **Derrière le coffre et fermé au verrouillage** : `term_profiles` et `term_open` refusent un coffre verrouillé ; verrouillage manuel, inactivité et rechargement ferment tous les terminaux locaux et tuent leurs processus, comme les sessions SSH. Un poste verrouillé n'offre pas de shell.
- **Pas de diffusion, de snippet ni de SFTP vers un terminal local** : ces fonctions visent des serveurs désignés par adresse ; y mêler le shell du poste augmente le risque d'erreur. Réversible.
- **Dossier de départ** : dossier personnel de l'utilisateur. **Environnement** : hérité de l'utilisateur (c'est son propre shell), plus `TERM=xterm-256color` et `COLORTERM=truecolor` ; aucune variable propre à Younji.
- **Arrêt garanti** : seul le fil d'attente possède le processus. À la fermeture, il le tue avant de l'avoir récupéré (SIGHUP, court délai, puis SIGKILL sous Linux ; `TerminateProcess` sous Windows), puis ferme le PTY. Aucun signal ne peut viser un PID déjà libéré et réattribué.
- **Flux** : mêmes `Channel` que SSH (octets bruts, événement `disconnected`), même contrôle de flux par acquittement (2 Mio non acquittés au plus). Aucun contenu ni commande journalisé.

**Alternatives écartées** :
- Recevoir de l'interface un chemin de programme ou une ligne de commande : exécution arbitraire depuis une WebView compromise.
- Lancer le shell via `sh -c` ou `cmd /c` : ajoute une interprétation, donc un risque d'injection, sans bénéfice.
- Réutiliser les commandes `ssh_*` avec un type de session : une seule confusion d'identifiant suffirait à envoyer une commande SFTP ou une décision de clé d'hôte au mauvais endroit.
- Terminal local accessible coffre verrouillé : un poste laissé verrouillé offrirait un shell ; incohérent avec ADR-026 et ADR-028.
- `Child::kill` appelé depuis un autre fil (« killer » cloné de portable-pty) : n'envoyait qu'un SIGHUP sous Linux, ignoré par certains programmes (revue de sécurité de l'étape 7).

**Conséquences** :
- Un shell installé hors des emplacements détectés (autre dossier, profil personnalisé) n'est pas proposé ; l'ajouter demandera une décision explicite (liste validée par le cœur), pas un champ libre.
- Un programme laissé en arrière-plan par le shell (`cmd &`, `trap '' HUP; cmd`) peut survivre à l'arrêt du shell et garder le PTY ouvert ; le fil de lecture du terminal concerné reste alors bloqué jusqu'à sa fin. Pas de fuite de données ; limite documentée dans `SECURITE.md`.
- Trois fils système par terminal local (lecture, écriture, attente) ; le registre est limité à 32 terminaux.
- Les variables d'environnement de l'utilisateur, y compris d'éventuels secrets qu'il y a placés lui-même, sont transmises à son shell, comme dans tout terminal.

## ADR-040 — RDP v1 : identifiants Windows de type domaine écrits par l'API, seul module `unsafe` du projet
**Statut** : Acceptée (25/11/2026) — prise en autonomie, à relire par le porteur. Met en œuvre ADR-007 (premier temps) ; prolonge ADR-026 (verrouillage) et ADR-030 (identités).
**Contexte** : RDP-02 demande d'ouvrir un hôte RDP du carnet dans le client du système (`mstsc` sous Windows, FreeRDP sous Linux) **sans jamais passer le mot de passe en ligne de commande** (`SECURITE.md` §4.3). mstsc n'accepte pas de mot de passe dans un fichier `.rdp` utilisable : il le lit dans le gestionnaire d'identifiants de Windows, sous la cible `TERMSRV/<hôte>`. Sous Linux, FreeRDP tourne sans terminal : il ne peut poser aucune question à l'utilisateur.
**Décision** :
- **Windows : identifiant de type « domaine » écrit par l'API Win32** (`CredWriteW`, `CredReadW`, `CredEnumerateW`, `CredDeleteW`, module `crates/core-rdp/src/wincred.rs`) : type `CRED_TYPE_DOMAIN_PASSWORD`, cible `TERMSRV/<hôte>`, persistance `CRED_PERSIST_SESSION` (jamais écrit sur disque ni itinérant). Le mot de passe d'un tel identifiant n'est utilisable que par LSA (mstsc via CredSSP) : un autre programme de la session ne peut pas le relire par `CredRead`. Jamais `cmdkey /pass:`, qui mettrait le mot de passe dans les arguments d'un processus.
- **`unsafe` limité à ce module** : la règle `unsafe_code` du workspace passe de `forbid` à `deny` **pour la seule crate `core-rdp`**, et seul `wincred` l'autorise localement (`#![allow(unsafe_code)]`). Clippy `undocumented_unsafe_blocks = "deny"` impose un commentaire `SAFETY:` sur chaque bloc. Toute autre utilisation d'`unsafe` dans la crate, ou ailleurs dans le workspace, reste une erreur de compilation.
- **Marqueur non secret** : chaque identifiant écrit par Younji porte le commentaire `Younji (temporaire)`. Il sert à distinguer un identifiant de Younji d'un identifiant enregistré par l'utilisateur.
- **Durée de vie** : l'identifiant et le fichier `.rdp` sont supprimés à la fin de mstsc, au plus tard 60 s après le lancement (mstsc lit l'identifiant en début de connexion), au verrouillage (clients arrêtés, ADR-026), à la fermeture de Younji (`RunEvent::Exit`, car Tauri termine le processus sans détruire l'état géré) et, après un arrêt brutal, au démarrage suivant (suppression des `TERMSRV/*` portant le marqueur).
- **Identifiant de l'utilisateur jamais touché** : si `TERMSRV/<hôte>` existe sans le marqueur, Younji ne l'écrase pas, ne le supprime pas, et mstsc l'utilise ; l'interface le signale.
- **Un seul lancement à la fois par serveur** : un second lancement vers la même cible, pendant que l'identifiant temporaire du premier existe (même instance ou autre instance de Younji), est refusé (`rdp.busy`) au lieu d'écraser l'identifiant avec un autre compte.
- **Linux : `xfreerdp3` (FreeRDP 3) seul**, cherché dans les seuls dossiers absolus du `PATH`. Mot de passe écrit sur l'entrée standard (`/from-stdin:force`), tube fermé aussitôt ; certificat en `/cert:tofu,deny` (premier certificat mémorisé, changement refusé sans question, comme les clés d'hôte SSH) ; mot de passe obligatoire dans l'identité. FreeRDP 2 (`xfreerdp`) n'est pas recherché : il ne connaît pas `/cert:tofu,deny`.
- **Clients arrêtés** au verrouillage, au rechargement de l'interface et à la fermeture de Younji, sur les deux systèmes.
- **Redirections limitées au presse-papiers** : ni disques, ni imprimantes, ni cartes à puce, ni ports.

**Alternatives écartées** :
- Crate `keyring` : elle écrit des identifiants **génériques**, dont le mot de passe est relisible en clair par tout programme de la session (`CredRead`).
- `cmdkey /add:… /pass:…` : mot de passe visible dans la liste des processus.
- Mot de passe dans le fichier `.rdp` (`password 51:b:`, chiffré par DPAPI) : fichier sur disque contenant un secret déchiffrable par toute la session.
- Linux : `/p:` en argument (visible dans `ps`), variable d'environnement (lisible dans `/proc/<pid>/environ`), `/cert:ignore` (aucune vérification du serveur), question interactive du certificat (impossible sans terminal).
- Prendre en charge FreeRDP 2 : il ne connaît ni `/cert:tofu,deny` ni le même dialogue sur l'entrée standard.

**Conséquences** :
- Premier et seul code `unsafe` de Younji : quelques blocs FFI documentés, compilés sous Windows seulement, à relire à chaque revue de sécurité. `windows-sys` (MIT OR Apache-2.0, déjà dans l'arbre via Tauri) est ajouté à `core-rdp` pour Windows seulement.
- Si l'utilisateur met plus de 60 s à accepter un certificat inconnu dans mstsc, l'identifiant a disparu : mstsc redemande le mot de passe. Limite assumée (secret présent le moins longtemps possible).
- Sous Linux, un hôte RDP sans mot de passe dans son identité ne peut pas être ouvert (`rdp.password_required`) ; sous Windows, mstsc le demande lui-même.
- La protection du certificat RDP sous Linux repose sur le magasin de FreeRDP (`~/.config/freerdp/`), pas sur le coffre ; la vérification TOFU intégrée viendra avec IronRDP (étape 12).

## ADR-041 — Import OpenSSH et sauvegarde chiffrée
**Statut** : Acceptée (25/11/2026) — prise en autonomie, à relire par le porteur. Met en œuvre ADR-011 (export/import chiffré) ; prolonge ADR-005 (format du coffre), ADR-020 (fichiers choisis par le cœur), ADR-023 et ADR-027 (TOFU strict, clés d'hôte dans le coffre).
**Contexte** : IMP-01 et IMP-02 demandent de reprendre `~/.ssh/config` et `~/.ssh/known_hosts` ; VLT-06, d'exporter et de réimporter le coffre avec une phrase de passe distincte, en fusion ou en remplacement. Ces trois fonctions font entrer dans le coffre des données venues de l'extérieur : un fichier de config piégé, un `known_hosts` qui glisse la clé d'un attaquant, une sauvegarde fabriquée par quelqu'un qui connaît sa phrase de passe.
**Décision** :
- **Import en deux temps, aperçu gardé dans le cœur** : `import_*_preview` lit le fichier (chemin OpenSSH par défaut ou fichier choisi dans la boîte de dialogue ouverte par le cœur), calcule l'aperçu et le garde dans `ImportState` ; `import_*_apply` ne reçoit qu'un identifiant d'aperçu et des **indices**. L'interface ne peut pas glisser d'autres hôtes ni d'autres clés. Un aperçu sert une fois, est remplacé par le suivant et oublié au verrouillage ou au rechargement.
- **`~/.ssh/config`** (crate `core-import`, sans Tauri) : `Host` (motifs, négations), `HostName`, `User`, `Port`, `IdentityFile`, `Include` (relatif à `~/.ssh`, jokers `*` et `?`) ; configuration effective calculée comme OpenSSH (**première valeur obtenue gagne**, blocs à jokers compris). Les blocs `Match` ne sont **pas pris en charge** : signalés, jamais interprétés. Seuls `%h` et `%%` sont développés ; tout autre jeton fait ignorer l'hôte. Chaque hôte est revalidé comme une saisie (`Target::new`, `upsert_host`) puis rangé dans le groupe « Import OpenSSH », sans identité. Bornes : profondeur d'`Include` 16, 256 fichiers, 1 Mio par fichier, 50 000 lignes, 10 000 alias.
- **Clés privées `IdentityFile` jamais lues** : leur chemin est signalé pour un import manuel (Clés > Importer, boîte de dialogue et passphrase). L'import ne touche pas au secret le plus sensible du poste.
- **Utilisateur absent** : utilisateur local (`USER` / `USERNAME`), comme OpenSSH, visible dans l'aperçu.
- **Hôtes déjà présents** (même adresse, port et utilisateur) : décochés et signalés, pas de doublon. `ProxyJump`, `ProxyCommand` et les redirections sont ignorés avec un avertissement par hôte : l'hôte importé se connectera directement.
- **`known_hosts` : jamais une autre clé pour un hôte connu**. Une ligne dont un nom en clair est déjà dans le coffre est **refusée** (conflit signalé) ; une ligne identique est un doublon ignoré. `@cert-authority` et les motifs à joker ou négation sont ignorés (non pris en charge) ; `@revoked` est accepté (il ne fait que restreindre). Chaque clé est revalidée (`is_valid_public_key` de `core-ssh` : clé décodable, algorithme admis, DSA refusé). Une ligne à nom haché (`|1|…`) est **refusée** : la vérification des connexions ne consulte pas les noms hachés (ADR-023), elle n'aurait aucun effet (et un `@revoked` haché ne révoquerait rien). L'application est refusée si les clés d'hôte du coffre ont changé depuis l'aperçu.
- **Sauvegarde `YNJB`** : même format que le coffre (`format.rs` paramétré par la magie), avec un **sel et une DEK neufs**. Une sauvegarde ne s'ouvre pas comme un coffre, ni l'inverse.
- **Phrase de passe distincte** : même politique que le mot de passe maître (12 caractères, zxcvbn ≥ 3), et **refus si elle ouvre le coffre courant** (`vault.backup_passphrase_reused`). Ce test dérive une clé avec le mot de passe proposé : pour qu'il ne serve pas d'oracle sur le mot de passe maître, la **boîte de dialogue d'enregistrement précède toute dérivation** (action humaine) et chaque export **compte comme une tentative** dans le compteur et le délai progressif du déverrouillage.
- **Revalidation complète à l'import** : en-tête et tag AEAD, puis chaque entrée comme une saisie (`validate_all` : noms, adresses, protocoles, snippets, unicité des identifiants, intégrité référentielle), **délai de verrouillage** compris (une des valeurs proposées, aussi revérifiée au chargement du coffre), et **algorithme, empreinte et clé publique de chaque clé recalculés** depuis la clé privée, qui doivent correspondre.
- **Fusion par défaut, sans écrasement** (`merge_from`) : seules les entrées absentes sont ajoutées (même identifiant : la version actuelle gagne ; hôte de même adresse, port et utilisateur : ignoré) ; réglages actuels conservés ; clés d'hôte fusionnées avec la règle de conflit ci-dessus.
- **Remplacement confirmé** : tout le contenu du coffre est remplacé par celui de la sauvegarde, **clés d'hôte et délai de verrouillage compris** ; le mot de passe maître ne change pas et la version précédente reste dans `vault.younji.bak`. La confirmation le dit et recommande de ne l'utiliser qu'avec sa propre sauvegarde.

**Alternatives écartées** :
- Importer les clés `IdentityFile` automatiquement : lecture de clés privées et demandes de passphrase en série, pour un gain de quelques clics.
- Interpréter `Match` : dépend de l'exécution de commandes (`Match exec`) et du contexte réseau ; une interprétation partielle importerait des valeurs fausses.
- Remplacer la clé d'un hôte connu par celle du `known_hosts` importé, ou demander à l'utilisateur : un fichier piégé pourrait faire accepter la clé d'un attaquant en un clic (ADR-023).
- Permettre le mot de passe maître comme phrase de passe de sauvegarde : une fuite de la sauvegarde exposerait le coffre, et inversement.
- Envoyer à l'interface l'aperçu complet puis recevoir les hôtes à créer : une interface compromise pourrait y ajouter ce qu'elle veut.
- Fusion qui écrase les entrées existantes : perte silencieuse de modifications locales.

**Conséquences** :
- **Limite** : un `known_hosts` entièrement haché (`HashKnownHosts yes`) n'apporte aucune clé ; ses hôtes passent par la confirmation TOFU à la première connexion.
- **« Remplacer » fait confiance aux clés d'hôte de la sauvegarde** : c'est une restauration, réservée à sa propre sauvegarde (documenté dans la confirmation et dans `SECURITE.md` §3.7).
- Après plusieurs exports d'affilée, le déverrouillage et l'export suivants sont ralentis (compteur partagé) ; un verrouillage suivi d'un déverrouillage réussi le remet à zéro.
- Une sauvegarde d'une version plus récente ou dont le délai de verrouillage n'est pas une valeur proposée est refusée en bloc.
- Aucune nouvelle dépendance : motifs et analyse écrits à la main, crypto du coffre réutilisée.
