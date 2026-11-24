//! Tests d'intégration contre un vrai serveur OpenSSH (conteneur en CI, VM du lab en local).
//!
//! Ignorés par défaut ; lancés par `npm run test:integration`. La cible est lue dans les
//! variables d'environnement (jamais d'identifiant en dur, voir docs/ENVIRONNEMENT_DE_TEST.md) :
//!
//! | Variable | Rôle |
//! |---|---|
//! | `YOUNJI_TEST_SSH_HOST`, `YOUNJI_TEST_SSH_PORT` | Serveur (port 22 par défaut) |
//! | `YOUNJI_TEST_SSH_USER`, `YOUNJI_TEST_SSH_PASSWORD` | Compte à mot de passe |
//! | `YOUNJI_TEST_SSH_KEY_USER`, `YOUNJI_TEST_SSH_KEY_PATH` | Compte à clé et sa clé privée |
//! | `YOUNJI_TEST_SSH_KEY_PASSPHRASE` | Passphrase de cette clé (facultative) |
//! | `YOUNJI_TEST_SSH_EXTRA_KEYS` | Autres clés non chiffrées (ECDSA, RSA…), séparées par `;` |
//! | `YOUNJI_TEST_REQUIRED` | `1` : une variable manquante fait échouer au lieu d'ignorer |
//!
//! Chaque test utilise un `known_hosts` vierge dans un dossier temporaire.

// Fichier de test : les fonctions utilitaires échouent volontairement par panique.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use core_ssh::auth::load_private_key;
use core_ssh::{
    AuthorizedKeyLine, ConnectParams, Credentials, DeployParams, KeyKind, KnownHosts, SessionEvent,
    SessionHandle, SessionSink, SshError, Target, TermSize, Timeouts,
};
use secrecy::SecretString;
use tokio::sync::mpsc;

const WAIT: Duration = Duration::from_secs(20);

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Lit une variable obligatoire pour ce test ; `None` (test ignoré) si elle manque, sauf si
/// `YOUNJI_TEST_REQUIRED=1`.
fn required(name: &str) -> Option<String> {
    let value = var(name);
    if value.is_none() {
        assert!(
            var("YOUNJI_TEST_REQUIRED").as_deref() != Some("1"),
            "variable {name} manquante alors que YOUNJI_TEST_REQUIRED=1"
        );
        eprintln!("test ignoré : variable {name} absente");
    }
    value
}

fn host_and_port() -> Option<(String, u32)> {
    let host = required("YOUNJI_TEST_SSH_HOST")?;
    let port = var("YOUNJI_TEST_SSH_PORT").map_or(22, |p| p.parse().expect("port invalide"));
    Some((host, port))
}

enum Received {
    Event(SessionEvent),
    Output(Vec<u8>),
}

struct Sink(mpsc::UnboundedSender<Received>);

impl SessionSink for Sink {
    fn event(&self, event: SessionEvent) {
        let _ = self.0.send(Received::Event(event));
    }
    fn output(&self, data: Vec<u8>) {
        let _ = self.0.send(Received::Output(data));
    }
}

struct Client {
    handle: SessionHandle,
    rx: mpsc::UnboundedReceiver<Received>,
    output: String,
    _dir: tempfile::TempDir,
    known_hosts: Arc<KnownHosts>,
}

impl Client {
    fn connect(user: &str, credentials: Credentials) -> Option<Self> {
        let (host, port) = host_and_port()?;
        let dir = tempfile::tempdir().unwrap();
        let known_hosts = Arc::new(KnownHosts::new(dir.path().join("known_hosts")));
        Some(Self::connect_with(
            &host,
            port,
            user,
            credentials,
            known_hosts,
            dir,
        ))
    }

    fn connect_with(
        host: &str,
        port: u32,
        user: &str,
        credentials: Credentials,
        known_hosts: Arc<KnownHosts>,
        dir: tempfile::TempDir,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let params = ConnectParams {
            target: Target::new(host, port, user).unwrap(),
            credentials,
            size: TermSize::new(80, 24).unwrap(),
            known_hosts: known_hosts.clone(),
            timeouts: Timeouts::default(),
        };
        Self {
            handle: SessionHandle::spawn(params, Arc::new(Sink(tx))),
            rx,
            output: String::new(),
            _dir: dir,
            known_hosts,
        }
    }

