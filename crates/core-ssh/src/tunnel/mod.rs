//! Tunnels SSH (TUN-01 à TUN-03) : redirections locale (`-L`), distante (`-R`) et dynamique
//! SOCKS5 (`-D`), chacune sur une connexion SSH dédiée (voir [`crate::SessionHandle::spawn_tunnel`]).
//!
//! Règles de sécurité (SECURITE.md §4.4) :
//! - écoute sur `127.0.0.1` par défaut ; `0.0.0.0` seulement si la définition le demande, ce
//!   que le coffre n'accepte qu'avec une confirmation explicite ;
//! - un canal `forwarded-tcpip` n'est accepté que s'il correspond au port demandé par le
//!   tunnel `-R` de cette connexion, et il est relié à la destination **configurée**, jamais à
//!   une adresse annoncée par le serveur ;
//! - nombre de connexions simultanées borné, négociation SOCKS et connexions locales bornées
//!   en temps ;
//! - aucune adresse (destination, client) dans les journaux.

pub(crate) mod socks;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::Channel;
use russh::client::{self, Handle};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc};
use tokio::time::{interval, sleep, timeout};
use tokio_util::sync::CancellationToken;

use crate::SshError;
use crate::target::normalize_host;

/// Connexions relayées simultanément par un tunnel ; au-delà, les nouvelles sont refusées.
pub const MAX_TUNNEL_CONNECTIONS: usize = 128;
/// Délai de la négociation SOCKS5 d'un client.
const SOCKS_TIMEOUT: Duration = Duration::from_secs(10);
/// Délai d'ouverture d'un canal `direct-tcpip` vers la destination.
const OPEN_TIMEOUT: Duration = Duration::from_secs(20);
/// Délai de connexion à la destination locale d'un tunnel `-R`.
const LOCAL_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Délai de la demande `tcpip-forward` au serveur.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(20);
/// Intervalle minimal entre deux envois des compteurs à l'interface.
const STATS_INTERVAL: Duration = Duration::from_millis(250);
/// Intervalle de vérification de la connexion SSH (tunnels `-L` et `-D`).
const LIVENESS_INTERVAL: Duration = Duration::from_secs(1);
/// Canaux `forwarded-tcpip` en attente de prise en charge.
const FORWARDED_QUEUE: usize = 16;

/// Portée de l'écoute (TUN-03). Fermée : aucune adresse libre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenScope {
    /// `127.0.0.1`.
    Loopback,
    /// `0.0.0.0`.
    AllInterfaces,
}

impl ListenScope {
    fn ip(self) -> IpAddr {
        match self {
            Self::Loopback => IpAddr::V4(Ipv4Addr::LOCALHOST),
            Self::AllInterfaces => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        }
    }
}

/// Type de redirection et destination validée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelMode {
    /// `-L` : destination joignable depuis le serveur.
    Local {
        /// Hôte de destination, normalisé.
        host: String,
        /// Port de destination.
        port: u16,
    },
    /// `-R` : destination joignable depuis le poste.
    Remote {
        /// Hôte de destination, normalisé.
        host: String,
        /// Port de destination.
        port: u16,
    },
    /// `-D` : proxy SOCKS5.
    Dynamic,
}

/// Définition validée d'un tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelSpec {
    mode: TunnelMode,
    scope: ListenScope,
    port: u16,
}

impl TunnelSpec {
    /// Redirection locale `-L` : écoute sur le poste, destination vue du serveur.
    pub fn local(
        scope: ListenScope,
        port: u32,
        dest_host: &str,
        dest_port: u32,
    ) -> Result<Self, SshError> {
        let (host, port_to) = destination(dest_host, dest_port)?;
        Self::new(
            TunnelMode::Local {
                host,
                port: port_to,
            },
            scope,
            port,
        )
    }

    /// Redirection distante `-R` : écoute sur le serveur, destination vue du poste.
    pub fn remote(
        scope: ListenScope,
        port: u32,
        dest_host: &str,
        dest_port: u32,
    ) -> Result<Self, SshError> {
        let (host, port_to) = destination(dest_host, dest_port)?;
        Self::new(
            TunnelMode::Remote {
                host,
                port: port_to,
            },
            scope,
            port,
        )
    }

