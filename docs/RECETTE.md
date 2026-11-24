# Recette manuelle — étapes 0 à 6

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
- [ ] **Firefox** et **Python 3** (`python --version`), pour les tests de tunnels (étape 6).

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
| `Get-NetTCPConnection -LocalPort 18080 -State Listen` (tunnels) | `ss -ltn \| grep 18080` : aucune ligne = port libéré |
| Serveur web de test dans `$env:TEMP\younji-r` (tunnels) | `mkdir -p /tmp/younji-r && echo "Bonjour depuis le PC" > /tmp/younji-r/index.html && cd /tmp/younji-r && python3 -m http.server 9000 --bind 127.0.0.1` |
| Boîte de confirmation native Windows (tunnel exposé) | Boîte de dialogue GTK, même contenu |
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

> **Préparation sur `younji-deb`** (snapshot `base-propre`) :
> - Serveur web de test, à laisser tourner : `mkdir -p ~/www && echo "Bonjour depuis younji-deb" > ~/www/index.html && cd ~/www && python3 -m http.server 8080 --bind 127.0.0.1`
> - Nom résolu **uniquement** par la VM (prouve que `-D` fait résoudre les noms par le serveur) : `echo "127.0.0.1 younji-socks.test" | sudo tee -a /etc/hosts`
> - `sudo sshd -T | grep -Ei 'allowtcpforwarding|gatewayports'` : attendu `allowtcpforwarding yes` et `gatewayports no`.
> - Sauvegarde des clés d'hôte avant le test de clé modifiée : `sudo cp -a /etc/ssh/ssh_host_* /root/`
>
> **Préparation sur le PC** (PowerShell) :
> - Serveur web local, à laisser tourner : `mkdir $env:TEMP\younji-r ; "Bonjour depuis le PC" | Out-File -Encoding ascii $env:TEMP\younji-r\index.html ; cd $env:TEMP\younji-r ; python -m http.server 9000 --bind 127.0.0.1`
> - (Facultatif) `npm run test:integration` avec les variables `YOUNJI_TEST_*`.
> - `npm run tauri dev`, déverrouiller ; hôte `younji-deb` du carnet avec `testpwd` (identité à mot de passe).

**Redirection locale `-L` et gestionnaire (TUN-01, TUN-02)**
- [ ] (Facultatif) `npm run test:integration` : les 11 tests OpenSSH passent, dont 2 de tunnels.
- [ ] ⭐ Tunnels → « Nouveau » : portée `127.0.0.1` par défaut, démarrage automatique décoché.
- [ ] ⭐ Tunnel « Web », `-L`, port `18080` → `127.0.0.1:8080`. Démarrer : clé d'hôte demandée si inconnue, puis état actif sur `127.0.0.1:18080`.
- [ ] ⭐ Firefox, `http://127.0.0.1:18080/` → « Bonjour depuis younji-deb ».
- [ ] Rafraîchir 3 fois : le total augmente, « en cours » redescend à 0.
- [ ] 2e tunnel `-L` `18022` → `127.0.0.1:22` ; deux `ssh -p 18022 testpwd@127.0.0.1` ouverts → « 2 en cours », puis 0 après `exit`.
- [ ] ⭐🔒 Arrêter « Web » ; `Get-NetTCPConnection -LocalPort 18080 -State Listen` → rien (port libéré).

**Proxy SOCKS5 `-D`**
- [ ] ⭐ Tunnel « Socks », `-D` `1080` en `127.0.0.1` ; Firefox → Paramètres réseau → proxy manuel SOCKS v5 `127.0.0.1:1080`, cocher « Utiliser un DNS distant ».
- [ ] ⭐ Firefox, `http://younji-socks.test:8080/` → page de la VM ; le compteur bouge. Remettre ensuite Firefox sans proxy.

**Redirection distante `-R`**
- [ ] ⭐ Tunnel « Retour », `-R` port serveur `9000` → `127.0.0.1:9000` côté PC. Démarrer.
- [ ] ⭐ Sur la VM : `curl http://127.0.0.1:9000/` → « Bonjour depuis le PC ».
- [ ] 🔒 Arrêter « Retour » ; sur la VM, `ss -ltn | grep 9000` → rien.

