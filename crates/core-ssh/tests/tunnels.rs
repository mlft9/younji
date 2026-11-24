//! Tests de bout en bout des tunnels (TUN-01 à TUN-03) contre un serveur russh lancé en
//! mémoire, sur 127.0.0.1. Les destinations sont des serveurs d'écho locaux : aucun accès
//! réseau externe. Toutes les clés sont générées à l'exécution.

// Fichier de test : les fonctions utilitaires échouent volontairement par panique.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use core_ssh::{
    Credentials, KnownHosts, ListenScope, SessionEvent, SessionHandle, SessionSink, SshError,
    Target, Timeouts, TunnelParams, TunnelSpec,
};
use russh::keys::{Algorithm, PrivateKey};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, MethodKind};
use secrecy::SecretString;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

const PASSWORD: &str = "mot-de-passe-de-test";
const WAIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- serveur de test

#[derive(Default)]
struct Shared {
    /// Destinations demandées par `direct-tcpip`.
    direct: Vec<(String, u32)>,
    /// Écoutes demandées par `tcpip-forward`.
    forwards: Vec<(String, u32)>,
    /// Écoutes annulées par `cancel-tcpip-forward`.
    cancelled: Vec<(String, u32)>,
    /// Pilotage de la dernière session (pour ouvrir des canaux `forwarded-tcpip`).
    handle: Option<server::Handle>,
}

#[derive(Clone)]
struct TestServer {
    shared: Arc<Mutex<Shared>>,
    /// Le serveur refuse les demandes `tcpip-forward`.
    refuse_forward: bool,
}

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: server::ChannelOpenHandle,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        {
            let mut shared = self.shared.lock().unwrap();
            shared
                .direct
                .push((host_to_connect.to_owned(), port_to_connect));
            shared.handle = Some(session.handle());
        }
        let target = (
            host_to_connect.to_owned(),
            u16::try_from(port_to_connect).unwrap(),
        );
        match TcpStream::connect((target.0.as_str(), target.1)).await {
            Ok(mut tcp) => {
                reply.accept().await;
                tokio::spawn(async move {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
                });
            }
            Err(_) => {
                reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
            }
        }
        Ok(())
    }

    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let mut shared = self.shared.lock().unwrap();
        shared.forwards.push((address.to_owned(), *port));
        shared.handle = Some(session.handle());
        Ok(!self.refuse_forward)
    }

    async fn cancel_tcpip_forward(
        &mut self,
        address: &str,
        port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.shared
            .lock()
            .unwrap()
            .cancelled
            .push((address.to_owned(), port));
        Ok(true)
    }
}

struct Fixture {
    port: u16,
    shared: Arc<Mutex<Shared>>,
    known_hosts: Arc<KnownHosts>,
    _dir: tempfile::TempDir,
}

async fn start_server(refuse_forward: bool) -> Fixture {
    let host_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![host_key],
        methods: (&[MethodKind::Password][..]).into(),
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        ..server::Config::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let handler = TestServer {
        shared: shared.clone(),
        refuse_forward,
    };
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let config = config.clone();
            let handler = handler.clone();
            tokio::spawn(async move {
                if let Ok(running) = server::run_stream(config, socket, handler).await {
                    let _ = running.await;
                }
            });
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let known_hosts = Arc::new(KnownHosts::new(dir.path().join("known_hosts")));
    Fixture {
        port,
        shared,
        known_hosts,
        _dir: dir,
    }
}

/// Serveur d'écho TCP local : renvoie tout ce qu'il reçoit.
async fn echo_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut read, mut write) = socket.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    port
}

/// Port libre sur 127.0.0.1 au moment de l'appel.
async fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

// ---------------------------------------------------------------- client de test

struct EventSink(mpsc::UnboundedSender<SessionEvent>);

impl SessionSink for EventSink {
    fn event(&self, event: SessionEvent) {
        let _ = self.0.send(event);
    }
    fn output(&self, _data: Vec<u8>) {}
}

struct Tunnel {
    handle: SessionHandle,
    rx: mpsc::UnboundedReceiver<SessionEvent>,
}