    /// Redirection dynamique `-D` : proxy SOCKS5 sur le poste.
    pub fn dynamic(scope: ListenScope, port: u32) -> Result<Self, SshError> {
        Self::new(TunnelMode::Dynamic, scope, port)
    }

    fn new(mode: TunnelMode, scope: ListenScope, port: u32) -> Result<Self, SshError> {
        Ok(Self {
            mode,
            scope,
            port: valid_port(port)?,
        })
    }

    /// Type et destination.
    pub fn mode(&self) -> &TunnelMode {
        &self.mode
    }

    /// Portée de l'écoute.
    pub fn scope(&self) -> ListenScope {
        self.scope
    }

    /// Port d'écoute.
    pub fn port(&self) -> u16 {
        self.port
    }
}

fn valid_port(port: u32) -> Result<u16, SshError> {
    u16::try_from(port)
        .ok()
        .filter(|p| *p != 0)
        .ok_or(SshError::InvalidPort)
}

fn destination(host: &str, port: u32) -> Result<(String, u16), SshError> {
    Ok((normalize_host(host)?, valid_port(port)?))
}

/// Redirection `-R` attendue par le `ClientHandler` de la connexion : seuls les canaux
/// `forwarded-tcpip` annoncés sur ce port sont acceptés, et transmis à la tâche du tunnel.
pub(crate) struct RemoteForward {
    port: u32,
    sender: mpsc::Sender<Channel<client::Msg>>,
}

/// Emplacement partagé entre le `ClientHandler` et la tâche du tunnel. Vide pour un shell ou
/// un déploiement : tout `forwarded-tcpip` est alors refusé.
pub(crate) type ForwardSlot = Arc<Mutex<Option<RemoteForward>>>;

/// Verdict du `ClientHandler` sur un canal `forwarded-tcpip` ouvert par le serveur.
pub(crate) enum ForwardedVerdict {
    /// Accepté : le canal est à transmettre par ce permis réservé.
    Accept(mpsc::OwnedPermit<Channel<client::Msg>>),
    /// Port non demandé par cette connexion.
    Unexpected,
    /// File pleine : le tunnel ne suit pas.
    Busy,
}

/// Décide du sort d'un canal `forwarded-tcpip` annoncé sur `port`.
pub(crate) fn forwarded_verdict(slot: &ForwardSlot, port: u32) -> ForwardedVerdict {
    let sender = match slot.lock() {
        Ok(guard) => guard
            .as_ref()
            .filter(|forward| forward.port == port)
            .map(|forward| forward.sender.clone()),
        Err(_) => None,
    };
    match sender {
        None => ForwardedVerdict::Unexpected,
        Some(sender) => match sender.try_reserve_owned() {
            Ok(permit) => ForwardedVerdict::Accept(permit),
            Err(_) => ForwardedVerdict::Busy,
        },
    }
}

/// Compteurs d'un tunnel.
#[derive(Default)]
pub(crate) struct Stats {
    active: AtomicU32,
    total: AtomicU64,
    changed: Notify,
}

impl Stats {
    /// Connexions en cours et total depuis le démarrage.
    pub(crate) fn snapshot(&self) -> (u32, u64) {
        (
            self.active.load(Ordering::Acquire),
            self.total.load(Ordering::Acquire),
        )
    }

    fn opened(self: &Arc<Self>) -> ConnectionGuard {
        self.active.fetch_add(1, Ordering::AcqRel);
        self.total.fetch_add(1, Ordering::AcqRel);
        self.changed.notify_one();
        ConnectionGuard(self.clone())
    }
}

/// Décompte une connexion à sa fin, quelle qu'en soit la cause.
struct ConnectionGuard(Arc<Stats>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
        self.0.changed.notify_one();
    }
}

