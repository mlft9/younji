# Recette manuelle — étapes 0 à 4

Fiche de test du porteur : ce qu'il faut installer, puis les tests à dérouler, étape par étape.
Coche au fur et à mesure, note ce qui coince (capture d'écran si possible), puis envoie tes retours avec `/retour`.

Légende : 🔒 test de sécurité · ⭐ test prioritaire (le minimum à faire si tu manques de temps).

---

## Partie A — Mise en place

### A1. Ton PC Windows (obligatoire)
- [ ] Prérequis Tauri : **Rust** (via `rustup`), **Node.js LTS**, **Visual Studio Build Tools** (charge « Développement Desktop en C++ »), **WebView2** (déjà présent sur Windows 11).
- [ ] Cloner le dépôt, puis `npm ci`.
- [ ] `npm run check` passe (premier lancement long : compilation Rust).
- [ ] `npm run tauri dev` ouvre la fenêtre Younji.
- [ ] Un éditeur hexadécimal ou Notepad++ (pour regarder le fichier du coffre).
- [ ] Une appli TOTP sur ton téléphone (Aegis, Google Authenticator…), pour la 2FA.

> Le coffre et les réglages sont dans `%APPDATA%\io.github.mlft9.younji\` (`vault.younji`, `settings.json`).
> Pour repartir d'un état « premier lancement », ferme Younji et renomme ce dossier (ne le supprime pas, garde-le en sauvegarde).

### A1 bis. Variante : poste Ubuntu (24.04 ou plus récent)
- [ ] Paquets système nécessaires à Tauri :
  ```bash
  sudo apt update && sudo apt install -y build-essential curl wget file git \
    libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libsoup-3.0-dev libxdo-dev libssl-dev patchelf
  ```
- [ ] **Rust** : `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` (puis rouvrir le terminal). La version est fixée par `rust-toolchain.toml`.
- [ ] **Node.js 22 ou plus** (celui d'`apt` est trop ancien) : par exemple via `nvm`, ou les paquets NodeSource.
- [ ] Cloner le dépôt, `npm ci`, `npm run check`, puis `npm run tauri dev`.
- [ ] Un éditeur de texte graphique installé dans `/usr/bin` (`gnome-text-editor`, fourni par défaut, convient) : il sert à l'édition SFTP.
- [ ] Une appli TOTP sur ton téléphone.

> Le coffre et les réglages sont dans `~/.local/share/io.github.mlft9.younji/` (`vault.younji`, `settings.json`), les journaux dans son sous-dossier `logs/`.
> Pour repartir d'un état « premier lancement », ferme Younji et renomme ce dossier.
> **Fenêtre blanche ou vide** (fréquent avec certains pilotes graphiques, NVIDIA surtout) : lance `WEBKIT_DISABLE_DMABUF_RENDERER=1 npm run tauri dev` et signale-le dans tes retours.

Sous Ubuntu, les tests restent les mêmes, avec ces différences :

| Test (fiche Windows) | Sous Ubuntu |
| --- | --- |
| Ouvrir `vault.younji` dans Notepad++ | `xxd ~/.local/share/io.github.mlft9.younji/vault.younji \| less` ou `strings` sur le fichier : aucun nom d'hôte ni utilisateur |
| Clé de lab dans `~\.younji-lab` | Même chose dans `~/.younji-lab/` (`chmod 600` sur la clé privée) |
| Édition SFTP dans le Bloc-notes | S'ouvre dans l'éditeur graphique trouvé (`gnome-text-editor`, `gedit`, `kate`…) |
| Dossiers locaux refusés `C:\`, `~\.ssh` | `/`, ton dossier personnel `~` entier, `~/.ssh` : refusés |
| Mark-of-the-Web (« provient d'un autre ordinateur ») | 🔒 À la place : `ls -l` sur un fichier téléchargé affiche `-rw-------` (0600), même s'il était exécutable sur le serveur |
| Installeur Windows (étape 0) | Installer le `.deb` (`sudo apt install ./younji_*.deb`) ou lancer l'AppImage produits par le workflow **build** |

### A2. VM `younji-deb` sur Proxmox (obligatoire)
Tout est détaillé dans `docs/ENVIRONNEMENT_DE_TEST.md` §3. En résumé :
- [ ] Debian 12/13 sans interface, sur un **réseau isolé** (pont/VLAN dédié en NAT, jamais exposé à Internet).
- [ ] Comptes `testpwd` (mot de passe), `testkey` (clé seule), `test2fa` (mot de passe + TOTP), `testsftp` (SFTP seul, chroot).
- [ ] Bloc `Match …` ajouté à `sshd_config`, PAM google-authenticator pour `test2fa`.
- [ ] Clé de lab générée sur ton PC **hors du dossier du projet** : `ssh-keygen -t ed25519 -f ~/.younji-lab/younji_test_ed25519` (avec une passphrase), `.pub` copiée chez `testkey`.
- [ ] `vim`, `htop`, `tmux` installés (`sudo apt install -y vim htop tmux`).
- [ ] Données SFTP créées (`~/sftp-tests`, fichier de 1 Go, dossier `malveillants`) : commandes dans le §3.
- [ ] **Snapshot Proxmox `base-propre`** une fois tout prêt (indispensable : certains tests cassent la VM).

### A3. Optionnel pour l'instant
- **Tests d'intégration automatiques** contre ta VM : fichier `.env.test` à la racine (§6 de `ENVIRONNEMENT_DE_TEST.md`), puis `npm run test:integration`. Utile mais pas nécessaire pour la recette manuelle (la CI les lance déjà contre un conteneur).
- **`younji-desktop`** (Ubuntu Desktop) : pour vérifier la version Linux (`.deb` / AppImage produits par le workflow `build.yml`). À faire au moins une fois avant la publication.
- **`younji-win`** : utile surtout à partir de l'étape 8 (RDP) et pour tester l'installeur sur un Windows vierge.

---

## Partie B — Tests

### Étape 0 — Fondations
- [ ] ⭐ `npm run tauri dev` ouvre la fenêtre.
- [ ] Paramètres → langue : bascule FR ↔ EN **sans redémarrer**, tous les textes changent.
- [ ] Thème : suit le thème Windows par défaut, peut être forcé en clair ou sombre.
- [ ] Installeur : lancer à la main le workflow **build** sur GitHub (onglet Actions), télécharger l'installeur Windows, l'installer et le lancer.
- [ ] 🔒 Clic droit → « Inspecter » ou F12 : rien ne doit permettre d'ouvrir une autre page web dans la fenêtre.

### Étape 1 — Terminal SSH
- [ ] ⭐ Connexion rapide sur `testpwd` (mot de passe).
- [ ] ⭐🔒 **Première connexion** : l'empreinte `SHA256:…` s'affiche et doit être acceptée. Compare-la avec `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` sur la VM : identique.
- [ ] ⭐ Connexion sur `testkey` avec ta clé de lab et sa passphrase.
- [ ] Mauvais mot de passe / mauvaise passphrase : message d'erreur clair, pas de plantage.
- [ ] `vim`, `htop`, `tmux` s'affichent correctement, couleurs comprises. Redimensionner la fenêtre : `htop` s'adapte.
- [ ] Trois onglets ouverts en même temps ; renommer (double-clic ou F2), réordonner par glisser-déposer, fermer.
- [ ] Copier/coller : Ctrl+Maj+C / Ctrl+Maj+V, clic droit. Ctrl+C dans le terminal interrompt bien une commande (`sleep 100`).
- [ ] Recherche dans le terminal.
- [ ] `cat` d'un gros fichier (`seq 1 2000000`) ou `yes` : l'interface reste réactive, Ctrl+C l'arrête.
- [ ] Couper le réseau de la VM (débrancher la carte dans Proxmox) : l'onglet passe à « déconnecté » et propose de se reconnecter.
- [ ] 🔒 **Collage piégé** : copie un texte de plusieurs lignes (`ls` + retour à la ligne + `whoami`) et colle-le : une confirmation avec aperçu doit apparaître, rien ne s'exécute tout seul.
- [ ] 🔒 **Lien trompeur** : `printf '\e]8;;https://exemple.invalid\e\\https://github.com\e]8;;\e\\\n'` puis Ctrl+clic sur le lien : une confirmation montre la **vraie** adresse.
- [ ] ⭐🔒 **Changement de clé d'hôte** (après le snapshot !) : sur la VM, `sudo rm /etc/ssh/ssh_host_* && sudo ssh-keygen -A && sudo systemctl restart ssh`, puis reconnecte-toi : connexion **bloquée**, alerte claire, **aucun** bouton pour passer outre. Restaure ensuite le snapshot.

### Étape 2 — Coffre chiffré et carnet d'hôtes
- [ ] ⭐ Premier lancement (dossier de données vide) : création du coffre. Un mot de passe trop court ou trop faible est refusé, l'indicateur de robustesse réagit en direct.
- [ ] ⭐ Relancer l'application : elle démarre **verrouillée**.
- [ ] ⭐🔒 Mauvais mot de passe : refusé, sans dire pourquoi. Après 3 essais, chaque nouvel essai est ralenti.
- [ ] ⭐🔒 Ouvrir `vault.younji` dans Notepad++ : **aucun** nom d'hôte, adresse ou nom d'utilisateur lisible.
- [ ] 🔒 Modifier un seul caractère du fichier (copie de sauvegarde avant !) : le coffre refuse de s'ouvrir, avec le même message qu'un mauvais mot de passe. Remets le fichier d'origine.
- [ ] Verrouillage manuel (bouton ou Ctrl+Maj+L) : les sessions SSH sont fermées et le contenu des terminaux effacé.
- [ ] Verrouillage automatique : régler 1 minute, ne plus toucher à rien : l'application se verrouille.
- [ ] Changement du mot de passe maître : le nouveau fonctionne, l'ancien est refusé.
- [ ] ⭐ Carnet : créer une identité (utilisateur + mot de passe), un hôte qui l'utilise, un groupe, des étiquettes. Recherche instantanée.
- [ ] ⭐ Double-clic sur l'hôte : connexion **sans rien redemander**.
- [ ] Section Clés : importer ta clé de lab (passphrase demandée une seule fois), l'utiliser dans une identité pour `testkey`, copier la clé publique.
- [ ] 🔒 Supprimer une clé ou une identité utilisée par un hôte : refusé.
- [ ] Clés d'hôte connues : la liste montre la VM ; en supprimer une puis se reconnecter redemande l'acceptation de l'empreinte.

### Étape 3 — 2FA et générateur de clés
- [ ] ⭐ Connexion à `test2fa` : le mot de passe puis le code TOTP sont demandés dans une fenêtre dédiée.
- [ ] Hôte du carnet `test2fa` avec mot de passe enregistré : seul le code TOTP est demandé.
- [ ] Code faux : refus propre, pas de plantage.
- [ ] ⭐ Générer une clé Ed25519 dans Younji, la **déployer** sur `testpwd`, puis se connecter avec cette clé.
- [ ] Redéployer la même clé : sur la VM, `cat ~/.ssh/authorized_keys` ne contient **qu'une** ligne pour cette clé.
- [ ] 🔒 Sur la VM : `stat -c '%a %n' ~/.ssh ~/.ssh/authorized_keys` affiche `700` et `600`.
- [ ] Exporter la clé publique dans un fichier `.pub`.
- [ ] Générer aussi une ECDSA et une RSA 4096 (la RSA peut prendre quelques secondes, l'interface ne doit pas se figer).

### Étape 4 — SFTP (pas encore validée, c'est la priorité)
Connecté en terminal sur `testpwd` :
- [ ] ⭐ Ouvrir le panneau SFTP (bouton ou Ctrl+Maj+E).
- [ ] ⭐ Suivi du dossier : si le panneau ne suit pas, cliquer « Activer le suivi » ; la commande est **insérée sans être exécutée**, valide-la avec Entrée. Ensuite `cd /tmp` puis `cd ~/sftp-tests` : le panneau suit.
- [ ] ⭐ Ouvrir l'onglet double panneau (local à gauche, serveur à droite).
- [ ] ⭐ Télécharger `~/sftp-tests` vers un dossier local : accents, espaces, dossier vide et fichiers vides sont bien là.
- [ ] Renvoyer ce dossier sur le serveur sous un autre nom ; comparer sur la VM (`diff -r`).
- [ ] ⭐ Fichier de 1 Go : la progression avance, l'interface reste fluide (tape dans le terminal pendant le transfert), **Annuler** fonctionne et ne laisse pas de fichier partiel.
- [ ] Conflit : retélécharger un fichier existant : Younji demande « écraser / ignorer / garder les deux », avec « appliquer aux suivants ».
- [ ] Créer un dossier, renommer, changer les droits (chmod), supprimer (confirmation demandée).
- [ ] Éditer un fichier texte distant : il s'ouvre dans le Bloc-notes ; enregistrer → le fichier est mis à jour sur la VM (`cat`).
- [ ] Modifier ce même fichier sur la VM pendant l'édition, puis réenregistrer dans le Bloc-notes : Younji signale le conflit au lieu d'écraser.
- [ ] ⭐🔒 **Noms malveillants** : télécharger `~/sftp-tests/malveillants`. Attendu : seul `normal.txt` est copié, les autres sont comptés en « noms refusés », les liens en « liens non copiés », et **rien** n'apparaît en dehors du dossier local choisi. Les noms bizarres sont signalés dans la liste.
- [ ] 🔒 `lien-vers-passwd` : n'est pas suivi au téléchargement d'un dossier.
- [ ] 🔒 Choisir comme dossier local `C:\`, ton dossier utilisateur entier ou `~\.ssh` : refusé.
- [ ] 🔒 Fichier téléchargé : Propriétés Windows → mention « Ce fichier provient d'un autre ordinateur » (Mark-of-the-Web).
- [ ] Compte `testsftp` (SFTP seul, chroot) : la connexion terminal échoue ou se ferme proprement, pas de plantage. *(Limite attendue : le SFTP s'appuie sur une session de terminal ouverte, voir ADR-034.)*
- [ ] Fermer l'onglet terminal : les onglets SFTP liés se ferment. Verrouiller pendant un transfert : le transfert s'arrête.

---

## Partie C — Remonter tes retours
Pour chaque problème : l'étape, le test, ce que tu as fait, ce que tu attendais, ce qui s'est passé (capture si possible).
Les journaux de diagnostic sont dans `%LOCALAPPDATA%\io.github.mlft9.younji\logs` (ils ne contiennent aucun secret). Lance ensuite `/retour` avec ta liste.