impl Tunnel {
    fn start(fixture: &Fixture, spec: TunnelSpec) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let params = TunnelParams {
            target: Target::new("127.0.0.1", u32::from(fixture.port), "testeur").unwrap(),
            credentials: Credentials::password(SecretString::from(PASSWORD.to_owned())).unwrap(),
            spec,
            known_hosts: fixture.known_hosts.clone(),
            timeouts: Timeouts {
                host_key_decision: Duration::from_secs(5),
                ..Timeouts::default()
            },
        };
        Self {
            handle: SessionHandle::spawn_tunnel(params, Arc::new(EventSink(tx))),
            rx,
        }
    }

    async fn next_event(&mut self) -> SessionEvent {
        match tokio::time::timeout(WAIT, self.rx.recv()).await {
            Ok(Some(event)) => event,
            Ok(None) => panic!("flux d'événements fermé"),
            Err(_) => panic!("aucun événement reçu à temps"),
        }
    }

    /// Accepte la clé d'hôte et attend que le tunnel écoute ; renvoie le port écouté.
    async fn accept_and_wait_ready(&mut self) -> u16 {
        assert!(matches!(
            self.next_event().await,
            SessionEvent::HostKeyUnknown { .. }
        ));
        self.handle.decide_host_key(true).unwrap();
        assert_eq!(self.next_event().await, SessionEvent::Authenticating);
        match self.next_event().await {
            SessionEvent::TunnelReady { port } => port,
            other => panic!("événement inattendu : {other:?}"),
        }
    }

    /// Attend des compteurs qui vérifient `predicate`.
    async fn wait_stats(&mut self, predicate: impl Fn(u32, u64) -> bool) {
        loop {
            match self.next_event().await {
                SessionEvent::TunnelStats { active, total } if predicate(active, total) => return,
                SessionEvent::TunnelStats { .. } => {}
                other => panic!("événement inattendu : {other:?}"),
            }
        }
    }

    /// Attend la fin du tunnel et renvoie sa cause.
    async fn wait_end(&mut self) -> SshError {
        loop {
            match self.next_event().await {
                SessionEvent::Disconnected { reason } => return reason,
                SessionEvent::TunnelStats { .. } => {}
                other => panic!("événement inattendu : {other:?}"),
            }
        }
    }
}

async fn round_trip<S>(stream: &mut S, message: &[u8])
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    stream.write_all(message).await.unwrap();
    let mut back = vec![0u8; message.len()];
    tokio::time::timeout(WAIT, stream.read_exact(&mut back))
        .await
        .expect("écho reçu à temps")
        .unwrap();
    assert_eq!(back, message);
}

// ---------------------------------------------------------------- tests

#[tokio::test]
async fn local_forward_reaches_the_destination_and_stop_frees_the_port() {
    let fixture = start_server(false).await;
    let echo = echo_server().await;
    let port = free_port().await;
    let spec = TunnelSpec::local(
        ListenScope::Loopback,
        u32::from(port),
        "127.0.0.1",
        u32::from(echo),
    )
    .unwrap();
    let mut tunnel = Tunnel::start(&fixture, spec);
    assert_eq!(tunnel.accept_and_wait_ready().await, port);

    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    round_trip(&mut client, b"bonjour par -L").await;
    tunnel
        .wait_stats(|active, total| active == 1 && total == 1)
        .await;
    assert_eq!(
        fixture.shared.lock().unwrap().direct,
        vec![("127.0.0.1".to_owned(), u32::from(echo))]
    );
    drop(client);
    tunnel
        .wait_stats(|active, total| active == 0 && total == 1)
        .await;

    tunnel.handle.close();
    assert_eq!(tunnel.wait_end().await, SshError::SessionClosed);
    // L'arrêt libère le port.
    TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("port libéré après l'arrêt");
}

#[tokio::test]
async fn stopping_closes_relayed_connections() {
    let fixture = start_server(false).await;
    let echo = echo_server().await;
    let port = free_port().await;
    let spec = TunnelSpec::local(
        ListenScope::Loopback,
        u32::from(port),
        "127.0.0.1",
        u32::from(echo),
    )
    .unwrap();
    let mut tunnel = Tunnel::start(&fixture, spec);
    tunnel.accept_and_wait_ready().await;
    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    round_trip(&mut client, b"ping").await;
    tunnel.handle.close();
    assert_eq!(tunnel.wait_end().await, SshError::SessionClosed);
    // La connexion relayée est fermée avec le tunnel.
    let mut buf = [0u8; 8];
    let read = tokio::time::timeout(WAIT, client.read(&mut buf))
        .await
        .expect("fermeture vue à temps");
    assert!(matches!(read, Ok(0) | Err(_)));
}

#[tokio::test]
async fn dynamic_forward_speaks_socks5() {
    let fixture = start_server(false).await;
    let echo = echo_server().await;
    let port = free_port().await;
    let spec = TunnelSpec::dynamic(ListenScope::Loopback, u32::from(port)).unwrap();
    let mut tunnel = Tunnel::start(&fixture, spec);
    tunnel.accept_and_wait_ready().await;

    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    client.write_all(&[5, 1, 0]).await.unwrap();
    let mut method = [0u8; 2];
    client.read_exact(&mut method).await.unwrap();
    assert_eq!(method, [5, 0]);
    // CONNECT par nom de domaine : résolu par le serveur, pas par le poste.
    let mut request = vec![5, 1, 0, 3, 9];
    request.extend_from_slice(b"localhost");
    request.extend_from_slice(&echo.to_be_bytes());
    client.write_all(&request).await.unwrap();
    let mut reply = [0u8; 10];
    client.read_exact(&mut reply).await.unwrap();
    assert_eq!(reply[..2], [5, 0]);
    round_trip(&mut client, b"bonjour par -D").await;
    assert_eq!(
        fixture.shared.lock().unwrap().direct,
        vec![("localhost".to_owned(), u32::from(echo))]
    );

    // Destination refusée par le serveur : réponse d'échec SOCKS, pas de blocage.
    let closed = free_port().await;
    let mut other = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    other.write_all(&[5, 1, 0]).await.unwrap();
    other.read_exact(&mut method).await.unwrap();
    let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
    request.extend_from_slice(&closed.to_be_bytes());
    other.write_all(&request).await.unwrap();
    other.read_exact(&mut reply).await.unwrap();
    assert_ne!(reply[1], 0);

    tunnel.handle.close();
    assert_eq!(tunnel.wait_end().await, SshError::SessionClosed);
}