/// Ce dont le tunnel a besoin de la session qui le porte.
pub(crate) struct TunnelContext<'a> {
    pub handle: &'a Arc<Handle<crate::session::ClientHandler>>,
    pub forward: &'a ForwardSlot,
    pub cancel: &'a CancellationToken,
    /// Appelé une fois l'écoute en place, avec le port écouté.
    pub on_ready: &'a (dyn Fn(u16) + Send + Sync),
    /// Appelé à chaque changement des compteurs (au plus toutes les 250 ms).
    pub on_stats: Arc<dyn Fn(u32, u64) + Send + Sync>,
}

/// Fait vivre le tunnel jusqu'à son arrêt ou la perte de la connexion. Renvoie la cause de la
/// fin. Au retour, l'écoute est fermée (port libéré), les connexions relayées sont annulées et
/// plus aucun compteur n'est envoyé.
pub(crate) async fn run(spec: TunnelSpec, ctx: TunnelContext<'_>) -> SshError {
    let tunnel_cancel = ctx.cancel.child_token();
    let stats = Arc::new(Stats::default());
    let reporter = tokio::spawn(report_stats(
        stats.clone(),
        tunnel_cancel.clone(),
        ctx.on_stats.clone(),
    ));
    let limit = Arc::new(Semaphore::new(MAX_TUNNEL_CONNECTIONS));
    let relays = Relays {
        handle: ctx.handle.clone(),
        stats,
        limit,
        cancel: tunnel_cancel.clone(),
    };
    let reason = match spec.mode.clone() {
        TunnelMode::Local { host, port } => {
            listen(&spec, &ctx, &relays, Target::Fixed(host, port)).await
        }
        TunnelMode::Dynamic => listen(&spec, &ctx, &relays, Target::Socks).await,
        TunnelMode::Remote { host, port } => remote(&spec, &ctx, &relays, (host, port)).await,
    };
    tunnel_cancel.cancel();
    let _ = reporter.await;
    reason
}

/// Destination d'une connexion acceptée sur le poste.
#[derive(Clone)]
enum Target {
    Fixed(String, u16),
    Socks,
}

/// Ressources partagées par les connexions relayées d'un tunnel.
struct Relays {
    handle: Arc<Handle<crate::session::ClientHandler>>,
    stats: Arc<Stats>,
    limit: Arc<Semaphore>,
    cancel: CancellationToken,
}

impl Relays {
    fn permit(&self) -> Option<OwnedSemaphorePermit> {
        let permit = self.limit.clone().try_acquire_owned().ok();
        if permit.is_none() {
            tracing::warn!("tunnel : trop de connexions simultanées, nouvelle connexion refusée");
        }
        permit
    }
}

/// `-L` et `-D` : écoute sur le poste et relais vers le serveur.
async fn listen(
    spec: &TunnelSpec,
    ctx: &TunnelContext<'_>,
    relays: &Relays,
    target: Target,
) -> SshError {
    let listener = match TcpListener::bind(SocketAddr::new(spec.scope.ip(), spec.port)).await {
        Ok(listener) => listener,
        Err(error) => return bind_error(&error),
    };
    (ctx.on_ready)(spec.port);
    let mut liveness = interval(LIVENESS_INTERVAL);
    loop {
        tokio::select! {
            biased;
            () = ctx.cancel.cancelled() => return SshError::SessionClosed,
            _ = liveness.tick() => {
                if ctx.handle.is_closed() {
                    return SshError::ConnectionLost;
                }
            }
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        tracing::debug!(%error, "tunnel : connexion entrante refusée par le système");
                        continue;
                    }
                };
                let Some(permit) = relays.permit() else { continue };
                let _ = stream.set_nodelay(true);
                let guard = relays.stats.opened();
                let handle = relays.handle.clone();
                let cancel = relays.cancel.clone();
                let target = target.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    let _guard = guard;
                    tokio::select! {
                        () = cancel.cancelled() => {}
                        () = relay_outbound(handle, stream, peer, target) => {}
                    }
                });
            }
        }
    }
}

