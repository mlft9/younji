# Environnement de test (lab Proxmox)

Ce lab sert aux tests manuels de chaque étape et aux tests d'intégration lancés en local.
En CI, un conteneur OpenSSH (`tests/docker/sshd/`, depuis l'étape 1) remplace la VM Linux ; il peut aussi servir en local (§6).

## 1. Règles du lab
- **Réseau isolé** : un pont ou un VLAN dédié (ex. `vmbr1`) en NAT, **jamais exposé à Internet**. Les comptes de test ont volontairement des configurations variées, parfois faibles.
- **Identifiants uniquement pour le lab**, jamais réutilisés ailleurs.
- **Snapshot Proxmox** de chaque VM une fois préparée (`base-propre`). Certains tests sont destructifs, comme la régénération des clés d'hôte.
- Les adresses et identifiants de test vont dans `.env.test` (ignoré par git, et que Claude Code n'a pas le droit de lire). Les adresses seules (sans mot de passe) peuvent aller dans `CLAUDE.local.md`.

## 2. VM à préparer

| VM | Système | Sert à | Dès l'étape |
|---|---|---|---|
| `younji-deb` | Debian 12 ou 13, sans interface graphique | Serveur SSH/SFTP principal | 1 |
| `younji-win` | Windows Server (évaluation 180 j) ou Windows 11 Entreprise (évaluation 90 j) | Cible RDP, serveur OpenSSH Windows, test d'installation sur un Windows vierge | 0 (installeur), 8 (RDP) |
| `younji-desktop` | Ubuntu Desktop 24.04 | **Tester la version Linux de Younji** (.deb, AppImage) et `xfreerdp3` | 0 |

> Ton PC est sous Windows : sans `younji-desktop`, la version Linux de l'application ne serait jamais testée en conditions réelles.
> Pour l'étape 11, prévois une **YubiKey** (ou une autre clé FIDO2), branchée sur ton PC, côté client.

## 3. Préparer `younji-deb`

```bash
sudo apt update && sudo apt install -y openssh-server libpam-google-authenticator python3

# Comptes de test
sudo useradd -m -s /bin/bash testpwd   && sudo passwd testpwd     # mot de passe
sudo useradd -m -s /bin/bash testkey                             # clé uniquement
sudo useradd -m -s /bin/bash test2fa   && sudo passwd test2fa     # mot de passe + TOTP
sudo groupadd sftponly
sudo useradd -m -g sftponly -s /usr/sbin/nologin testsftp && sudo passwd testsftp
```

Crée le fichier `/etc/ssh/sshd_config.d/younji-lab.conf` (et non à la fin de `sshd_config` : `sshd` garde la **première** valeur lue, et le fichier par défaut de Debian contient déjà `KbdInteractiveAuthentication no` ; les fichiers de `sshd_config.d/` sont lus en premier). Vérifie ensuite avec `sudo sshd -t`, puis `sudo sshd -T | grep -i kbdinteractive` (attendu : `yes`) :
```
KbdInteractiveAuthentication yes
UsePAM yes

Match User testkey
    AuthenticationMethods publickey

Match User test2fa
    AuthenticationMethods keyboard-interactive

Match Group sftponly
    ChrootDirectory /srv/sftp/%u
    ForceCommand internal-sftp
    AllowTcpForwarding no
```

```bash
# Clé de test pour testkey : génère une paire DÉDIÉE au lab sur ton PC,
# HORS du dossier du projet (sinon elle pourrait être commitée ou lue par Claude Code) :
#   mkdir ~/.younji-lab ; ssh-keygen -t ed25519 -f ~/.younji-lab/younji_test_ed25519 -C younji-lab
# puis copie la partie .pub sur la VM :
sudo install -d -m 700 -o testkey -g testkey /home/testkey/.ssh
sudo install -m 600 -o testkey -g testkey younji_test_ed25519.pub /home/testkey/.ssh/authorized_keys

# 2FA pour test2fa : se connecter en tant que test2fa, puis lancer :
google-authenticator -t -d -f -r 3 -R 30 -w 3   # scanner le QR code dans une appli TOTP
# -d : un code ne sert qu'une fois ; -r 3 -R 30 : 3 connexions au plus par 30 s (au-delà, refus sans
# demander le code). Pendant les tests, attendre le code suivant entre deux essais. Horloge : `timedatectl`.
# Dans /etc/pam.d/sshd, ajouter :  auth required pam_google_authenticator.so nullok
# (nullok : les comptes sans TOTP configuré, dont le tien, continuent de se connecter normalement)

# Chroot SFTP
sudo mkdir -p /srv/sftp/testsftp/depot
sudo chown root:root /srv/sftp/testsftp && sudo chmod 755 /srv/sftp/testsftp
sudo chown testsftp:sftponly /srv/sftp/testsftp/depot

sudo systemctl restart ssh
```

Données pour les tests SFTP (dans le dossier de `testpwd`) :
```bash
mkdir -p ~/sftp-tests/{vide,"dossier avec espaces",accentué-éàü}
fallocate -l 1G ~/sftp-tests/gros-1G.bin
touch ~/sftp-tests/{"fichier avec espaces.txt","éàü.txt",".cache-caché"}
ln -s /etc/passwd ~/sftp-tests/lien-vers-passwd
```

Noms malveillants (étape 4, critère « ne peut pas écrire hors du dossier local choisi »), dans un dossier à part. Ils se créent sous Linux, où seuls `/` et NUL sont interdits ; `printf` produit les caractères spéciaux :
```bash
mkdir -p ~/sftp-tests/malveillants && cd ~/sftp-tests/malveillants
touch "$(printf 'retour\nligne.txt')"          # retour à la ligne
touch "$(printf 'echap\033[31mrouge.txt')"     # séquence d'échappement (ESC)
touch "$(printf 'facture\342\200\256fdp.exe')" # U+202E : inverse le sens du texte affiché
touch CON nul.txt 'COM1.log'                    # noms réservés de Windows
touch '..\x' 'C:\evil.txt' 'fichier:flux' 'point.' 'espace '
echo normal > normal.txt                         # témoin : doit être copié
ln -s /etc lien-vers-etc                         # lien vers un dossier système
ln -s .. lien-vers-parent                        # boucle
cd -
```
Attendu au téléchargement du dossier `malveillants` : seul `normal.txt` est copié ; les autres noms sont comptés en « noms refusés », les liens en « liens non copiés », et rien n'apparaît en dehors du dossier local choisi. Dans la liste, ces noms sont signalés comme inhabituels.

**Tester le changement de clé d'hôte (HKY-02)** : après un snapshot, lance
`sudo rm /etc/ssh/ssh_host_* && sudo ssh-keygen -A && sudo systemctl restart ssh`, puis reconnecte-toi avec Younji : la connexion doit être refusée. Restaure ensuite le snapshot.

**Comparer une empreinte** : `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` sur la VM doit afficher la même empreinte `SHA256:…` que Younji.

## 4. Préparer `younji-win`
- Activer le Bureau à distance avec NLA. Créer un compte local `testrdp`.
- **Pour l'étape 8 (RDP)**, dans un PowerShell administrateur, puis snapshot `rdp-pret` :
  ```powershell
  # Bureau à distance activé, NLA exigé, pare-feu ouvert
  Set-ItemProperty 'HKLM:\System\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 0
  Set-ItemProperty 'HKLM:\System\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp' -Name UserAuthentication -Value 1
  Enable-NetFirewallRule -Name RemoteDesktop-UserMode-In-TCP
  # Compte de test, membre de « Utilisateurs du Bureau à distance » (SID indépendant de la langue)
  net user testrdp "<mot de passe du lab>" /add
  Add-LocalGroupMember -SID S-1-5-32-555 -Member testrdp
  hostname   # nom NetBIOS, à utiliser comme domaine dans les tests
  ```
  Mot de passe propre au lab, jamais réutilisé ailleurs ; le plan de test de l'étape propose d'y glisser un marqueur unique pour le chercher ensuite (`grep`) dans les processus et les fichiers.
- **Changer le certificat RDP** (test du refus d'un certificat modifié) : `Get-ChildItem 'Cert:\LocalMachine\Remote Desktop' | Remove-Item; Restart-Service TermService -Force`. Restaurer ensuite le snapshot `rdp-pret`.
- (Optionnel) Installer le serveur OpenSSH (Paramètres → Fonctionnalités facultatives) pour tester SSH vers Windows et PowerShell à distance.
- Snapshot `base-propre` **avant** d'y installer Younji, pour tester l'installeur sur un Windows vierge (SmartScreen, WebView2).

## 5. Préparer `younji-desktop`
- Ubuntu Desktop 24.04 avec **FreeRDP 3** : `sudo apt install -y freerdp3-x11`, puis `xfreerdp3 --version` (3.x attendu). Younji ne cherche que la commande `xfreerdp3` : FreeRDP 2 (`freerdp2-x11`, commande `xfreerdp`) n'est pas pris en charge (ADR-040).
- Avant une recette RDP, `rm -rf ~/.config/freerdp` efface les certificats mémorisés par FreeRDP (premier certificat accepté, `/cert:tofu,deny`).
- Sert à installer le `.deb` ou l'`AppImage` produits par `build.yml` et à refaire le plan de test de l'étape sous Linux.

## 6. Tests d'intégration en local
Crée `.env.test` à la racine du projet (ignoré par git) :
```
YOUNJI_TEST_SSH_HOST=192.168.x.x
YOUNJI_TEST_SSH_PORT=22
YOUNJI_TEST_SSH_USER=testpwd
YOUNJI_TEST_SSH_PASSWORD=...
YOUNJI_TEST_SSH_KEY_USER=testkey
YOUNJI_TEST_SSH_KEY_PATH=C:/Users/<toi>/.younji-lab/younji_test_ed25519
YOUNJI_TEST_SSH_KEY_PASSPHRASE=...
YOUNJI_TEST_SSH_EXTRA_KEYS=C:/Users/<toi>/.younji-lab/younji_test_ecdsa;C:/Users/<toi>/.younji-lab/younji_test_rsa
```

| Variable | Rôle |
|---|---|
| `YOUNJI_TEST_SSH_HOST`, `YOUNJI_TEST_SSH_PORT` | Serveur de test (port 22 par défaut) |
| `YOUNJI_TEST_SSH_USER`, `YOUNJI_TEST_SSH_PASSWORD` | Compte à mot de passe (`testpwd`) |
| `YOUNJI_TEST_SSH_KEY_USER`, `YOUNJI_TEST_SSH_KEY_PATH` | Compte à clé (`testkey`) et sa clé privée |
| `YOUNJI_TEST_SSH_KEY_PASSPHRASE` | Passphrase de cette clé (facultative) |
| `YOUNJI_TEST_SSH_EXTRA_KEYS` | Autres clés **non chiffrées** pour `testkey` (ECDSA, RSA…), séparées par `;` (facultatif) |
| `YOUNJI_TEST_REQUIRED` | `1` : une variable manquante fait échouer le test au lieu de l'ignorer (utilisé en CI) |

Puis lance `npm run test:integration` : le script charge `.env.test` automatiquement s'il existe (sans afficher les valeurs), puis lance les tests marqués `#[ignore]`. Chaque test part d'un `known_hosts` vierge, dans un dossier temporaire. Utilise des **clés dédiées aux tests**, jamais ta clé personnelle, et range-les hors du dossier du projet.

### Avec le conteneur OpenSSH (Docker), sans la VM
Le serveur de la CI fonctionne aussi en local. Il crée `testpwd` (mot de passe lu dans `TEST_PASSWORD`) et `testkey` (clé uniquement, clés publiques lues dans `/keys/authorized_keys`), et régénère ses clés d'hôte à chaque démarrage. Exemple en shell POSIX (sous Git Bash, préfixer `docker run` par `MSYS_NO_PATHCONV=1` et donner au volume un chemin Windows `C:/Users/<toi>/…`, sinon Git Bash réécrit `/keys/…`) :
```bash
dir=~/.younji-lab/docker && install -d -m 700 "$dir"
ssh-keygen -q -t ed25519 -f "$dir/id_ed25519" -C younji-lab        # passphrase demandée
cat "$dir"/id_*.pub > "$dir/authorized_keys"
printf 'TEST_PASSWORD=%s\n' "$(openssl rand -hex 24)" > "$dir/sshd.env"   # pas sur la ligne de commande
docker build -t younji-sshd tests/docker/sshd
docker run -d --rm --name younji-sshd -p 127.0.0.1:2222:22 \
  --env-file "$dir/sshd.env" -v "$dir/authorized_keys:/keys/authorized_keys:ro" younji-sshd
```
Puis, dans `.env.test` : `YOUNJI_TEST_SSH_HOST=127.0.0.1`, `YOUNJI_TEST_SSH_PORT=2222`, `YOUNJI_TEST_SSH_USER=testpwd`, le mot de passe de `sshd.env`, `YOUNJI_TEST_SSH_KEY_USER=testkey` et les chemins des clés. Arrête le conteneur avec `docker stop younji-sshd`.