    async fn next_event(&mut self) -> SessionEvent {
        loop {
            match tokio::time::timeout(WAIT, self.rx.recv()).await {
                Ok(Some(Received::Event(event))) => return event,
                Ok(Some(Received::Output(data))) => self.absorb(&data),
                _ => panic!("aucun événement reçu à temps"),
            }
        }
    }

    fn absorb(&mut self, data: &[u8]) {
        self.handle.ack(data.len());
        self.output.push_str(&String::from_utf8_lossy(data));
    }

    async fn wait_output(&mut self, needle: &str) {
        let deadline = tokio::time::Instant::now() + WAIT;
        while !self.output.contains(needle) {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(left, self.rx.recv()).await {
                Ok(Some(Received::Output(data))) => self.absorb(&data),
                Ok(Some(Received::Event(event))) => panic!("événement inattendu : {event:?}"),
                _ => panic!("{needle:?} absent de la sortie :\n{}", self.output),
            }
        }
    }

    /// Accepte la clé d'hôte (known_hosts vierge) et attend l'ouverture du shell.
    async fn accept_and_open(&mut self) {
        match self.next_event().await {
            SessionEvent::HostKeyUnknown { key } => {
                assert!(key.fingerprint.starts_with("SHA256:"));
            }
            other => panic!("attendu HostKeyUnknown, reçu {other:?}"),
        }
        self.handle.decide_host_key(true).unwrap();
        assert_eq!(self.next_event().await, SessionEvent::Authenticating);
        assert_eq!(self.next_event().await, SessionEvent::Connected);
    }

    /// Vérifie le shell : calcul, taille du terminal après redimensionnement, sortie propre.
    async fn exercise_shell(&mut self) {
        // Le calcul évite de confondre l'écho de la commande avec son résultat.
        self.handle
            .write(b"echo younji-$((40+2))\r".to_vec())
            .await
            .unwrap();
        self.wait_output("younji-42").await;
        self.handle
            .write(b"echo term=$TERM\r".to_vec())
            .await
            .unwrap();
        self.wait_output("term=xterm-256color").await;

        self.handle
            .resize(TermSize::new(132, 43).unwrap())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.handle.write(b"stty size\r".to_vec()).await.unwrap();
        self.wait_output("43 132").await;

        self.handle.write(b"exit\r".to_vec()).await.unwrap();
        assert_eq!(
            self.next_event().await,
            SessionEvent::Disconnected {
                reason: SshError::SessionEnded
            }
        );
    }
}