/// Relaie une connexion acceptée sur le poste vers un canal `direct-tcpip`.
async fn relay_outbound(
    handle: Arc<Handle<crate::session::ClientHandler>>,
    mut stream: TcpStream,
    peer: SocketAddr,
    target: Target,
) {
    let (host, port, socks) = match target {
        Target::Fixed(host, port) => (host, port, false),
        Target::Socks => match timeout(SOCKS_TIMEOUT, socks::handshake(&mut stream)).await {
            Ok(Ok(request)) => (request.host, request.port, true),
            Ok(Err(error)) => {
                tracing::debug!(?error, "tunnel : négociation SOCKS refusée");
                return;
            }
            Err(_) => {
                tracing::debug!("tunnel : négociation SOCKS trop lente");
                return;
            }
        },
    };
    let opened = timeout(
        OPEN_TIMEOUT,
        handle.channel_open_direct_tcpip(
            host,
            u32::from(port),
            peer.ip().to_string(),
            u32::from(peer.port()),
        ),
    )
    .await;
    let channel = match opened {
        Ok(Ok(channel)) => channel,
        Ok(Err(error)) => {
            tracing::debug!(%error, "tunnel : le serveur refuse la destination");
            if socks {
                socks::reply(&mut stream, socks::Reply::HostUnreachable).await;
            }
            return;
        }
        Err(_) => {
            if socks {
                socks::reply(&mut stream, socks::Reply::GeneralFailure).await;
            }
            return;
        }
    };
    if socks {
        socks::reply(&mut stream, socks::Reply::Succeeded).await;
    }
    pipe(stream, channel.into_stream()).await;
}

/// `-R` : demande d'écoute au serveur et relais des canaux reçus vers la destination locale.
async fn remote(
    spec: &TunnelSpec,
    ctx: &TunnelContext<'_>,
    relays: &Relays,
    (dest_host, dest_port): (String, u16),
) -> SshError {
    let (sender, mut receiver) = mpsc::channel(FORWARDED_QUEUE);
    let port = u32::from(spec.port);
    // Inscription avant la demande : un canal ouvert aussitôt après l'acceptation est reconnu.
    if let Ok(mut slot) = ctx.forward.lock() {
        *slot = Some(RemoteForward { port, sender });
    } else {
        return SshError::ChannelFailed;
    }
    let address = spec.scope.ip().to_string();
    let requested = tokio::select! {
        () = ctx.cancel.cancelled() => return SshError::SessionClosed,
        result = timeout(FORWARD_TIMEOUT, ctx.handle.tcpip_forward(address.clone(), port)) => result,
    };
    match requested {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            tracing::debug!(%error, "tunnel : écoute refusée par le serveur");
            return SshError::TunnelForwardRefused;
        }
        Err(_) => return SshError::Timeout,
    }
    (ctx.on_ready)(spec.port);
    let mut liveness = interval(LIVENESS_INTERVAL);
    let reason = loop {
        tokio::select! {
            biased;
            () = ctx.cancel.cancelled() => break SshError::SessionClosed,
            _ = liveness.tick() => {
                if ctx.handle.is_closed() {
                    break SshError::ConnectionLost;
                }
            }
            channel = receiver.recv() => {
                // Le `ClientHandler` garde un émetteur tant que la connexion vit.
                let Some(channel) = channel else { break SshError::ConnectionLost };
                let Some(permit) = relays.permit() else { continue };
                let guard = relays.stats.opened();
                let cancel = relays.cancel.clone();
                let host = dest_host.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    let _guard = guard;
                    tokio::select! {
                        () = cancel.cancelled() => {}
                        () = relay_inbound(channel, host, dest_port) => {}
                    }
                });
            }
        }
    };
    if let Ok(mut slot) = ctx.forward.lock() {
        slot.take();
    }
    if !ctx.handle.is_closed() {
        let _ = timeout(
            Duration::from_secs(2),
            ctx.handle.cancel_tcpip_forward(address, port),
        )
        .await;
    }
    reason
}

/// Relaie un canal `forwarded-tcpip` vers la destination locale configurée.
async fn relay_inbound(channel: Channel<client::Msg>, host: String, port: u16) {
    let stream = match timeout(
        LOCAL_CONNECT_TIMEOUT,
        TcpStream::connect((host.as_str(), port)),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(error)) => {
            tracing::debug!(%error, "tunnel : destination locale injoignable");
            let _ = channel.close().await;
            return;
        }
        Err(_) => {
            let _ = channel.close().await;
            return;
        }
    };
    let _ = stream.set_nodelay(true);
    pipe(stream, channel.into_stream()).await;
}

