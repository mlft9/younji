# Recette manuelle — étapes 0 à 8

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
- **`younji-win`** : nécessaire pour l'étape 8 (RDP, préparation dans `ENVIRONNEMENT_DE_TEST.md` §4) et utile pour tester l'installeur sur un Windows vierge.

---

## Partie B — Tests

### Étape 0 — Fondations
- [ ] ⭐ `npm run tauri dev` ouvre la fenêtre.
- [ ] Paramètres → langue : bascule FR ↔ EN **sans redémarrer**, tous les textes changent.
- [ ] Thème : suit le thème Windows par défaut, peut être forcé en clair ou sombre.
- [ ] Installeur : lancer à la main le workflow **build** sur GitHub (onglet Actions), télécharger l'installeur Windows, l'installer et le lancer.
- [ ] 🔒 Inspecteur web : en mode développement (`npm run tauri dev`), clic droit **dans un champ de saisie** → « Inspecter » est **normal** (outil de développement). Dans la version **installée** (`.deb`, AppImage ou installeur Windows produits par `build.yml`), le clic droit ne doit **pas** proposer « Inspecter ». Depuis l'étape 5, le clic droit ailleurs dans l'application n'ouvre plus aucun menu natif (ni « Recharger »).

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

> **Si la 2FA échoue**, vérifie d'abord le lab (pièges fréquents, ce ne sont pas des bugs de Younji) :
> - **Un code ne sert qu'une fois** (`-d`) : après un essai avec `ssh` dans un terminal, attends le code suivant (30 s) avant d'essayer dans Younji.
> - **3 connexions au plus par 30 secondes** (`-r 3 -R 30`) : au-delà, PAM refuse **sans même demander le code**. Attends 30 s.
> - **Horloge** : sur la VM, `timedatectl` doit afficher `System clock synchronized: yes` et l'heure juste ; un décalage de plus de ~1 min invalide les codes.
> - **Mode de connexion** : dans la connexion rapide, « mot de passe » (le mot de passe est envoyé automatiquement, seul le code est demandé) et « Questions du serveur (2FA) » (les deux sont demandés) fonctionnent tous les deux.
> - **Cause exacte** : sur la VM, `sudo journalctl -u ssh -n 30` montre pourquoi PAM a refusé.
>
> Si ça échoue encore, note le message affiché par Younji et ces lignes de journal (elles ne contiennent pas de secret) dans ton `/retour`.
- [ ] ⭐ Connexion à `test2fa` : le mot de passe puis le code TOTP sont demandés dans une fenêtre dédiée.
- [ ] Hôte du carnet `test2fa` avec mot de passe enregistré : seul le code TOTP est demandé.
- [ ] Code faux : refus propre, pas de plantage.
- [ ] ⭐ Générer une clé Ed25519 dans Younji, la **déployer** sur `testpwd`, puis se connecter avec cette clé.
- [ ] Redéployer la même clé : sur la VM, `cat ~/.ssh/authorized_keys` ne contient **qu'une** ligne pour cette clé.
- [ ] 🔒 Sur la VM : `stat -c '%a %n' ~/.ssh ~/.ssh/authorized_keys` affiche `700` et `600`.
- [ ] Exporter la clé publique dans un fichier `.pub`.
- [ ] Générer aussi une ECDSA et une RSA 4096 (la RSA peut prendre quelques secondes, l'interface ne doit pas se figer).

### Étape 4 — SFTP
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

### Étape 5 — Split, snippets, diffusion multi-sessions (validée le 24/11/2026)

> **Préparation sur `younji-deb`** (snapshot `base-propre`) : `sudo systemctl is-active ssh cron` (les deux actifs), puis `sudo rm -f /tmp/diff-*`.
> **Préparation dans Younji** : trois hôtes du carnet vers `younji-deb` : `testpwd` (mot de passe enregistré), `testkey` (clé du lab), ton compte. Un nouveau panneau ouvert depuis un hôte du carnet se connecte sans rien redemander ; depuis une connexion rapide, le mot de passe est redemandé (normal, ADR-035).

**Écran partagé (SSH-07)**
- [ ] Ouvrir `testpwd` depuis le carnet, puis Ctrl+Maj+D : un deuxième panneau s'ouvre à droite et se connecte seul. Un seul onglet dans la barre.
- [ ] ⭐ Ctrl+Maj+O dans chaque panneau jusqu'à une grille 2×2 de 4 terminaux connectés. `tty` dans chacun : quatre valeurs différentes.
- [ ] Clic dans un panneau : il prend le focus (bordure) et la frappe y arrive. *(Ctrl+Maj+flèches : facultatif, signalé non fonctionnel sous Ubuntu.)*
- [ ] ⭐ Taper `echo éèàçù` au clavier puis Entrée : le texte s'affiche tel quel, y compris sur une Debian sans `LANG`.
  - Si `(arg: 9)` ou rien ne s'affiche : vérifier d'abord la compilation testée (Paramètres → Version, le commit doit correspondre au dernier commit de la branche), puis dans la session : `locale` (attendu : `LC_CTYPE="C.UTF-8"` ou une autre locale UTF-8, même si `LANG=C`) et, sur le serveur, `sudo sshd -T | grep -i acceptenv` (attendu : `acceptenv LANG`).
- [ ] ⭐ Glisser un séparateur à la souris : les panneaux se redimensionnent, `htop` s'adapte.
- [ ] Tab jusqu'à un séparateur, puis flèches, Début, Fin : il bouge, sans réduire un panneau à zéro.
- [ ] Le nom du serveur s'affiche en tête de chaque panneau.
- [ ] Au-delà de 8 panneaux, plus rien ne s'ouvre, sans plantage.
- [ ] Ctrl+Maj+X ou ✕ ferme un panneau : son voisin prend la place. Fermer le panneau en haut à gauche : l'onglet et les autres panneaux restent.
- [ ] Ctrl+Maj+W ferme l'onglet entier ; sur la VM, `who` ne montre plus ces sessions.
- [ ] Coupure réseau (carte de la VM débranchée dans Proxmox) : chaque panneau passe à « déconnecté » et propose de se reconnecter.
- [ ] Ctrl+Maj+E dans un panneau : le panneau SFTP s'ouvre pour ce panneau.

**Diffusion de la saisie (SSH-08)** — ouvrir trois sessions (`testpwd`, `testkey`, ton compte) plus une 4e sur `testpwd`, qui ne sera pas cochée.
- [ ] ⭐🔒 Ctrl+Maj+I (ou icône antenne) : le dialogue liste les sessions par nom et adresse, seule la session active est cochée ; avec une seule case cochée, l'activation est refusée.
- [ ] ⭐ Cocher les 3 et confirmer : bandeau rouge en haut, bordure rouge épaisse et en-tête « DIFFUSION » sur chaque cible, pastille sur les onglets.
- [ ] ⭐ Dans une cible : `touch /tmp/diff-$(whoami)-$$` + Entrée. Sur la VM, `ls /tmp/diff-*` montre 3 fichiers ; la 4e session n'a rien reçu.
- [ ] Taper dans la 4e session : la frappe ne va que là.
- [ ] Ctrl+C, flèches, Tab, `vim` puis `:q` : même réaction dans chaque cible. Longue ligne tapée vite : aucun caractère perdu.
- [ ] Aller sur l'Accueil ou les Paramètres : le bandeau rouge reste visible.
- [ ] ⭐ « Arrêter la diffusion » : bandeau et bordures disparaissent en un clic.
- [ ] Réactiver, puis Ctrl+Maj+I : arrêt immédiat, sans dialogue. *Si les outils de développement s'ouvrent (mode `tauri dev`), le noter.*
- [ ] 🔒 Collage pendant la diffusion (une seule ligne, `whoami`) : une confirmation apparaît toujours et liste les serveurs.
- [ ] 🔒 Fermer une cible sur 3 : la diffusion continue vers 2 ; en fermer une autre : elle s'arrête.
- [ ] 🔒 Déconnecter une cible : elle sort de la diffusion et n'y revient pas après reconnexion.
- [ ] 🔒 Ctrl+Maj+L pendant la diffusion : après déverrouillage, diffusion inactive, aucun dialogue resté ouvert.
- [ ] 🔒 (Optionnel) Dans `testpwd`, avant d'activer : `sleep 8; printf '\e[6n'` + Entrée, puis activer la diffusion vers les 3 dans les 8 s sans toucher au clavier. Des caractères parasites peuvent apparaître au plus dans `testpwd`, rien dans les deux autres.
- [ ] Clavier AZERTY : en diffusion, `@ # { } | \ ~` (AltGr) et `é è à ç` arrivent identiques dans les 3 cibles.

**Snippets (SNP-01 à SNP-03)**
- [ ] ⭐ Section Snippets, « Nouveau snippet » : `Statut service`, `systemctl status {{service}} --no-pager`. L'interface affiche « Variables demandées à l'envoi : service ».
- [ ] Modifier, rechercher, supprimer (confirmation demandée).
- [ ] Verrouiller puis déverrouiller : le snippet est toujours là. 🔒 Ni nom ni commande lisibles dans `vault.younji` (Notepad++).
- [ ] ⭐ Dans un panneau, Ctrl+Maj+S, choisir le snippet : `service` est demandé avant l'envoi ; avec `ssh`, l'aperçu montre `systemctl status ssh --no-pager`.
- [ ] « Insérer » : la commande s'affiche sans être exécutée. « Exécuter » : elle est exécutée.
- [ ] 🔒 Valeur avec un retour à la ligne (coller `ssh` + Entrée + `whoami`) : refusée, envoi bloqué.
- [ ] SNP-03 : `service` = `cron` vers 2 sessions cochées sur 3 : seules ces deux l'exécutent ; le bouton indique « Exécuter dans 2 sessions ».
- [ ] Snippet sur deux lignes (`whoami` / `hostname`) : avertissement « plusieurs lignes » ; avec « Insérer », bash n'exécute rien avant Entrée.
- [ ] Langue EN : sections Snippets, diffusion et infobulles des panneaux en anglais (Ctrl+Shift+…).

### Étape 6 — Tunnels SSH
Retirée du périmètre (ADR-038) : rien à tester.

### Étape 7 — Terminal local

> **Préparation sous Windows** : PowerShell 7 (`winget install Microsoft.PowerShell`), une distribution WSL (`wsl -l -q`, sinon `wsl --install -d Ubuntu`) avec `vim` et `htop`. Gestionnaire des tâches ouvert, onglet « Détails ».
> **Préparation sur la VM Linux du lab (avec bureau)** : `sudo apt install -y zsh fish vim htop` ; noter `cat /etc/shells` et `echo $SHELL` ; garder un second terminal système ouvert pour `ps`.

**Windows (LOC-01)**
- [ ] ⭐ « Terminal local » liste Windows PowerShell, PowerShell 7, CMD et chaque distribution de `wsl -l -q` (sans `docker-desktop*`).
- [ ] ⭐ Windows PowerShell s'ouvre dans `C:\Users\<toi>` ; `Get-Date` fonctionne.
- [ ] PowerShell 7 : `$PSVersionTable.PSVersion` affiche 7.x.
- [ ] CMD : `ver`, `dir`, `echo éàç` corrects.
- [ ] ⭐ La distribution WSL s'ouvre dans `~` (`pwd` = `/home/<user>`).
- [ ] Couleurs : `Get-ChildItem` coloré (pwsh) ; dans WSL, `ls --color` et `printf '\e[38;2;255;100;0mTRUECOLOR\e[0m\n'` (orange).
- [ ] ⭐ Redimensionnement : `tput cols; tput lines` (WSL) et `$Host.UI.RawUI.WindowSize` (pwsh) suivent la fenêtre.
- [ ] ⭐ Plein écran : `vim` et `htop` dans WSL (redimensionner pendant `htop`), terminal intact après la sortie.
- [ ] Découpe d'un onglet local : le nouveau panneau ouvre le même profil ; ni SFTP, ni snippet, ni diffusion proposés.
- [ ] 🔒 Fermer un onglet pwsh (noter `$PID` avant) : le processus disparaît, sans `OpenConsole.exe` ni `conhost.exe` en trop.
- [ ] ⭐🔒 Verrouillage avec PowerShell, CMD et WSL ouverts : onglets fermés, contenu effacé, processus absents du Gestionnaire des tâches.
- [ ] 🔒 Verrouillage pour inactivité : même résultat.
- [ ] `exit` : l'onglet indique que le shell s'est terminé, et « Relancer » fonctionne.
- [ ] ⭐ Forte sortie puis `exit` : `Get-ChildItem -Recurse C:\ -ErrorAction SilentlyContinue`, interface réactive, Ctrl+C, `exit` : état « terminé » en 3 s environ, sans figer l'application.
- [ ] ⭐ Même commande, puis fermer l'onglet (ou verrouiller) sans attendre : fermeture immédiate, processus tués ; un nouveau terminal local fonctionne ensuite.
- [ ] Sortie massive dans WSL (`yes | head -c 200M`, `cat /dev/urandom | base64` puis Ctrl+C) : application fluide, mémoire stable.
- [ ] 🔒 Aucun champ ne permet de saisir un programme ou des arguments ; une distribution désinstallée disparaît après « Actualiser ».
- [ ] Coffre verrouillé : la section n'est plus accessible ; après déverrouillage, la liste est détectée à nouveau.
- [ ] Interface en anglais : libellés et erreurs traduits.

**Linux (LOC-02), sur la VM du lab**
- [ ] ⭐ bash, zsh et fish proposés (un par nom, ceux de `/etc/shells`) ; le shell de `$SHELL` en premier, marqué « par défaut ».
- [ ] `echo $TERM $COLORTERM` affiche `xterm-256color truecolor` ; l'invite est dans `~`.
- [ ] zsh et fish fonctionnent (`echo $0`).
- [ ] Couleurs 256 et truecolor (`printf` comme sous Windows, palette `for i in {0..255}; …`).
- [ ] ⭐ `watch -n1 'tput cols; tput lines'` suit le redimensionnement.
- [ ] ⭐ `vim`, `htop`, `less /etc/services` corrects.
- [ ] `yes`, `cat /dev/urandom | base64` : interface fluide, Ctrl+C immédiat, mémoire stable.
- [ ] 🔒 `echo $$`, fermer l'onglet : `ps -p <PID>` ne renvoie rien.
- [ ] ⭐🔒 `bash`, puis `trap '' HUP; echo $$`, fermer l'onglet : `ps -p <PID>` ne renvoie rien, en moins d'une seconde.
- [ ] ⭐🔒 Deux terminaux locaux ouverts, verrouillage : leurs PID ont disparu (`ps -ef --forest`).
- [ ] 🔒 Rechargement de l'interface (F5 en mode dev, hors du terminal) : coffre verrouillé, shell tué.
- [ ] 🔒 Relancer Younji avec `SHELL=/usr/bin/python3` : python n'apparaît pas dans la liste.
- [ ] 🔒 `echo /usr/bin/python3 | sudo tee -a /etc/shells`, puis « Actualiser » : python n'est pas proposé. Remettre en état : `sudo sed -i '\#^/usr/bin/python3$#d' /etc/shells`.
- [ ] `exit` puis « Relancer » : même shell.
- [ ] 🔒 Journaux de Younji : aucun contenu de terminal ni commande (`grep -ri 'TRUECOLOR\|trap'` dans le dossier `logs/`).

**Limite connue (simple observation)**
- [ ] `sleep 300 & disown` puis `exit` : l'onglet peut rester ouvert jusqu'à la fin de `sleep` (le processus restant garde le terminal ouvert). Sous Windows, `Start-Process notepad` puis `exit` : le Bloc-notes reste ouvert, c'est attendu.

### Étape 8 — RDP v1 (client du système)

> **Préparation de la VM `younji-win`** (PowerShell administrateur, puis snapshot `rdp-pret`) : commandes de `ENVIRONNEMENT_DE_TEST.md` §4. Choisis pour `testrdp` un mot de passe propre au lab contenant un **marqueur unique** (par exemple `Rdp7319`), pour le chercher ensuite avec `grep` ; ne le note pas dans le dépôt. Relève le nom renvoyé par `hostname` (`<NOMPC>`).
> **Préparation de `younji-desktop`** : `sudo apt install -y freerdp3-x11 && xfreerdp3 --version` (3.x attendu), puis `rm -rf ~/.config/freerdp`.
> **Dans Younji** : une identité `rdp-lab` (utilisateur `testrdp`, mot de passe ci-dessus).
> **PC Windows** : `cmdkey /list | findstr TERMSRV` ne doit rien afficher pour l'IP de la VM.

**Carnet (RDP-01)**
- [ ] ⭐ R1 — Nouvel hôte, protocole RDP : le port passe à 3389, les champs Domaine, Plein écran, Largeur, Hauteur apparaissent.
- [ ] R2 — Retour en SSH : port 22, réglages RDP masqués.
- [ ] ⭐ R3 — « Win-plein » (IP de la VM, identité `rdp-lab`, domaine vide, plein écran) : icône écran et badge « RDP ».
- [ ] R4 — « Win-1280 » (domaine `<NOMPC>`, 1280 × 800) : valeurs conservées à la modification.
- [ ] 🔒 R5 — Domaine `/p:x` ou `LAB x`, largeur 100 ou 9000 : enregistrement refusé, message traduit.

**Windows : mstsc (RDP-02)**
- [ ] ⭐ R6 — Double-clic sur « Win-plein » : mstsc se connecte en `testrdp` sans demander le mot de passe.
- [ ] R7 — Premier lancement : laisser l'avertissement de certificat ouvert plus de 60 s, puis accepter ; noter si mstsc redemande le mot de passe (possible : l'identifiant temporaire a disparu).
- [ ] ⭐🔒 R8 — `wmic process where name='mstsc.exe' get commandline` (ou Process Explorer) : seul le chemin `…\rdp\younji-XXXX.rdp`, aucun mot de passe.
- [ ] 🔒 R9 — Le `.rdp` de `%LOCALAPPDATA%\io.github.mlft9.younji\rdp\` (premières secondes) : `full address`, `username`, redirections, aucune ligne `password`.
- [ ] ⭐🔒 R10 — Dans les 60 s : `cmdkey /list` montre `TERMSRV/<ip>` ; 60 s plus tard (session ouverte) : disparu, dossier `rdp\` vide.
- [ ] 🔒 R11 — Fermer mstsc en moins de 60 s : `TERMSRV/<ip>` et `.rdp` absents aussitôt.
- [ ] 🔒 R12 — Verrouiller le coffre en moins de 60 s : mstsc se ferme, rien ne reste.
- [ ] 🔒 R13 — Fermer Younji en moins de 60 s : mstsc se ferme, `TERMSRV/<ip>` absent. Variante : `taskkill /F /IM younji.exe`, l'identifiant reste, puis rouvrir Younji : il a disparu.
- [ ] ⭐🔒 R14 — `cmdkey /add:TERMSRV/<ip> /user:testrdp /pass` (saisie masquée), lancer « Win-plein » : message « identifiant déjà enregistré dans Windows… » ; après fermeture et 60 s, l'identifiant est toujours là, inchangé. Le supprimer ensuite (`cmdkey /delete:TERMSRV/<ip>`).
- [ ] R15 — Identité sans mot de passe : mstsc le demande lui-même.
- [ ] R16 — « Win-1280 » : fenêtre 1280 × 800, connexion réussie.

**Linux : xfreerdp3 (RDP-02), sur `younji-desktop`**
- [ ] ⭐ R17 — « Win-plein » : connexion en plein écran sans aucune question ; certificat mémorisé dans `~/.config/freerdp/`.
- [ ] ⭐🔒 R18 — Pendant la session, après `M=<ton marqueur>` : `ps aux | grep -i [f]reerdp`, `pid=$(pgrep -n xfreerdp3); tr '\0' ' ' </proc/$pid/cmdline`, `tr '\0' '\n' </proc/$pid/environ | grep -c "$M"` → arguments `/v:… /u:testrdp /f /cert:tofu,deny +clipboard /from-stdin:force`, aucun `/p:`, compteur à 0.
- [ ] 🔒 R19 — `grep -rl "$M" ~/.local/share/io.github.mlft9.younji ~/.config/freerdp` : aucun résultat.
- [ ] R20 — Utilisateur `testrdp`, puis `testrdp@<NOMPC>`, puis `<NOMPC>\testrdp` : noter le résultat de chaque forme (un échec doit être une erreur d'authentification propre, sans question ni mot de passe affiché).
- [ ] ⭐🔒 R21 — Changer le certificat de la VM (`Get-ChildItem 'Cert:\LocalMachine\Remote Desktop' | Remove-Item; Restart-Service TermService -Force`), relancer : xfreerdp3 refuse **sans question**. Restaurer le snapshot `rdp-pret`.
- [ ] R22 — `sudo apt remove -y freerdp3-x11` : message « Aucun client RDP trouvé… freerdp3-x11 ». Réinstaller.
- [ ] R23 — Identité sans mot de passe : message « l'identité doit contenir un mot de passe », aucun xfreerdp3 lancé.
- [ ] 🔒 R24 — Verrouiller le coffre pendant une session : xfreerdp3 se ferme (`pgrep xfreerdp3` vide).
- [ ] R25 — Verrouillage automatique à 1 min, rester dans la session RDP : coupée au bout d'1 min environ. Noter si c'est gênant.

**Commun**
- [ ] 🔒 R26 — Un hôte RDP n'ouvre jamais d'onglet terminal et n'apparaît pas dans « Déployer la clé ».
- [ ] R27 — VM éteinte : le client affiche son erreur, Younji reste réactif ; sous Windows, `cmdkey /list` propre au bout de 60 s au plus.
- [ ] R28 — Deux clics rapides sur le même hôte (Windows) : « Une ouverture RDP vers ce serveur est déjà en cours ».
- [ ] R29 — Interface en anglais : formulaire, badge, bouton, erreurs traduits.
- [ ] 🔒 R30 — Coffre créé avant l'étape 8 : les hôtes existants restent SSH et fonctionnent.

---

## Partie C — Remonter tes retours
Pour chaque problème : l'étape, le test, ce que tu as fait, ce que tu attendais, ce qui s'est passé (capture si possible).
Les journaux de diagnostic sont dans `%LOCALAPPDATA%\io.github.mlft9.younji\logs` (ils ne contiennent aucun secret). Lance ensuite `/retour` avec ta liste.
