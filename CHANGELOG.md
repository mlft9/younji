# Journal des modifications

Toutes les évolutions notables du projet sont consignées ici.
Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions selon [SemVer](https://semver.org/lang/fr/).

## [Non publié]

### Ajouté
- Cadrage du projet : cahier des charges, feuille de route, architecture, règles de sécurité, journal des décisions, environnement de test.
- Configuration de Claude Code : instructions du projet, agents (sécurité, QA, documentation), commandes `/etape`, `/retour`, `/release` et `/revue-securite`.
- Choix du nom définitif du projet : **Vigie** (ADR-014), puis changement pour **Younji** (ADR-017), « Vigie » étant déjà une marque déposée pour des logiciels.
- Première version de l'application de bureau (Tauri 2 + React) pour Windows et Linux.
- Mise en page de base : barre latérale (Hôtes, Clés, Snippets, Tunnels, SFTP, Paramètres) et zone d'onglets. Seuls les Paramètres sont fonctionnels pour l'instant.
- Interface en français et en anglais : langue du système détectée au démarrage, changement de langue à chaud depuis les Paramètres.
- Thème clair ou sombre, qui suit le système par défaut et peut être forcé.
- Affichage du nom et de la version de l'application dans les Paramètres.
- Journaux de diagnostic dans le dossier de l'application, conservés 7 jours, sans aucune donnée sensible. Leur niveau de détail se règle avec la variable `YOUNJI_LOG`.
- Outillage de développement : une seule commande (`npm run check`) vérifie le formatage, le lint, les types et les tests ; `npm run audit` contrôle les vulnérabilités et les licences des dépendances.
- Intégration continue : vérifications à chaque push (tests, audit des dépendances, détection de secrets) et compilation à la demande des installeurs Windows (NSIS, MSI) et Linux (deb, AppImage), téléchargeables directement depuis la page du run, sans archive .zip.
- Mises à jour hebdomadaires des dépendances par Dependabot, avec un délai de 7 jours avant d'adopter une nouvelle version.
- Feuille de route : phase 2 « Younji, le client qui protège » (étapes 14 à 22), issue du tri de 36 idées d'audit, de durcissement et de confort (28 retenues).
- Dépôt public sous licence GPL-3.0 (fichier `LICENSE`), avec une politique de signalement des vulnérabilités (`SECURITY.md`) (ADR-016).
- Connexion SSH rapide : hôte, port, utilisateur, puis mot de passe ou clé privée (Ed25519, ECDSA, RSA, avec ou sans passphrase). La clé se choisit dans la boîte de dialogue de fichiers du système.
- Terminal complet dans chaque onglet (couleurs, Unicode, vim, htop, tmux), avec redimensionnement répercuté sur le serveur et historique de 10 000 lignes.
- Plusieurs sessions en onglets : ouvrir, fermer, renommer (double-clic ou F2), réordonner par glisser-déposer ou au clavier. Un onglet masqué garde son contenu.
- Recherche dans le terminal, copier/coller (Ctrl+Maj+C / Ctrl+Maj+V, Maj+Inser, menu du clic droit) et liens web ouverts par Ctrl+clic dans le navigateur.
- Raccourcis clavier en Ctrl+Maj, pour laisser au shell les siens (Ctrl+C, Ctrl+R, Ctrl+W…). Leur liste est affichée dans les Paramètres.
- Détection des coupures réseau : l'onglet passe à « déconnecté » et propose de se reconnecter, avec le formulaire pré-rempli (le mot de passe ou la passphrase est redemandé).
- L'interface reste réactive même quand le serveur envoie un très gros volume de texte (`cat` d'un gros fichier, `yes`).
- Terminal basé sur xterm.js 5.5 : la version 6 empêchait l'application de démarrer avec le durcissement `freezePrototype` (fenêtre blanche) ; un test le vérifie désormais (ADR-025).
- Tests d'intégration contre un vrai serveur OpenSSH, lancés en conteneur à chaque push et utilisables en local (`npm run test:integration`).
- Coffre chiffré : au premier lancement, création avec un mot de passe maître (12 caractères au moins), un indicateur de robustesse en direct et un avertissement clair : un mot de passe oublié ne peut pas être récupéré. Aux lancements suivants, l'application s'ouvre verrouillée.
- Verrouillage manuel (bouton de la barre latérale ou Ctrl+Maj+L) et automatique après une période d'inactivité réglable (1, 5, 10, 15, 30 ou 60 minutes ; 15 par défaut). La mise en veille de l'ordinateur compte comme de l'inactivité.
- Changement du mot de passe maître depuis les Paramètres.
- Carnet d'hôtes : nom, adresse, port, utilisateur, identité, groupe, étiquettes et notes. Classement par groupes, recherche instantanée, double-clic pour se connecter.
- Identités réutilisables (utilisateur, avec mot de passe, clé ou les deux), partagées entre plusieurs hôtes. Une connexion ou une reconnexion depuis le carnet ne redemande rien. Si l'identité a une clé et un mot de passe, la clé est essayée d'abord.
- Section Clés : import d'une clé privée existante dans le coffre (sa passphrase n'est demandée qu'une fois), renommage, copie de la clé publique.
- Gestion des clés d'hôte connues : liste et suppression après un avertissement. L'alerte de clé modifiée y mène directement.
- Taille de police du terminal réglable dans les Paramètres.
- Double authentification (2FA) : quand le serveur pose des questions (code de vérification TOTP, mot de passe PAM…), elles s'affichent dans une fenêtre dédiée de l'onglet. Les serveurs qui exigent plusieurs méthodes à la suite (clé puis code, mot de passe puis code) sont pris en charge. Pour un hôte du carnet « mot de passe + code », le mot de passe enregistré est envoyé automatiquement et seul le code est demandé (ADR-031).
- Connexion rapide : nouveau mode « Questions du serveur (2FA) », où rien n'est saisi d'avance.
- Générateur de clés SSH : Ed25519 (par défaut), ECDSA P-256, P-384 ou P-521, RSA 4096. La clé est créée directement dans le coffre (ADR-032).
- Export de la clé publique dans un fichier `.pub`, à l'emplacement choisi dans la boîte de dialogue du système.
- Déploiement d'une clé sur un hôte du carnet : la clé publique est ajoutée à `~/.ssh/authorized_keys`, sans doublon si on recommence, avec des permissions restrictives (700 pour `~/.ssh`, 600 pour le fichier). Serveurs Unix uniquement (ADR-033).
- Explorateur SFTP sur la connexion d'une session de terminal ouverte, sans nouvelle authentification (ADR-034) :
  - panneau latéral à côté du terminal (bouton ou Ctrl+Maj+E), qui suit les `cd` du shell. Si le shell n'annonce pas son dossier, le bouton « Activer le suivi » insère dans le terminal une courte commande pour bash ou zsh, à relire puis valider par Entrée ;
  - onglet double panneau, dossier local à gauche et serveur à droite, avec glisser-déposer entre les deux ;
  - opérations sur le serveur : naviguer, créer un dossier, renommer, supprimer (avec confirmation), modifier les droits (chmod), envoyer et télécharger des fichiers ou des dossiers entiers ;
  - file de transferts avec progression, annulation et bilan ; quand un élément existe déjà, Younji demande s'il faut écraser, ignorer ou garder les deux, avec « appliquer aux suivants » ;
  - ouverture d'un fichier distant dans un éditeur de texte local (Bloc-notes sous Windows), renvoyé automatiquement au serveur à chaque enregistrement. Si le fichier a été modifié sur le serveur entre-temps, rien n'est écrasé sans votre accord.