/// Copie dans les deux sens jusqu'à la fin des deux côtés.
async fn pipe<A, B>(mut a: A, mut b: B)
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    if let Err(error) = tokio::io::copy_bidirectional(&mut a, &mut b).await {
        tracing::debug!(%error, "tunnel : connexion relayée interrompue");
    }
}

/// Envoie les compteurs à chaque changement, au plus toutes les [`STATS_INTERVAL`].
async fn report_stats(
    stats: Arc<Stats>,
    cancel: CancellationToken,
    on_stats: Arc<dyn Fn(u32, u64) + Send + Sync>,
) {
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return,
            () = stats.changed.notified() => {}
        }
        tokio::select! {
            biased;
            () = cancel.cancelled() => return,
            () = sleep(STATS_INTERVAL) => {}
        }
        let (active, total) = stats.snapshot();
        on_stats(active, total);
    }
}

fn bind_error(error: &std::io::Error) -> SshError {
    tracing::debug!(%error, "tunnel : écoute impossible");
    if error.kind() == std::io::ErrorKind::AddrInUse {
        SshError::TunnelPortInUse
    } else {
        SshError::TunnelBindFailed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_are_validated() {
        let spec = TunnelSpec::local(ListenScope::Loopback, 8080, "LOCALHOST", 80).unwrap();
        assert_eq!(
            spec.mode(),
            &TunnelMode::Local {
                host: "localhost".into(),
                port: 80
            }
        );
        assert_eq!(spec.port(), 8080);
        assert_eq!(
            TunnelSpec::local(ListenScope::Loopback, 0, "h", 80),
            Err(SshError::InvalidPort)
        );
        assert_eq!(
            TunnelSpec::remote(ListenScope::Loopback, 70_000, "h", 80),
            Err(SshError::InvalidPort)
        );
        assert_eq!(
            TunnelSpec::remote(ListenScope::Loopback, 80, "h", 0),
            Err(SshError::InvalidPort)
        );
        assert_eq!(
            TunnelSpec::local(ListenScope::Loopback, 80, "a b", 80),
            Err(SshError::InvalidHost)
        );
        assert_eq!(
            TunnelSpec::local(ListenScope::Loopback, 80, "[::1]", 80)
                .unwrap()
                .mode(),
            &TunnelMode::Local {
                host: "::1".into(),
                port: 80
            }
        );
        assert!(TunnelSpec::dynamic(ListenScope::AllInterfaces, 1080).is_ok());
    }

    #[test]
    fn scopes_map_to_fixed_addresses() {
        assert_eq!(ListenScope::Loopback.ip().to_string(), "127.0.0.1");
        assert_eq!(ListenScope::AllInterfaces.ip().to_string(), "0.0.0.0");
    }

    #[test]
    fn only_the_requested_port_is_forwarded() {
        let slot = ForwardSlot::default();
        assert!(matches!(
            forwarded_verdict(&slot, 8080),
            ForwardedVerdict::Unexpected
        ));
        let (sender, _receiver) = mpsc::channel(1);
        *slot.lock().unwrap() = Some(RemoteForward { port: 8080, sender });
        assert!(matches!(
            forwarded_verdict(&slot, 22),
            ForwardedVerdict::Unexpected
        ));
        let first = forwarded_verdict(&slot, 8080);
        assert!(matches!(first, ForwardedVerdict::Accept(_)));
        // File pleine tant que le permis n'est pas consommé.
        assert!(matches!(
            forwarded_verdict(&slot, 8080),
            ForwardedVerdict::Busy
        ));
    }

    #[test]
    fn stats_count_open_and_closed_connections() {
        let stats = Arc::new(Stats::default());
        let a = stats.opened();
        let b = stats.opened();
        assert_eq!(stats.snapshot(), (2, 2));
        drop(a);
        assert_eq!(stats.snapshot(), (1, 2));
        drop(b);
        assert_eq!(stats.snapshot(), (0, 2));
    }
}
