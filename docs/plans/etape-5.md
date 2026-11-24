# Plan — Étape 5 : split, snippets, diffusion multi-sessions

> Rédigé en mode autonome (24/11/2026). Branche `etape/5-split-snippets-diffusion`, partie de `main` (étape 4 fusionnée, PR #11).

Exigences : SSH-07 (split horizontal/vertical, redimensionnable), SSH-08 (diffusion de la saisie avec indicateur visuel fort), SNP-01 (bibliothèque de snippets dans le coffre), SNP-02 (variables `{{nom}}` demandées à l'exécution), SNP-03 (envoi vers la session active ou plusieurs sessions).
Références : `SECURITE.md` §2 (M7 : diffusion vers les mauvais serveurs), §6 (diffusion) ; ADR-024 (flux du terminal et acquittement), ADR-026 (verrouillage), ADR-027/030 (contenu du coffre).

## 1. Architecture retenue

- **Panneaux** : un onglet de terminal contient un **arbre de disposition** (feuilles = panneaux, nœuds = séparations horizontales ou verticales avec un ratio). Chaque panneau est une session à part entière (enregistrement `SessionTab` avec son propre `TerminalSession`, son état de connexion, son panneau SFTP) ; seul le panneau racine apparaît dans la barre d'onglets. Aucun nouvel appel au cœur : chaque panneau ouvre sa propre connexion avec les commandes existantes.
- **Diffusion** : entièrement côté interface, par-dessus `ssh_write` (aucune nouvelle commande IPC, donc aucun nouveau privilège). La saisie tapée dans un terminal **cible** est recopiée dans la file d'entrée de chaque autre cible connectée (chaque file garde son contrôle de flux).
- **Snippets** : stockés dans le coffre chiffré (`core-vault`, champ `snippets` du schéma), validés côté Rust (bornes, caractères de contrôle, noms de variables). Deux commandes : `snippet_save`, `snippet_delete` ; la liste arrive avec `vault_snapshot`. La substitution des variables et l'envoi se font dans l'interface (le snippet n'est pas un secret), avec un aperçu obligatoire du texte final.

## 2. Sous-tâches (dans l'ordre)

| # | Sous-tâche | Fichiers / crates | Commit(s) |
|---|---|---|---|
| 1 | **Modèle des snippets** : `Snippet { id, name, content, description }`, CRUD validé (nom 1–128, contenu 1–16 Kio sans contrôle hors `\n`/`\t`, 32 variables max, noms `[A-Za-z_][A-Za-z0-9_-]{0,31}`), 1 000 snippets max, lecture tolérante d'un coffre sans snippets | `crates/core-vault/src/model.rs`, `error.rs` | `feat(vault): …` |
| 2 | **IPC** : `snippet_save`, `snippet_delete`, snippets dans `SnapshotDto` ; `build.rs`, capability, isolation, codes d'erreur | `src-tauri/src/commands/book.rs`, `error.rs`, `lib.rs`, `build.rs`, `capabilities/main.json`, `isolation/index.js` | `feat(ipc): …` |
| 3 | **Split** : arbre de disposition dans le store, ouverture d'un panneau (à droite / en bas, 8 panneaux max par onglet), fermeture (le voisin prend la place), séparateurs redimensionnables à la souris et au clavier, focus par clic, navigation `Ctrl+Maj+flèches` (d'abord `Ctrl+Alt+flèches`, abandonné après recette : pris par GNOME), raccourcis `Ctrl+Maj+D` / `Ctrl+Maj+O`, fermeture de panneau `Ctrl+Maj+X` | `src/stores/sessions.ts`, `src/features/terminal/{SplitView,TerminalPane,…}.tsx`, `src/features/shortcuts/`, `src/components/layout/` | `feat(ui): …` |
| 4 | **Diffusion** : dialogue de choix des sessions (≥ 2 connectées) avec **confirmation** listant les cibles, bordure rouge épaisse et bandeau sur chaque cible, bandeau global « Diffusion active » toujours visible avec bouton d'arrêt en un clic, pastille sur les onglets concernés ; arrêt automatique (moins de deux cibles connectées, verrouillage, fermeture) ; raccourci d'arrêt `Ctrl+Maj+I` | `src/stores/broadcast.ts`, `src/features/broadcast/`, `TerminalSession.ts` | `feat(ui): …` |
| 5 | **Snippets (interface)** : section Snippets (liste, recherche, création, modification, suppression), dialogue d'envoi (valeurs des variables, aperçu, cibles : session active ou plusieurs, « Insérer » ou « Exécuter »), accès depuis un panneau (`Ctrl+Maj+S`, menu contextuel) | `src/features/snippets/`, `src/lib/{ipc,types}.ts`, `src/stores/vault.ts`, locales | `feat(ui): …` |
| 6 | `npm run check`, revue de sécurité, QA, documentation | — | `fix(security): …`, `docs: …` |

## 3. Dépendances à ajouter

Aucune. Les séparateurs redimensionnables sont écrits à la main (quelques dizaines de lignes, pointeur + clavier) plutôt qu'avec `react-resizable-panels` : surface d'attaque et chaîne d'approvisionnement inchangées.

## 4. Risques de sécurité et parades

| Risque | Parade |
|---|---|
| Diffusion vers les mauvais serveurs (M7) | Activation seulement après un dialogue qui **liste les cibles par nom** et demande une confirmation explicite ; aucune cible cochée par défaut sauf la session active ; état jamais persistant (désactivé au démarrage, au verrouillage, à la fermeture) |
| Mode diffusion oublié | Bordure rouge épaisse + bandeau « diffusion » sur chaque terminal cible, bandeau global en haut de la fenêtre (visible depuis tous les onglets) avec bouton « Arrêter », pastille sur les onglets ; arrêt en un clic ou au raccourci |
| Diffusion vers une session reconnectée (autre serveur possible après reconnexion) | Une cible qui se déconnecte est retirée de la diffusion ; une reconnexion ne la réintègre jamais ; sous deux cibles, la diffusion s'arrête |
| Frappes perdues ou désordonnées vers une cible lente | Recopie dans la file d'entrée de chaque cible (`InputQueue`, ADR-024), sans contourner le contrôle de flux |
| Snippet ou valeur de variable qui injecte des séquences d'échappement (sortie du collage encadré, commandes cachées) | Contenu validé côté Rust (aucun caractère de contrôle hors `\n` et `\t`) et revalidé dans l'interface ; valeurs des variables **sans aucun caractère de contrôle** (un retour à la ligne exécuterait la commande avant la fin) ; envoi en collage encadré si le shell l'a demandé |
| Snippet envoyé sans relecture, ou vers plusieurs serveurs par erreur | Aperçu obligatoire du texte final ; « Insérer » (sans Entrée) par défaut, « Exécuter » explicite ; cibles listées par nom ; envoi multiple seulement vers des sessions cochées |
| Snippet qui contient un secret | Les snippets ne sont pas des secrets (ils sont affichés) : l'interface le dit dans le formulaire ; ils restent chiffrés au repos dans le coffre |
| Coffre modifié hors de l'application (snippets trop nombreux, trop longs) | Bornes revérifiées au chargement (`check_loaded`) ; le coffre est de toute façon authentifié |
| Contenu des snippets dans les journaux | Aucun journal du contenu, des noms ni des valeurs |
| Nouvelles commandes IPC | Déclarées dans `build.rs`, accordées une par une, ajoutées à la liste blanche de l'isolation ; désérialisation stricte (`deny_unknown_fields`) |

## 5. Tests prévus
- **core-vault** : CRUD des snippets, validation (contrôles, longueurs, variables), compatibilité de lecture d'un coffre sans snippets, bornes au chargement.
- **src-tauri** : désérialisation stricte de `SnippetRequest`, snapshot sans secret, traduction des nouveaux codes, cohérence `build.rs` / capabilities / isolation (test existant).
- **Interface (Vitest)** : arbre de disposition (ouvrir, fermer, promotion du panneau racine, voisins pour la navigation, borne de 8) ; diffusion (recopie vers les cibles, pas de recopie depuis une non-cible, arrêt sous deux cibles, arrêt à la déconnexion) ; analyse et substitution des variables, refus des contrôles ; dialogue d'envoi (variables demandées avant l'envoi) ; section Snippets (CRUD) ; locales FR/EN.
- **Manuel** : grille de 4 terminaux, diffusion vers 3 VM, snippet `{{service}}` (plan de test de QA).

## 6. Choix tranchés en autonomie (règle « option la plus sûre »)
1. **Diffusion côté interface, sans nouvelle commande** : aucun nouveau privilège IPC ; une interface compromise pouvait déjà écrire dans chaque session.
2. **Cibles explicites** : la diffusion concerne des sessions cochées (dans tous les onglets), pas « tous les panneaux de l'onglet » ; confirmation à chaque activation ; jamais persistée.
3. **Cible déconnectée = retirée** définitivement de la diffusion en cours.
4. **Snippet : « Insérer » par défaut**, « Exécuter » (Entrée ajoutée) seulement sur clic explicite ; aperçu systématique.
5. **Valeurs de variables sur une seule ligne**, sans caractère de contrôle ; aucune échappement shell automatique (il changerait la commande de façon peu lisible) : l'aperçu montre exactement ce qui part.
6. **Schéma du coffre** : champ `snippets` ajouté avec valeur par défaut, sans changer `SCHEMA_VERSION` (ajout pur, lu par la nouvelle version ; une ancienne version refuse le fichier grâce à `deny_unknown_fields`, sans perte silencieuse).
7. **8 panneaux maximum par onglet** : au-delà, chaque terminal devient illisible et chaque panneau ouvre une connexion.
8. **Split sans dépendance** (séparateurs maison).

## 7. Écarts en cours d'étape

- **Base de la branche** : rebasée sur `claude/epic-archimedes-fhqq6d`, qui contient la validation de l'étape 4 et les mises à jour `cookie` / `url` (RUSTSEC-2026-0592 et 0594) pas encore fusionnées dans `main`. Sans elles, `cargo deny check` échoue.
- **Collage en diffusion** (revue de sécurité, Moyenne) : la première version recopiait vers les cibles les octets produits par le terminal source, encadrés selon **son** mode de collage. Désormais tout collage diffusé est confirmé (le dialogue liste les serveurs), et chaque cible reçoit le texte encadré selon son propre mode.
- **Seule la saisie de l'utilisateur est diffusée** (revue, Faible) : les réponses automatiques d'xterm (position du curseur, identification, couleurs, focus), les rapports souris et l'intégration au prompt restent dans leur session. Limite : Maj+F3 produit la même séquence qu'une réponse de position du curseur et n'est pas diffusé.
- **Snippets revérifiés au chargement** et **caractères invisibles** (bidirectionnels, largeur nulle, BOM) refusés dans le contenu et les valeurs (revue, Faible).
- **En-tête de panneau** : en grille, chaque panneau affiche le nom de son serveur (en rouge avec « DIFFUSION » s'il reçoit la diffusion) ; sans cela, on ne savait pas quel panneau visait quel serveur.

### Retours de recette (24/11/2026, version `.deb` sous Ubuntu)

- **Panneau non connecté impossible à fermer** : la barre d'outils n'existait qu'une fois connecté, et l'écran de connexion recouvrait le panneau. Elle est maintenant toujours affichée sur un onglet découpé, au-dessus des écrans de connexion, avec seulement « Fermer » hors connexion.
- **Ctrl+Alt+flèches** change d'espace de travail sous GNOME : la navigation passe à **Ctrl+Maj+flèches**, active seulement depuis un terminal (dans un champ, la combinaison sélectionne des mots).
- **« é » et « è » non envoyés** sous WebKitGTK : xterm ignore ces touches (pas de code de touche exploitable) ; repli sur l'événement `input` non traité (voir `TerminalSession.handleUnhandledInput`).
- **Menu contextuel natif** (Précédent, Suivant, Arrêt, Recharger) : bloqué hors des champs de saisie ; « Recharger » fermait toutes les sessions. Un menu contextuel propre à l'application est reporté à l'étape 18 (`ROADMAP.md`).