- Écran partagé : un onglet de terminal se découpe à droite (Ctrl+Maj+D) ou en bas (Ctrl+Maj+O), jusqu'à 8 panneaux, chacun avec sa propre connexion (ADR-035) :
  - séparateurs redimensionnables à la souris ou au clavier ;
  - passage au panneau voisin par Ctrl+Maj+flèches, fermeture d'un panneau par Ctrl+Maj+X ou son bouton ✕ ;
  - le nom du serveur s'affiche en tête de chaque panneau ;
  - un nouveau panneau ouvert depuis un hôte du carnet se connecte seul ; depuis une connexion rapide, le mot de passe est redemandé.
- Diffusion de la saisie : ce que vous tapez part en même temps vers plusieurs sessions choisies, dans n'importe quel onglet (Ctrl+Maj+I ou l'icône antenne). Bandeau rouge en haut de la fenêtre, bordure rouge sur chaque session concernée, arrêt en un clic ou par Ctrl+Maj+I (ADR-036).
- Snippets : bibliothèque de commandes réutilisables enregistrée dans le coffre, avec recherche, création, modification et suppression. Les variables `{{nom}}` sont demandées au moment de l'envoi, avec un aperçu de la commande finale. Envoi vers la session active ou plusieurs sessions cochées, par Ctrl+Maj+S ou la barre d'outils du terminal, en « Insérer » (sans Entrée) ou « Exécuter » (ADR-037).
- Terminal local : nouvelle section « Terminal local » de la barre latérale, qui liste les shells installés sur le poste et les ouvre dans un onglet, dans votre dossier personnel (ADR-039) :
  - sous Windows : Windows PowerShell, PowerShell 7, Invite de commandes et chaque distribution WSL, détectées automatiquement ;
  - sous Linux : les shells de `/etc/shells` (bash, zsh, fish…), votre shell par défaut en premier ;
  - couleurs, redimensionnement, applications plein écran (vim, htop), recherche, copier/coller et écran partagé fonctionnent comme dans un terminal SSH ; quand le shell se termine, « Relancer » le rouvre.