**Modification et suppression**
- [ ] Modifier un tunnel démarré : bouton désactivé (« arrêtez le tunnel pour le modifier »).
- [ ] Supprimer un tunnel démarré : il est arrêté (port libéré) puis supprimé.

**Exposition sur `0.0.0.0` (TUN-03)**
- [ ] ⭐🔒 Tunnel « Expo », `-L` `18082` → `127.0.0.1:8080` en `0.0.0.0` : avertissement affiché, enregistrement impossible sans cocher la confirmation.
- [ ] 🔒 Une fois enregistré : badge permanent « Exposé au réseau (0.0.0.0) » et bordure orange dans la liste.
- [ ] ⭐🔒 Démarrer « Expo » → boîte de dialogue **native** Windows (nom, `-L 0.0.0.0:18082:127.0.0.1:8080`, hôte). Annuler → erreur « démarrage annulé », rien n'écoute sur 18082.
- [ ] 🔒 Démarrer à nouveau et confirmer ; depuis la VM, `curl http://<IP du PC>:18082/` (autoriser le pare-feu Windows) → page de la VM.
- [ ] 🔒 Deux tunnels exposés démarrés très vite l'un après l'autre : une seule boîte native ; le second affiche « confirmation déjà ouverte ».

**Démarrage automatique et verrouillage**
- [ ] Cocher le démarrage automatique sur « Web » et « Expo » ; relancer Younji et déverrouiller : « Web » démarre seul, « Expo » ouvre d'abord la boîte native. Rien ne démarre avant le déverrouillage. *(Avec plusieurs tunnels exposés en démarrage automatique, seul le premier obtient sa boîte ; les autres affichent « confirmation déjà ouverte ».)*
- [ ] ⭐🔒 Tunnels démarrés → Verrouiller ; `Get-NetTCPConnection -LocalPort 18080,1080 -State Listen` → rien. Au déverrouillage, seuls les tunnels en démarrage automatique repartent, une seule fois.
- [ ] 🔒 Même test avec le verrouillage automatique (délai d'1 min dans les Paramètres).

**Erreurs et sécurité de la connexion**
- [ ] Sur le PC : `python -m http.server 18083 --bind 127.0.0.1` ; tunnel `-L` sur `18083` → « port déjà utilisé ».
- [ ] Serveur web de la VM actif sur 8080 ; `-R` sur le port serveur `8080` → « le serveur refuse d'écouter ».
- [ ] Formulaire : port `0`, `70000`, destination vide → refusés avant tout appel au cœur, message traduit.
- [ ] Mauvais mot de passe dans l'identité → échec d'authentification, aucun port ouvert.
- [ ] 🔒 Supprimer la clé d'hôte connue, démarrer, refuser la clé → arrêt, aucun port ouvert.
- [ ] 🔒 Régénérer les clés d'hôte de la VM (`sudo rm /etc/ssh/ssh_host_*; sudo ssh-keygen -A; sudo systemctl restart ssh`) ; démarrer « Web » → bloqué « clé modifiée », aucune option pour passer outre. Restaurer : `sudo cp -a /root/ssh_host_* /etc/ssh/ && sudo systemctl restart ssh`.
- [ ] 🔒 Hôte `test2fa` sans identité, tunnel `-L` `18084` → `127.0.0.1:8080` : questions mot de passe et code TOTP affichées même hors de la section Tunnels ; puis tunnel actif et page joignable.
- [ ] Code TOTP faux (attendre le code suivant) → refus, aucun port ouvert.
- [ ] Pendant la 2FA, « Arrêter » ou « Annuler » → arrêt propre, aucune boîte restée ouverte.
- [ ] Débrancher la carte réseau de la VM (Proxmox) avec « Web » actif → après le keepalive (~45 s), « Arrêté » avec une erreur, port libéré.
- [ ] Supprimer l'hôte `younji-deb` → ses tunnels disparaissent.
- [ ] Langue EN, refaire le démarrage de « Expo » et le test « port déjà utilisé » → textes en anglais, boîte native comprise.

---

## Partie C — Remonter tes retours
Pour chaque problème : l'étape, le test, ce que tu as fait, ce que tu attendais, ce qui s'est passé (capture si possible).
Les journaux de diagnostic sont dans `%LOCALAPPDATA%\io.github.mlft9.younji\logs` (ils ne contiennent aucun secret). Lance ensuite `/retour` avec ta liste.