fn password_credentials(value: String) -> Credentials {
    Credentials::password(SecretString::from(value)).unwrap()
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH de test"]
async fn password_session_runs_a_full_terminal() {
    let (Some(user), Some(password)) = (
        required("YOUNJI_TEST_SSH_USER"),
        required("YOUNJI_TEST_SSH_PASSWORD"),
    ) else {
        return;
    };
    let Some(mut client) = Client::connect(&user, password_credentials(password)) else {
        return;
    };
    client.accept_and_open().await;
    client.exercise_shell().await;
}

/// Lettres accentuées tapées au clavier : bash (readline) les reçoit octet par octet. Sans
/// locale UTF-8 côté serveur, readline les convertit en séquences « Méta » et n'affiche rien
/// (un collage encadré, lui, passait). Le serveur de test n'a pas de `LANG`, comme une Debian
/// minimale.
#[tokio::test]
#[ignore = "nécessite un serveur SSH de test"]
async fn typed_accented_letters_reach_the_shell() {
    let (Some(user), Some(password)) = (
        required("YOUNJI_TEST_SSH_USER"),
        required("YOUNJI_TEST_SSH_PASSWORD"),
    ) else {
        return;
    };
    let Some(mut client) = Client::connect(&user, password_credentials(password)) else {
        return;
    };
    client.accept_and_open().await;
    // Comme au clavier : chaque caractère dans une écriture séparée, sans collage encadré.
    for chunk in ["echo ", "<", "é", "è", "à", "ç", ">", "\r"] {
        client
            .handle
            .write(chunk.as_bytes().to_vec())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    client.wait_output("<éèàç>\r\n").await;
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH de test"]
async fn wrong_password_is_refused() {
    let Some(user) = required("YOUNJI_TEST_SSH_USER") else {
        return;
    };
    let wrong = password_credentials("mauvais-mot-de-passe-younji".into());
    let Some(mut client) = Client::connect(&user, wrong) else {
        return;
    };
    assert!(matches!(
        client.next_event().await,
        SessionEvent::HostKeyUnknown { .. }
    ));
    client.handle.decide_host_key(true).unwrap();
    assert_eq!(client.next_event().await, SessionEvent::Authenticating);
    assert_eq!(
        client.next_event().await,
        SessionEvent::Disconnected {
            reason: SshError::AuthFailed
        }
    );
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH de test"]
async fn key_session_with_passphrase() {
    let (Some(user), Some(path)) = (
        required("YOUNJI_TEST_SSH_KEY_USER"),
        required("YOUNJI_TEST_SSH_KEY_PATH"),
    ) else {
        return;
    };
    let passphrase = var("YOUNJI_TEST_SSH_KEY_PASSPHRASE").map(SecretString::from);
    let key = load_private_key(&PathBuf::from(path), passphrase.as_ref()).unwrap();
    let Some(mut client) = Client::connect(&user, Credentials::PrivateKey(Arc::new(key))) else {
        return;
    };
    client.accept_and_open().await;
    client.exercise_shell().await;
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH de test"]
async fn other_key_types_authenticate() {
    let (Some(user), Some(paths)) = (
        var("YOUNJI_TEST_SSH_KEY_USER"),
        var("YOUNJI_TEST_SSH_EXTRA_KEYS"),
    ) else {
        eprintln!("test ignoré : aucune clé supplémentaire");
        return;
    };
    for path in paths.split(';').filter(|p| !p.is_empty()) {
        let key = load_private_key(&PathBuf::from(path), None).unwrap();
        eprintln!("clé {}", key.algorithm().as_str());
        let Some(mut client) = Client::connect(&user, Credentials::PrivateKey(Arc::new(key)))
        else {
            return;
        };
        client.accept_and_open().await;
        client.handle.write(b"exit\r".to_vec()).await.unwrap();
        assert_eq!(
            client.next_event().await,
            SessionEvent::Disconnected {
                reason: SshError::SessionEnded
            }
        );
    }
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH de test"]
async fn changed_host_key_is_blocked() {
    let (Some(user), Some(password)) = (
        required("YOUNJI_TEST_SSH_USER"),
        required("YOUNJI_TEST_SSH_PASSWORD"),
    ) else {
        return;
    };
    let Some((host, port)) = host_and_port() else {
        return;
    };
    // L'hôte est déjà « connu » avec une autre clé (fixture publique).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("known_hosts");
    let target = Target::new(&host, port, &user).unwrap();
    let pattern = core_ssh::known_hosts::host_pattern(target.host(), target.port());
    let other = include_str!("fixtures/test_host_ed25519.pub");
    let mut fields = other.split_whitespace();
    let line = format!(
        "{pattern} {} {}\n",
        fields.next().unwrap(),
        fields.next().unwrap()
    );
    std::fs::write(&path, line).unwrap();
    let known_hosts = Arc::new(KnownHosts::new(path));

    let mut client = Client::connect_with(
        &host,
        port,
        &user,
        password_credentials(password),
        known_hosts,
        dir,
    );
    assert!(matches!(
        client.next_event().await,
        SessionEvent::HostKeyChanged { .. }
    ));
    assert_eq!(
        client.next_event().await,
        SessionEvent::Disconnected {
            reason: SshError::HostKeyChanged
        }
    );
    // Le fichier n'a pas été modifié : l'ancienne clé reste la seule enregistrée.
    let content = std::fs::read_to_string(client.known_hosts.path().unwrap()).unwrap();
    assert_eq!(content.lines().count(), 1);
}

/// Déploie une clé et renvoie l'issue (`Some(true)` : ajoutée, `Some(false)` : déjà présente).
async fn deploy_once(user: &str, password: &str, line: &AuthorizedKeyLine) -> Option<bool> {
    let (host, port) = host_and_port()?;
    let dir = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let params = DeployParams {
        target: Target::new(&host, port, user).unwrap(),
        credentials: password_credentials(password.to_owned()),
        key: line.clone(),
        known_hosts: Arc::new(KnownHosts::new(dir.path().join("known_hosts"))),
        timeouts: Timeouts::default(),
    };
    let handle = SessionHandle::spawn_deploy(params, Arc::new(Sink(tx)));
    loop {
        match tokio::time::timeout(WAIT, rx.recv()).await {
            Ok(Some(Received::Event(SessionEvent::HostKeyUnknown { .. }))) => {
                handle.decide_host_key(true).unwrap();
            }
            Ok(Some(Received::Event(SessionEvent::KeyDeployed { added }))) => return Some(added),
            Ok(Some(Received::Event(SessionEvent::Disconnected { reason }))) => {
                panic!("déploiement interrompu : {reason:?}")
            }
            Ok(Some(_)) => {}
            _ => panic!("aucun événement reçu à temps"),
        }
    }
}

/// KEY-03 : déployer une clé générée, redéployer sans doublon, se connecter par clé et
/// vérifier les permissions, puis retirer la clé ajoutée.
#[tokio::test]
#[ignore = "nécessite un serveur SSH de test"]
async fn generated_key_is_deployed_once_and_then_used() {
    let (Some(user), Some(password)) = (
        required("YOUNJI_TEST_SSH_USER"),
        required("YOUNJI_TEST_SSH_PASSWORD"),
    ) else {
        return;
    };
    let generated = core_ssh::generate_key(KeyKind::Ed25519).unwrap();
    let key = core_ssh::auth::decode_stored_key(&generated.private_key).unwrap();
    let line = AuthorizedKeyLine::from_private(&key).unwrap();

    assert_eq!(deploy_once(&user, &password, &line).await, Some(true));
    assert_eq!(deploy_once(&user, &password, &line).await, Some(false));

    let Some(mut client) = Client::connect(&user, Credentials::PrivateKey(Arc::new(key))) else {
        return;
    };
    client.accept_and_open().await;
    let blob = line.as_str().split(' ').nth(1).unwrap();
    // Une seule ligne pour cette clé, permissions 700 / 600 (`stat` GNU).
    let check = format!(
        "echo n=$(grep -cF {blob} ~/.ssh/authorized_keys) m=$(stat -c %a ~/.ssh ~/.ssh/authorized_keys)\r"
    );
    client.handle.write(check.into_bytes()).await.unwrap();
    client.wait_output("n=1 m=700 600").await;
    // Nettoyage : la clé de test ne reste pas sur le serveur.
    let cleanup = format!(
        "grep -vF {blob} ~/.ssh/authorized_keys > ~/.ssh/ak.tmp; cat ~/.ssh/ak.tmp > ~/.ssh/authorized_keys; rm -f ~/.ssh/ak.tmp; echo nettoye-$((1+1))\r"
    );
    client.handle.write(cleanup.into_bytes()).await.unwrap();
    client.wait_output("nettoye-2").await;
}

// ---------------------------------------------------------------------- SFTP (étape 4)

struct Reporter;

impl core_ssh::sftp::transfer::TransferReporter for Reporter {
    fn progress(&self, _progress: core_ssh::sftp::transfer::Progress) {}

    fn conflict(
        &self,
        _name: String,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = core_ssh::sftp::transfer::ConflictDecision>
                + Send
                + '_,
        >,
    > {
        Box::pin(async {
            core_ssh::sftp::transfer::ConflictDecision {
                action: core_ssh::sftp::transfer::ConflictAction::Skip,
                apply_to_all: true,
            }
        })
    }
}

fn read_tree(dir: &std::path::Path) -> Vec<(String, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).unwrap().flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                out.push((rel, None));
                stack.push(path);
            } else {
                out.push((rel, Some(std::fs::read(&path).unwrap())));
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH de test"]
async fn sftp_round_trip_of_a_recursive_folder() {
    use core_ssh::sftp::local::LocalRoot;
    use core_ssh::sftp::transfer::{ConflictPolicy, Download, LocalDest, Upload, download, upload};
    use core_ssh::sftp::{EntryKind, RemoteFs, ops};
    use tokio_util::sync::CancellationToken;

    let (Some(user), Some(password)) = (
        required("YOUNJI_TEST_SSH_USER"),
        required("YOUNJI_TEST_SSH_PASSWORD"),
    ) else {
        return;
    };
    let Some(mut client) = Client::connect(&user, password_credentials(password)) else {
        return;
    };
    client.accept_and_open().await;
    let sftp = Arc::new(client.handle.open_sftp().await.unwrap());

    // Dossier de travail unique dans le dossier personnel.
    let home = ops::home(sftp.as_ref()).await.unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = home.join(&format!("younji-sftp-{nonce}")).unwrap();
    ops::mkdir(sftp.as_ref(), &base).await.unwrap();

    // Arborescence locale : accents, espaces, fichier et dossier vides, fichier de 3 Mio.
    let local = tempfile::tempdir().unwrap();
    let src = local.path().join("Dossier été");
    std::fs::create_dir_all(src.join("sous dossier")).unwrap();
    std::fs::create_dir_all(src.join("dossier vide")).unwrap();
    std::fs::write(src.join("fichier vide"), b"").unwrap();
    std::fs::write(src.join("sous dossier/données ü.txt"), "contenu accentué é").unwrap();
    let big: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    std::fs::write(src.join("gros.bin"), &big).unwrap();

    let summary = upload(
        sftp.as_ref(),
        Upload {
            sources: vec![std::fs::canonicalize(&src).unwrap()],
            dest: base.clone(),
            policy: ConflictPolicy::Ask,
        },
        &Reporter,
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(summary.error, None, "{summary:?}");
    assert_eq!(summary.transferred, 3);

    let remote_dir = base.join("Dossier été").unwrap();
    let listing = ops::list(sftp.as_ref(), &remote_dir).await.unwrap();
    let mut names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["dossier vide", "fichier vide", "gros.bin", "sous dossier"]
    );

    // Téléchargement dans un autre dossier et comparaison octet par octet.
    let back = tempfile::tempdir().unwrap();
    let root = LocalRoot::new(back.path()).unwrap();
    let summary = download(
        sftp.as_ref(),
        Download {
            sources: vec![remote_dir.clone()],
            dest: LocalDest {
                dir: root.path().to_path_buf(),
                root,
            },
            policy: ConflictPolicy::Ask,
        },
        &Reporter,
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(summary.error, None, "{summary:?}");
    assert_eq!(summary.transferred, 3);
    assert_eq!(read_tree(local.path()), read_tree(back.path()));

    // Renommage sans écrasement, droits, suppression récursive.
    let file = remote_dir.join("fichier vide").unwrap();
    let renamed = remote_dir.join("renommé").unwrap();
    assert_eq!(
        ops::rename(sftp.as_ref(), &file, &remote_dir.join("gros.bin").unwrap()).await,
        Err(SshError::SftpAlreadyExists)
    );
    ops::rename(sftp.as_ref(), &file, &renamed).await.unwrap();
    ops::chmod(sftp.as_ref(), &renamed, 0o600).await.unwrap();
    let meta = sftp.lstat(&renamed).await.unwrap();
    assert_eq!(
        (meta.kind, meta.permissions),
        (EntryKind::File, Some(0o600))
    );
    ops::remove(sftp.as_ref(), &base).await.unwrap();
    assert_eq!(sftp.lstat(&base).await, Err(SshError::SftpNotFound));

    // Le SFTP vit avec la session : après `exit`, il ne répond plus.
    client.handle.write(b"exit\r".to_vec()).await.unwrap();
    assert_eq!(
        client.next_event().await,
        SessionEvent::Disconnected {
            reason: SshError::SessionEnded
        }
    );
    assert!(ops::home(sftp.as_ref()).await.is_err());
}
