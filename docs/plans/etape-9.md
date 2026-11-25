# Plan — Étape 9 : import OpenSSH, sauvegarde chiffrée

> Rédigé en mode autonome (25/11/2026). Branche `etape/9-import-sauvegarde`, partie de `etape/8-rdp-systeme` (étapes 7 et 8 pas encore validées : la PR de l'étape 9 dépend de la leur).

Exigences : IMP-01 (import de `~/.ssh/config` : Host, HostName, User, Port, IdentityFile, Include, avec un rapport des entrées ignorées), IMP-02 (import des clés d'hôte de `~/.ssh/known_hosts`), VLT-06 (export / import d'une sauvegarde chiffrée avec une phrase de passe distincte, fusion ou remplacement).
Références : `SECURITE.md` §3 (coffre), §3.7 (export), §4.1 (TOFU strict, ADR-023/027), §5 (sélection de fichiers par le cœur, ADR-020) ; ADR-011 (pas de synchronisation).

## 1. Architecture retenue

- **Crate `core-import`** (sans Tauri) :
  - `ssh_config.rs` : lecture de `~/.ssh/config` avec `Include` (motifs `*`/`?`, chemins relatifs à `~/.ssh`, profondeur et nombre de fichiers bornés), blocs `Host` (motifs, négations) ; `Match` non pris en charge (signalé). Pour chaque alias concret, calcul de la configuration effective comme OpenSSH (**première valeur obtenue gagne**, blocs à jokers compris). Rapport : hôtes importables, entrées ignorées avec leur raison, avertissements par hôte (`ProxyJump`, `ProxyCommand`, redirections, `IdentityFile` à importer à part).
  - `known_hosts.rs` : analyse et fusion d'un `known_hosts` avec celui du coffre, sans jamais ajouter une deuxième clé à un hôte déjà connu (conflit signalé).
- **Sauvegarde** (`core-vault/src/backup.rs`) : même format que le coffre avec la magie `YNJB`, un **sel et une DEK neufs**, une phrase de passe distincte (même politique que le mot de passe maître, et refus si elle ouvre le coffre courant). Import : déchiffrement (tag AEAD, en-tête authentifié), revalidation complète de chaque entrée, puis **fusion** (ajout des entrées absentes, aucune écrasée) ou **remplacement**.
- **src-tauri** : `import_ssh_config_preview` / `import_ssh_config_apply`, `import_known_hosts_preview` / `import_known_hosts_apply` (l'aperçu reste dans le cœur ; l'interface n'envoie que des indices), `backup_export`, `backup_import`. Fichiers choisis par la boîte de dialogue ouverte par le cœur (ADR-020), ou chemin par défaut `~/.ssh/…`.
- **Interface** : « Importer ~/.ssh/config » dans le carnet (aperçu, cases à cocher, rapport des entrées ignorées), « Importer known_hosts » dans les clés d'hôte, section « Sauvegarde » dans les paramètres.

## 2. Sous-tâches

| # | Sous-tâche | Fichiers | Commit |
|---|---|---|---|
| 1 | Crate `core-import` : `ssh_config` (Include, jokers, rapport) et `known_hosts` (fusion sans conflit), tests avec fixtures `tests/fixtures/test_*` | `crates/core-import/` | `feat(import): …` |
| 2 | Sauvegarde chiffrée `YNJB` : format paramétré par la magie, export, lecture, revalidation, fusion et remplacement | `crates/core-vault/src/{format,backup,model}.rs` | `feat(vault): …` |
| 3 | IPC : aperçus et applications d'import, export et import de sauvegarde, capability, isolation | `src-tauri/` | `feat(ipc): …` |
| 4 | Interface : dialogues d'import, section Sauvegarde, FR/EN, tests | `src/` | `feat(ui): …` |
| 5 | Contrôles et documentation | — | `docs: …` |

## 3. Dépendances

Aucune nouvelle dépendance externe (motifs `*`/`?` et analyse écrits à la main ; crypto existante du coffre).

## 4. Risques de sécurité et parades

| Risque | Parade |
|---|---|
| `Include` récursif ou en boucle, fichiers énormes | Profondeur 16, 256 fichiers, 1 Mio par fichier, 50 000 lignes au total ; au-delà, l'élément est ignoré et signalé |
| Hôte importé qui se connecte ailleurs qu'attendu (`ProxyJump` ignoré en silence) | Avertissement explicite par hôte ; l'utilisateur coche ce qu'il importe |
| Jeton `%` non géré dans `HostName` | `%h` et `%%` seuls ; tout autre jeton : hôte ignoré avec la raison |
| Valeurs piégées (caractères de contrôle, adresse invalide) | Mêmes validations que la saisie manuelle (`Target::new`, `upsert_host`) ; entrée refusée et signalée |
| Lecture des clés privées `IdentityFile` | Jamais lues par l'import : chemin signalé pour un import manuel par Clés > Importer (boîte de dialogue, passphrase) |
| Import de `known_hosts` qui ajoute la clé d'un attaquant pour un hôte déjà connu | Une ligne dont un nom d'hôte existe déjà dans le coffre est **refusée** (conflit signalé) ; `@cert-authority` ignoré (non pris en charge) ; `@revoked` conservé ; clés revalidées (algorithme autorisé, base64 décodable) ; aperçu et confirmation |
| Aperçu modifié par l'interface | L'aperçu reste dans le cœur ; l'application ne reçoit que des indices bornés ; un aperçu sert une fois et expire au verrouillage |
| Sauvegarde ouverte avec le mot de passe maître | Phrase de passe distincte imposée (refus si elle ouvre le coffre) et même robustesse exigée |
| Sauvegarde altérée | Tag AEAD (XChaCha20-Poly1305), en-tête (magie, paramètres, sel) en AAD : un octet modifié fait échouer l'import avec un code unique |
| Sauvegarde piégée (paramètres Argon2 énormes, taille annoncée) | Mêmes planchers/plafonds et contrôle de longueur avant allocation que le coffre |
| Sauvegarde contenant des données invalides (fabriquée avec la phrase de passe) | Chaque entrée est revalidée comme une saisie (noms, adresses, protocoles, snippets) et l'intégrité référentielle est vérifiée |
| Écrasement involontaire des données | Fusion par défaut (aucune entrée existante écrasée) ; remplacement après confirmation explicite ; la version précédente du coffre reste dans `.bak` |
| Fichier exporté lisible par d'autres utilisateurs | Écriture atomique, 0600 sous Linux, jamais d'écrasement non confirmé par la boîte de dialogue |
| Phrase de passe en mémoire | `SecretString`, jamais journalisée ni renvoyée |

## 5. Tests

- `core-import` : config réelle avec `Include` (relatif, absolu, joker), `Host *` en fin de fichier qui fournit des valeurs par défaut, négation, `Match` ignoré, première valeur gagnante, `%h`, jetons refusés, ports invalides, boucle d'`Include`, bornes ; `known_hosts` : ajout, doublon, conflit, hachés, marqueurs.
- `core-vault` : aller-retour export/import identique, un octet modifié rejeté (en-tête, nonce, données, tag), magie du coffre refusée comme sauvegarde, phrase de passe faible ou égale au mot de passe maître refusée, fusion sans écrasement, remplacement, références pendantes.
- `src-tauri` : désérialisation stricte, indices hors bornes.
- Interface : aperçu et sélection de l'import, raisons affichées, export (confirmation de la phrase de passe), import (mode), FR/EN.

## 6. Choix tranchés en autonomie

1. **Clés privées (`IdentityFile`) non importées automatiquement** : leur lecture demande souvent une passphrase et touche le secret le plus sensible du poste ; elles sont listées pour un import manuel.
2. **Utilisateur absent de la config** : nom de l'utilisateur local (comportement d'OpenSSH), visible dans l'aperçu.
3. **Hôtes déjà présents** (même adresse, port et utilisateur) : décochés et signalés, pas de doublon.
4. **Conflits de `known_hosts` refusés**. D'abord, les lignes hachées n'étaient ajoutées que si elles étaient absentes à l'identique ; après la revue de la documentation, elles sont **refusées** (raison `hashed`) : la vérification des connexions ne consulte pas les noms hachés, elles n'auraient eu aucun effet.
5. **Fusion de sauvegarde** : les entrées existantes (même identifiant) gagnent ; réglages du coffre (délai de verrouillage) conservés ; clés d'hôte fusionnées avec la même règle de conflit.
6. **Phrase de passe de sauvegarde** : même politique que le mot de passe maître (12 caractères, zxcvbn ≥ 3).

## 7. Écarts en cours d'étape (revue de sécurité)

- **Verrouillage automatique désactivable par une sauvegarde fabriquée (Moyenne)** : le délai de verrouillage n'était pas revalidé. Il doit désormais être une des valeurs proposées (1 à 60 min), au chargement du coffre comme à l'import ; sinon le fichier est refusé.
- **Fichiers spéciaux (Faible)** : `Include`, `known_hosts` et sauvegarde ne lisent que des fichiers ordinaires (type vérifié avant l'ouverture puis sur le descripteur), avec une lecture bornée quelle que soit la taille annoncée.
- **Oracle sur le mot de passe maître (Faible)** : chaque export de sauvegarde compte comme une tentative et partage le compteur et le délai progressif du déverrouillage (en plus de la boîte de dialogue qui précède toute dérivation).
- **Empreintes trompeuses (Faible)** : l'algorithme, l'empreinte et la clé publique de chaque clé d'une sauvegarde sont recalculés depuis la clé privée et doivent correspondre.
- **« Remplacer » fait confiance aux clés d'hôte de la sauvegarde (Faible, documenté)** : c'est une restauration ; le message de confirmation le dit et recommande de ne l'utiliser qu'avec sa propre sauvegarde. La fusion, elle, applique la règle de conflit.