#[tokio::test]
async fn remote_forward_reaches_the_configured_local_destination() {
    let fixture = start_server(false).await;
    let echo = echo_server().await;
    let spec =
        TunnelSpec::remote(ListenScope::Loopback, 9090, "127.0.0.1", u32::from(echo)).unwrap();
    let mut tunnel = Tunnel::start(&fixture, spec);
    assert_eq!(tunnel.accept_and_wait_ready().await, 9090);
    let server = {
        let shared = fixture.shared.lock().unwrap();
        // Portée « loopback » : le serveur est prié d'écouter sur 127.0.0.1 seulement.
        assert_eq!(shared.forwards, vec![("127.0.0.1".to_owned(), 9090)]);
        shared.handle.clone().unwrap()
    };

    // Le serveur annonce une adresse quelconque : elle est ignorée, la destination est celle
    // du tunnel.
    let channel = server
        .channel_open_forwarded_tcpip("10.9.8.7", 9090, "203.0.113.5", 40000)
        .await
        .expect("canal demandé accepté");
    let mut stream = channel.into_stream();
    round_trip(&mut stream, b"bonjour par -R").await;
    tunnel
        .wait_stats(|active, total| active == 1 && total == 1)
        .await;

    // Port non demandé : refusé.
    assert!(
        server
            .channel_open_forwarded_tcpip("127.0.0.1", 9091, "203.0.113.5", 40001)
            .await
            .is_err()
    );

    tunnel.handle.close();
    assert_eq!(tunnel.wait_end().await, SshError::SessionClosed);
}

#[tokio::test]
async fn unrequested_forwarded_channels_are_refused() {
    // Une connexion sans tunnel -R refuse tout canal forwarded-tcpip ouvert par le serveur.
    let fixture = start_server(false).await;
    let echo = echo_server().await;
    let port = free_port().await;
    let spec = TunnelSpec::local(
        ListenScope::Loopback,
        u32::from(port),
        "127.0.0.1",
        u32::from(echo),
    )
    .unwrap();
    let mut tunnel = Tunnel::start(&fixture, spec);
    tunnel.accept_and_wait_ready().await;
    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    round_trip(&mut client, b"x").await;
    let server = fixture.shared.lock().unwrap().handle.clone().unwrap();
    assert!(
        server
            .channel_open_forwarded_tcpip("127.0.0.1", u32::from(port), "203.0.113.5", 1)
            .await
            .is_err()
    );
    // Les autres canaux ouverts par le serveur sont refusés eux aussi (russh les accepte par
    // défaut) : session, agent, direct-tcpip.
    assert!(server.channel_open_session().await.is_err());
    assert!(server.channel_open_agent().await.is_err());
    assert!(
        server
            .channel_open_direct_tcpip("127.0.0.1", 22, "203.0.113.5", 1)
            .await
            .is_err()
    );
    tunnel.handle.close();
    assert_eq!(tunnel.wait_end().await, SshError::SessionClosed);
}

#[tokio::test]
async fn port_in_use_is_reported() {
    let fixture = start_server(false).await;
    let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = taken.local_addr().unwrap().port();
    let spec = TunnelSpec::dynamic(ListenScope::Loopback, u32::from(port)).unwrap();
    let mut tunnel = Tunnel::start(&fixture, spec);
    assert!(matches!(
        tunnel.next_event().await,
        SessionEvent::HostKeyUnknown { .. }
    ));
    tunnel.handle.decide_host_key(true).unwrap();
    assert_eq!(tunnel.next_event().await, SessionEvent::Authenticating);
    assert_eq!(tunnel.wait_end().await, SshError::TunnelPortInUse);
}

#[tokio::test]
async fn refused_remote_forward_is_reported() {
    let fixture = start_server(true).await;
    let spec = TunnelSpec::remote(ListenScope::AllInterfaces, 9090, "127.0.0.1", 80).unwrap();
    let mut tunnel = Tunnel::start(&fixture, spec);
    assert!(matches!(
        tunnel.next_event().await,
        SessionEvent::HostKeyUnknown { .. }
    ));
    tunnel.handle.decide_host_key(true).unwrap();
    assert_eq!(tunnel.next_event().await, SessionEvent::Authenticating);
    assert_eq!(tunnel.wait_end().await, SshError::TunnelForwardRefused);
    // Portée « toutes les interfaces » : demandée explicitement au serveur.
    assert_eq!(
        fixture.shared.lock().unwrap().forwards,
        vec![("0.0.0.0".to_owned(), 9090)]
    );
}