- Bureau à distance (RDP) : un hôte du carnet peut être de type RDP, avec son domaine Windows et sa résolution (plein écran ou largeur × hauteur). Double-clic ou « Ouvrir en RDP » le lance dans le client du système avec l'identité de l'hôte, sans ressaisir le mot de passe (ADR-040) :
  - sous Windows : Connexion Bureau à distance (mstsc). Si vous avez déjà enregistré un identifiant Windows pour ce serveur, il est utilisé tel quel et Younji vous le signale ;
  - sous Linux : FreeRDP 3 (`xfreerdp3`, paquet `freerdp3-x11`), qui doit être installé ; l'identité de l'hôte doit alors contenir un mot de passe.
- Import de `~/.ssh/config` depuis le carnet (« Importer ~/.ssh/config » ou « Autre fichier… ») : les fichiers inclus (`Include`, avec jokers) sont suivis et les réglages communs (`Host *`, `Host lab-*`) appliqués comme le fait OpenSSH. Un aperçu liste les hôtes à cocher, avec un avertissement quand un réglage n'est pas repris (rebond `ProxyJump`, redirections, clé à importer à part), et les entrées ignorées avec leur raison. Les hôtes déjà présents dans le carnet sont décochés. Les hôtes importés rejoignent le groupe « Import OpenSSH » (ADR-041).
- Import d'un fichier `known_hosts` depuis Clés > Clés d'hôte connues : aperçu des clés ajoutées, des doublons et des lignes refusées avec leur raison. Les lignes à nom haché (`|1|…`) sont refusées : Younji ne s'en sert pas pour reconnaître un serveur.
- Sauvegarde chiffrée du coffre (Paramètres > Sauvegarde) : export protégé par une phrase de passe distincte du mot de passe maître, puis import sur le même poste ou sur un autre, en « Fusionner » (ajoute ce qui manque, sans rien écraser) ou « Remplacer » (restauration complète, après confirmation) (ADR-041).

### Retiré
- Tunnels SSH (étape 6, TUN-01 à TUN-03) : fonction retirée du périmètre, jugée non nécessaire pour le début du projet (ADR-038). La section « Tunnels » de la barre latérale, qui n'affichait qu'un emplacement réservé, est supprimée.

### Modifié
- Toute l'application, connexion rapide comprise, n'est accessible qu'après le déverrouillage du coffre.
- Verrouiller l'application, ou recharger l'interface, ferme toutes les sessions SSH et efface le contenu des terminaux (ADR-026).
- Les clés d'hôte acceptées sont désormais conservées dans le coffre, et non plus dans un fichier `known_hosts` en clair. Celles déjà acceptées sont reprises automatiquement à la création du coffre (ADR-027).
- Les préférences (langue, thème, police) sont enregistrées par l'application dans un fichier `settings.json` ; les réglages déjà faits sont repris automatiquement.
- Une identité avec une clé seule, refusée par le serveur, peut désormais aboutir aux questions du serveur (mot de passe, code), comme avec OpenSSH.
- Le temps passé à répondre aux questions du serveur n'est plus compté dans le délai réseau de la connexion ; chaque demande laisse 2 minutes pour répondre.
- Fermer un onglet de terminal ferme aussi les onglets SFTP qui utilisent sa connexion ; verrouiller l'application arrête aussi les transferts et les éditions en cours.
- Un coffre enregistré par cette version ne peut plus être ouvert par une version plus ancienne de Younji, qui le refuse sans rien perdre (ADR-037).
- Verrouiller l'application, ou recharger l'interface, ferme aussi les terminaux locaux et arrête les programmes qui y tournent.
- Verrouiller l'application, la recharger ou la fermer ferme aussi les fenêtres RDP ouvertes depuis Younji.

### Corrigé
- Un coffre enregistré par une version de test de l'étape 6 (tunnels, retirée) était refusé à l'ouverture avec « Mot de passe incorrect ou coffre endommagé ». Le champ `tunnels` est désormais accepté puis oublié ; il disparaît à la prochaine sauvegarde du coffre. Tout autre champ inconnu reste refusé.
- Les lettres accentuées tapées au clavier n'apparaissaient pas sur un serveur sans locale (bash en locale « C ») : Younji propose désormais `LANG=C.UTF-8` et `LC_CTYPE=C.UTF-8` à l'ouverture du shell, comme OpenSSH envoie `LANG LC_*` ; `LC_CTYPE` reste efficace quand le serveur réécrit `LANG=C` (conteneurs Proxmox).
- Un panneau non connecté peut être fermé (bouton ✕ toujours visible sur un onglet découpé).
- Le menu contextuel natif de la WebView (dont « Recharger », qui fermait toutes les sessions) est bloqué hors des champs de saisie.

### Sécurité
- Le client SSH refuse tout canal que le serveur tente d'ouvrir vers le poste (redirections, X11, transfert d'agent, nouveau shell) ; la bibliothèque SSH les acceptait par défaut.
- Configuration Tauri durcie : politique de sécurité du contenu (CSP) stricte, aucun contenu distant, pas d'API Tauri globale exposée à l'interface, prototypes JavaScript gelés.
- Chaque commande du cœur doit être autorisée explicitement pour la fenêtre (capabilities) ; les appels de l'interface passent en plus par une iframe d'isolation qui filtre les commandes (ADR-015).
- La fenêtre ne peut afficher que le contenu de l'application : la navigation vers une autre adresse et l'ouverture de nouvelles fenêtres sont refusées.
- Tests automatiques qui échouent si la configuration de sécurité est affaiblie.
- L'inspecteur web (clic droit → Inspecter) n'existe qu'en mode développement ; un test garantit que la version publiée ne l'embarque pas.
- Vérification des clés d'hôte SSH : à la première connexion, l'empreinte `SHA256:…` est affichée et doit être acceptée. Si la clé d'un serveur connu change, même pour un autre type de clé, la connexion est bloquée avec une alerte, sans moyen de passer outre. Les clés acceptées sont conservées dans un fichier `known_hosts` propre à Younji (ADR-023).
- Algorithmes SSH modernes uniquement : échange de clés hybride post-quantique en priorité, ni SHA-1, ni CBC, ni compression (ADR-019).
- Les mots de passe et passphrases ne sont transmis qu'une fois au cœur de l'application, détruits dès la fin de l'authentification (une copie interne à la bibliothèque SSH ne peut pas être effacée, limite documentée), et jamais renvoyés à l'interface ni écrits dans les journaux. L'interface ne connaît jamais le chemin ni le contenu de la clé privée (ADR-020, ADR-021).
- Collage protégé : un texte de plusieurs lignes ou contenant des caractères invisibles demande une confirmation avec aperçu, et ne peut pas sortir du mode « collage » du shell.
- Le serveur ne peut pas lire le presse-papiers par une séquence d'échappement. Un lien dont le texte affiché peut différer de l'adresse réelle demande une confirmation qui montre cette adresse ; seuls les liens `http` et `https` s'ouvrent (ADR-022).
- Les messages d'accueil du serveur sont affichés comme du texte simple, sans séquences d'échappement, et leur taille est limitée.
- La CSP admet désormais les balises de style intégrées dont le terminal a besoin, sans autoriser les attributs de style ni aucune source distante (ADR-018).
- Recharger l'interface ferme proprement les sessions ouvertes ; F5 et Ctrl+R sont désactivés hors du terminal pour éviter une fermeture accidentelle.
- Coffre chiffré par Argon2id et XChaCha20-Poly1305 : le fichier ne contient aucune donnée lisible et toute modification est détectée. Un mauvais mot de passe et un fichier abîmé donnent le même message, sans indice sur la cause (ADR-005).
- Après 3 mots de passe maître erronés, chaque nouvel essai est ralenti, jusqu'à 30 secondes, y compris la vérification du mot de passe actuel lors d'un changement (ADR-028).
- Le verrouillage automatique est décidé par le cœur de l'application, pas par l'interface. Son délai est stocké dans le coffre : il ne peut pas être modifié en dehors de l'application (ADR-028).
- La robustesse du mot de passe maître est vérifiée par le cœur de l'application, pas seulement par l'indicateur affiché (ADR-029).
- Les mots de passe des identités et les clés privées ne sont jamais renvoyés à l'interface : seules la présence d'un mot de passe, la clé publique et l'empreinte sont affichées (ADR-030).
- Les clés d'hôte, chiffrées et authentifiées dans le coffre, ne peuvent plus être lues ni falsifiées par un autre logiciel (ADR-027).
- Après un changement de mot de passe maître, la copie de sauvegarde du coffre est remplacée par une copie du nouveau coffre, ou supprimée : l'ancien mot de passe n'ouvre plus aucune copie gérée par Younji.
- Le coffre est écrit de façon à ne jamais être laissé à moitié écrit, avec une copie de sauvegarde de la version précédente ; sa création n'écrase jamais un coffre existant ni une copie de sauvegarde restée seule.
- Questions du serveur affichées comme du texte simple, débarrassées des caractères de contrôle et invisibles, et limitées en taille et en nombre. La fenêtre rappelle quel serveur les pose et que Younji ne demande jamais le mot de passe maître à cet endroit (protection contre l'hameçonnage).
- Le mot de passe enregistré n'est envoyé automatiquement qu'à une question « Password » / « Mot de passe » exacte et masquée, une seule fois : jamais à une question comme « New password ». Les codes saisis ne sont ni conservés, ni journalisés, ni renvoyés à l'interface.
- Les clés générées utilisent l'aléa du système et ne quittent jamais le coffre : aucun export de la clé privée. L'export de la clé publique impose l'extension `.pub` et n'écrase jamais un fichier sans confirmation, pour ne pas remplacer par erreur une clé privée voisine.
- Le déploiement de clé n'insère jamais la clé dans une commande exécutée sur le serveur : elle est transmise séparément, ce qui empêche toute injection. Il passe par les mêmes vérifications qu'une connexion (clé d'hôte, 2FA) et s'interrompt au verrouillage.
- Un serveur SFTP malveillant ne peut pas écrire en dehors du dossier local choisi : les noms dangereux (`../`, `C:\…`, `CON`, `nul.txt`, `fichier:flux`, caractères invisibles ou qui inversent le sens du texte…) sont refusés, jamais corrigés, et comptés dans le bilan du transfert. Ces noms sont signalés dans la liste des fichiers.
- Les liens symboliques ne sont pas suivis lors d'un transfert de dossier, et un lien piégé déposé dans le dossier de destination ne peut pas détourner l'écriture.
- Un fichier n'est jamais écrasé sans que vous l'ayez demandé, et un transfert annulé ne laisse pas de fichier à moitié copié sur votre ordinateur.
- Younji ne peut lire et écrire que dans les dossiers locaux que vous avez choisis dans la boîte de dialogue du système, et l'interface ne connaît jamais leur chemin. La racine d'un disque, le dossier personnel entier, `~/.ssh` et les dossiers de Younji sont refusés. Le panneau local ne permet ni de supprimer ni de renommer des fichiers (ADR-034).
- Un fichier distant ouvert pour être modifié s'ouvre toujours dans un éditeur de texte, jamais dans le programme associé à son extension : un `.bat` ou un `.js` n'est pas exécuté. Sa copie locale n'est lisible que par vous et elle est supprimée à la fin de l'édition, de la session ou au verrouillage.
- Sous Windows, les fichiers téléchargés sont marqués comme venant d'Internet : SmartScreen et le mode protégé d'Office s'appliquent. Sous Linux, ils ne sont lisibles que par vous.
- Un serveur qui renvoie des listes de fichiers démesurées ou des arborescences trop profondes ne peut pas saturer la mémoire ni figer l'application.
- Les fichiers glissés depuis le bureau ou l'explorateur du système dans la fenêtre sont ignorés.
- Les journaux ne contiennent aucun nom ni contenu de fichier transféré.
- La diffusion de la saisie ne s'active qu'après avoir coché les sessions visées dans une liste qui montre le nom et l'adresse de chaque serveur (seule la session active est cochée d'office). Elle n'est jamais mémorisée, s'arrête au verrouillage, et une session déconnectée en sort définitivement, même après reconnexion.
- Pendant la diffusion, un collage demande toujours confirmation, même sur une seule ligne, et rappelle les serveurs visés. Seules vos frappes sont diffusées : un serveur ne peut pas faire écrire son terminal dans les autres sessions.
- Les snippets restent chiffrés dans le coffre. Leur contenu (hormis retours à la ligne et tabulations) et les valeurs des variables ne peuvent contenir ni caractère de contrôle ni caractère invisible (par exemple ceux qui inversent le sens du texte), pour qu'aucune commande cachée ne parte vers le serveur. Un snippet n'est exécuté que sur demande explicite, après aperçu, et seulement dans les sessions cochées.
- Le terminal local ne peut lancer que les shells détectés par Younji : aucun champ ne permet de choisir un autre programme, et une variable `SHELL` modifiée ne fait pas lancer un programme absent de `/etc/shells`. Il n'est accessible qu'avec le coffre déverrouillé ; fermer un onglet ou verrouiller arrête le shell, même s'il ignore le signal de fin habituel. Rien de ce qui s'y tape ou s'y affiche n'est écrit dans les journaux. Un programme que vous laissez tourner en arrière-plan (`commande &`) peut survivre à la fermeture du shell, comme dans un terminal classique.
- RDP : le mot de passe n'apparaît jamais dans la liste des processus ni dans un fichier. Sous Windows, il passe par un identifiant temporaire du gestionnaire d'identifiants, que les autres programmes ne peuvent pas relire, jamais écrit sur disque, et supprimé dès la fermeture de la fenêtre RDP ou au plus tard 60 secondes après son ouverture (puis à nouveau au démarrage suivant si Younji a été arrêté brutalement). Sous Linux, il est transmis à FreeRDP par un canal privé, jamais en argument. Si vous mettez plus de 60 secondes à accepter le certificat d'un nouveau serveur, mstsc vous redemandera le mot de passe.
- RDP sous Linux : le certificat d'un serveur est mémorisé à la première connexion ; s'il change ensuite, la connexion est refusée sans question, comme pour les clés d'hôte SSH.
- Les réglages d'un hôte RDP (adresse, utilisateur, domaine) sont contrôlés pour qu'aucune option ou instruction cachée ne puisse être glissée au client RDP. Seul le presse-papiers est partagé avec le serveur : ni disques, ni imprimantes, ni cartes à puce.
- Un hôte RDP ne peut jamais être utilisé pour une connexion SSH ni pour déployer une clé.
- L'import de `~/.ssh/config` ne lit jamais vos clés privées : les fichiers `IdentityFile` sont seulement signalés, à importer vous-même dans Clés. Les blocs `Match` ne sont pas interprétés. Chaque hôte importé est contrôlé comme une saisie manuelle, et des fichiers inclus trop nombreux, trop gros ou en boucle sont ignorés et signalés.
- L'import d'un `known_hosts` n'ajoute jamais une autre clé pour un serveur déjà connu : la ligne est refusée et signalée comme conflit. Les lignes `@cert-authority` et les motifs (`*.lab`) sont ignorés.
- La phrase de passe d'une sauvegarde doit être aussi robuste que le mot de passe maître et différente de lui. Chaque export compte comme un essai de mot de passe : plusieurs exports d'affilée sont ralentis, comme le déverrouillage.
- Une sauvegarde est chiffrée et authentifiée comme le coffre, avec sa propre clé : un seul octet modifié la fait refuser, et une mauvaise phrase de passe donne le même message qu'un fichier abîmé. Son contenu est revérifié entièrement à l'import, empreintes des clés comprises ; un fichier invalide est refusé en bloc, sans rien modifier.
- « Remplacer » restaure aussi les clés d'hôte de la sauvegarde telles quelles : à réserver à vos propres sauvegardes. « Fusionner » applique la même règle de conflit que l'import de `known_hosts`.
- Le délai de verrouillage automatique enregistré dans le coffre ou dans une sauvegarde est revérifié à l'ouverture : un fichier qui tenterait de désactiver le verrouillage est refusé.
- Younji ne lit que des fichiers ordinaires lors d'un import (ni périphérique ni tube), et toujours en quantité limitée.
