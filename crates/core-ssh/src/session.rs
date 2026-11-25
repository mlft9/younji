//! Session SSH interactive : connexion, clé d'hôte, authentification, PTY et shell
//! (SSH-01, SSH-02, SSH-04, SSH-06, AUTH-03), et déploiement d'une clé publique (KEY-03), qui
//! partage la même connexion, la même vérification de clé d'hôte et la même authentification.
//!
//! Chaque session vit dans une tâche tokio propriétaire, annulable. L'appelant la pilote via
//! un [`SessionHandle`] et reçoit les événements et la sortie du terminal via un
//! [`SessionSink`]. La crate ne connaît pas Tauri : `src-tauri` fournit le `SessionSink`.
//!
//! **Contrôle de flux** : la sortie est envoyée par lots (quelques millisecondes ou 32 Kio).
//! L'appelant acquitte les octets une fois affichés ([`SessionHandle::ack`]). Au-delà de
//! [`HIGH_WATERMARK`] octets non acquittés, la session cesse de lire le canal SSH : la
//! fenêtre SSH se remplit et le serveur s'arrête d'émettre. La mémoire reste bornée et
//! l'interface n'est jamais submergée, même avec `yes` ou un `cat` de plusieurs gigaoctets.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::MethodKind;
use russh::client::{self, Handle, KeyboardInteractiveAuthResponse};
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use russh::{ChannelMsg, ChannelWriteHalf, Disconnect};
use secrecy::{ExposeSecret, SecretString};
use tokio::net::TcpStream;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::{Instant, sleep_until, timeout};
use tokio_util::sync::CancellationToken;

use crate::auth::Credentials;
use crate::config::{Timeouts, client_config};
use crate::deploy::{self, AuthorizedKeyLine, DeployOutcome};
use crate::known_hosts::{HostKeyStatus, KeyInfo, KnownHosts};
use crate::prompt::{AuthPrompt, MAX_PROMPT_ROUNDS, PromptAnswers};
use crate::sftp::SftpClient;
use crate::{SshError, Target, TermSize};

/// Type de terminal annoncé au serveur.
const TERM: &str = "xterm-256color";
/// Taille maximale d'un envoi de données vers le serveur en un seul appel.
pub const MAX_INPUT_CHUNK: usize = 64 * 1024;
/// Délai maximal de regroupement de la sortie avant envoi.
const BATCH_DELAY: Duration = Duration::from_millis(8);
/// Taille maximale d'un lot de sortie.
const BATCH_MAX: usize = 32 * 1024;
/// Octets envoyés mais non acquittés au-delà desquels la lecture est suspendue.
pub const HIGH_WATERMARK: usize = 2 * 1024 * 1024;
/// Commandes en attente vers la tâche de session (frappes, redimensionnements).
const CONTROL_CAPACITY: usize = 256;
/// Taille maximale d'une bannière serveur affichée.
const MAX_BANNER_LEN: usize = 8 * 1024;
/// Total des bannières affichées par session : au-delà, un serveur qui en enverrait en
/// boucle pendant l'authentification est ignoré.
const MAX_BANNER_TOTAL: usize = 64 * 1024;

/// Événement d'état d'une session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// Clé d'hôte inconnue : l'appelant doit appeler [`SessionHandle::decide_host_key`].
    HostKeyUnknown {
        /// Clé présentée par le serveur.
        key: KeyInfo,
    },
    /// Clé d'hôte différente de celle enregistrée : la connexion est refusée.
    HostKeyChanged {
        /// Clé présentée par le serveur.
        key: KeyInfo,
        /// Clés enregistrées pour cet hôte.
        known: Vec<KeyInfo>,
    },
    /// Clé d'hôte acceptée, authentification en cours. Émis aussi après chaque réponse de
    /// l'utilisateur à une demande du serveur.
    Authenticating,
    /// Le serveur pose des questions (keyboard-interactive : code TOTP…) : l'appelant doit
    /// appeler [`SessionHandle::answer_prompts`] avec le même numéro de tour.
    AuthPrompt {
        /// Numéro de la demande (1, 2…) : une réponse ne vaut que pour ce tour.
        round: u32,
        /// Questions du serveur.
        prompt: AuthPrompt,
    },
    /// Déploiement de clé terminé avec succès (le dernier événement suit aussitôt).
    KeyDeployed {
        /// `true` si la clé a été ajoutée, `false` si elle était déjà présente.
        added: bool,
    },
    /// Shell ouvert, la session est utilisable.
    Connected,
    /// Session terminée. Dernier événement émis.
    Disconnected {
        /// Cause, sous forme de code stable.
        reason: SshError,
    },
}

/// Destinataire des événements et de la sortie d'une session.
pub trait SessionSink: Send + Sync + 'static {
    /// Changement d'état.
    fn event(&self, event: SessionEvent);
    /// Lot d'octets produits par le terminal distant, à acquitter une fois affiché.
    fn output(&self, data: Vec<u8>);
}

/// Paramètres d'ouverture d'une session.
#[derive(Debug)]
pub struct ConnectParams {
    /// Cible validée.
    pub target: Target,
    /// Moyen d'authentification. Il est détruit dès la fin de l'authentification.
    pub credentials: Credentials,
    /// Taille initiale du terminal.
    pub size: TermSize,
    /// Fichier des clés d'hôte connues.
    pub known_hosts: Arc<KnownHosts>,
    /// Délais.
    pub timeouts: Timeouts,
}

/// Paramètres d'un déploiement de clé publique (KEY-03).
#[derive(Debug)]
pub struct DeployParams {
    /// Cible validée.
    pub target: Target,
    /// Moyen d'authentification. Il est détruit dès la fin de l'authentification.
    pub credentials: Credentials,
    /// Ligne de clé publique à ajouter dans `authorized_keys`.
    pub key: AuthorizedKeyLine,
    /// Fichier des clés d'hôte connues.
    pub known_hosts: Arc<KnownHosts>,
    /// Délais.
    pub timeouts: Timeouts,
}

/// Ce que fait la session une fois authentifiée.
enum Purpose {
    Shell(TermSize),
    Deploy(AuthorizedKeyLine),
}

enum Control {
    Input(Vec<u8>),
    Resize(TermSize),
    /// Ouvre un client SFTP sur la connexion de la session (SFTP-01).
    OpenSftp(oneshot::Sender<Result<SftpClient, SshError>>),
}

/// Délai d'ouverture d'un client SFTP (deux canaux et initialisation du protocole).
const SFTP_OPEN_TIMEOUT: Duration = Duration::from_secs(45);

type DecisionSlot = Arc<Mutex<Option<oneshot::Sender<bool>>>>;

/// Demande keyboard-interactive en attente de réponse : nombre de réponses attendues et
/// destinataire (`None` = annulation).
struct PendingPrompt {
    round: u32,
    expected: usize,
    sender: oneshot::Sender<Option<PromptAnswers>>,
}

type PromptSlot = Arc<Mutex<Option<PendingPrompt>>>;

/// Compteur des octets envoyés à l'appelant et pas encore acquittés.
#[derive(Default)]
struct Flow {
    unacked: AtomicUsize,
    acked: Notify,
}

impl Flow {
    fn unacked(&self) -> usize {
        self.unacked.load(Ordering::Acquire)
    }

    fn sent(&self, bytes: usize) {
        self.unacked.fetch_add(bytes, Ordering::AcqRel);
    }

    fn ack(&self, bytes: usize) {
        // Un acquittement excessif (interface bogguée ou malveillante) ne fait pas déborder le
        // compteur : il le ramène à zéro.
        let mut current = self.unacked.load(Ordering::Acquire);
        loop {
            let next = current.saturating_sub(bytes);
            match self.unacked.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
        self.acked.notify_one();
    }
}

/// Pilotage d'une session. La détruire ferme la session.
pub struct SessionHandle {
    control: mpsc::Sender<Control>,
    decision: DecisionSlot,
    prompt: PromptSlot,
    flow: Arc<Flow>,
    cancel: CancellationToken,
}

impl std::fmt::Debug for SessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionHandle")
            .field("closed", &self.cancel.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl SessionHandle {
    /// Démarre une session dans une nouvelle tâche tokio (à appeler dans un contexte tokio).
    pub fn spawn(params: ConnectParams, sink: Arc<dyn SessionSink>) -> Self {
        let ConnectParams {
            target,
            credentials,
            size,
            known_hosts,
            timeouts,
        } = params;
        let common = Common {
            target,
            credentials,
            known_hosts,
            timeouts,
        };
        Self::start(common, Purpose::Shell(size), sink)
    }

    /// Démarre un déploiement de clé publique dans une nouvelle tâche tokio. Le pilotage
    /// (clé d'hôte, questions du serveur, fermeture) est le même que pour une session ;
    /// l'issue arrive par [`SessionEvent::KeyDeployed`] puis [`SessionEvent::Disconnected`].
    pub fn spawn_deploy(params: DeployParams, sink: Arc<dyn SessionSink>) -> Self {
        let DeployParams {
            target,
            credentials,
            key,
            known_hosts,
            timeouts,
        } = params;
        let common = Common {
            target,
            credentials,
            known_hosts,
            timeouts,
        };
        Self::start(common, Purpose::Deploy(key), sink)
    }

    fn start(common: Common, purpose: Purpose, sink: Arc<dyn SessionSink>) -> Self {
        let (control, control_rx) = mpsc::channel(CONTROL_CAPACITY);
        let decision = DecisionSlot::default();
        let prompt = PromptSlot::default();
        let flow = Arc::new(Flow::default());
        let cancel = CancellationToken::new();
        let task = SessionTask {
            control_rx,
            decision: decision.clone(),
            prompt: prompt.clone(),
            flow: flow.clone(),
            cancel: cancel.clone(),
            sink,
        };
        tokio::spawn(task.run(common, purpose));
        Self {
            control,
            decision,
            prompt,
            flow,
            cancel,
        }
    }

    /// Envoie des octets au shell distant (frappes, collage).
    pub async fn write(&self, data: Vec<u8>) -> Result<(), SshError> {
        if data.len() > MAX_INPUT_CHUNK {
            return Err(SshError::InputTooLarge);
        }
        if data.is_empty() {
            return Ok(());
        }
        self.control
            .send(Control::Input(data))
            .await
            .map_err(|_| SshError::SessionClosed)
    }

    /// Répercute un redimensionnement du terminal sur le serveur.
    pub async fn resize(&self, size: TermSize) -> Result<(), SshError> {
        self.control
            .send(Control::Resize(size))
            .await
            .map_err(|_| SshError::SessionClosed)
    }

    /// Ouvre un client SFTP sur la connexion de cette session, sans nouvelle authentification
    /// (même clé d'hôte vérifiée). La session doit être un terminal ouvert. Le client vit
    /// avec la session : il cesse de fonctionner quand elle se ferme.
    pub async fn open_sftp(&self) -> Result<SftpClient, SshError> {
        let (sender, receiver) = oneshot::channel();
        self.control
            .send(Control::OpenSftp(sender))
            .await
            .map_err(|_| SshError::SessionClosed)?;
        match timeout(SFTP_OPEN_TIMEOUT, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(SshError::SessionClosed),
            Err(_) => Err(SshError::Timeout),
        }
    }

    /// Acquitte des octets de sortie affichés par l'appelant.
    pub fn ack(&self, bytes: usize) {
        self.flow.ack(bytes);
    }

    /// Transmet la décision de l'utilisateur sur une clé d'hôte inconnue.
    pub fn decide_host_key(&self, accept: bool) -> Result<(), SshError> {
        let sender = self
            .decision
            .lock()
            .map_err(|_| SshError::NoPendingDecision)?
            .take()
            .ok_or(SshError::NoPendingDecision)?;
        sender.send(accept).map_err(|_| SshError::NoPendingDecision)
    }

    /// Transmet les réponses de l'utilisateur à la demande keyboard-interactive `round` en
    /// attente, ou l'annule (`None`). Une réponse destinée à un autre tour (en retard, en
    /// double) est refusée ; il faut exactement une réponse par question. Dans ces deux cas,
    /// la demande reste en attente et l'appel échoue.
    pub fn answer_prompts(
        &self,
        round: u32,
        answers: Option<PromptAnswers>,
    ) -> Result<(), SshError> {
        let mut slot = self.prompt.lock().map_err(|_| SshError::NoPendingPrompt)?;
        let pending = slot.take().ok_or(SshError::NoPendingPrompt)?;
        if pending.round != round {
            *slot = Some(pending);
            return Err(SshError::NoPendingPrompt);
        }
        if let Some(answers) = &answers
            && answers.len() != pending.expected
        {
            *slot = Some(pending);
            return Err(SshError::InvalidAnswers);
        }
        pending
            .sender
            .send(answers)
            .map_err(|_| SshError::NoPendingPrompt)
    }

    /// Ferme la session (sans effet si elle l'est déjà).
    pub fn close(&self) {
        self.cancel.cancel();
    }

    /// Vrai si la session a été fermée par l'appelant.
    pub fn is_closed(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Paramètres communs à une session et à un déploiement.
struct Common {
    target: Target,
    credentials: Credentials,
    known_hosts: Arc<KnownHosts>,
    timeouts: Timeouts,
}

/// Issue d'une méthode d'authentification.
enum AuthStep {
    Success,
    /// Échec (ou succès partiel) : méthodes que le serveur propose de poursuivre.
    Failure(Vec<MethodKind>),
}

impl From<client::AuthResult> for AuthStep {
    fn from(result: client::AuthResult) -> Self {
        match result {
            client::AuthResult::Success => Self::Success,
            client::AuthResult::Failure {
                remaining_methods, ..
            } => Self::Failure(remaining_methods.to_vec()),
        }
    }
}

/// Temps réseau de l'authentification : chaque échange est borné par `per_call`, et leur
/// total par `remaining`. Le temps de saisie de l'utilisateur n'y est pas compté : un serveur
/// ne peut pas faire durer l'authentification indéfiniment (demandes vides en série…).
struct NetBudget {
    per_call: Duration,
    remaining: Duration,
}

impl NetBudget {
    fn new(timeouts: &Timeouts) -> Self {
        Self {
            per_call: timeouts.auth,
            remaining: timeouts.auth_total,
        }
    }

    async fn run<T>(
        &mut self,
        operation: impl Future<Output = Result<T, russh::Error>>,
    ) -> Result<T, SshError> {
        let allowed = self.per_call.min(self.remaining);
        let start = Instant::now();
        let result = net(allowed, operation).await;
        self.remaining = self.remaining.saturating_sub(start.elapsed());
        result
    }
}

/// Attend une opération réseau de l'authentification, bornée par `limit`.
async fn net<T>(
    limit: Duration,
    operation: impl Future<Output = Result<T, russh::Error>>,
) -> Result<T, SshError> {
    match timeout(limit, operation).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            tracing::debug!(%error, "erreur pendant l'authentification");
            Err(SshError::ConnectionLost)
        }
        Err(_) => Err(SshError::Timeout),
    }
}

struct SessionTask {
    control_rx: mpsc::Receiver<Control>,
    decision: DecisionSlot,
    prompt: PromptSlot,
    flow: Arc<Flow>,
    cancel: CancellationToken,
    sink: Arc<dyn SessionSink>,
}

impl SessionTask {
    async fn run(mut self, common: Common, purpose: Purpose) {
        let target = common.target.clone();
        tracing::debug!(%target, "ouverture d'une session SSH");
        let cancel = self.cancel.clone();
        let reason = tokio::select! {
            biased;
            () = cancel.cancelled() => SshError::SessionClosed,
            reason = self.drive(common, purpose) => reason,
        };
        tracing::debug!(%target, code = reason.code(), "fin de session SSH");
        self.sink.event(SessionEvent::Disconnected { reason });
    }

    /// Connexion et authentification, puis shell interactif ou déploiement. Renvoie la cause
    /// de la fin.
    async fn drive(&mut self, common: Common, purpose: Purpose) -> SshError {
        let limit = common.timeouts.auth;
        let handle = match self.establish(common).await {
            Ok(handle) => Arc::new(handle),
            Err(reason) => return reason,
        };
        match purpose {
            Purpose::Shell(size) => match timeout(limit, open_shell(&handle, size)).await {
                Ok(Ok((writer, reader))) => {
                    self.sink.event(SessionEvent::Connected);
                    let reason = self.io_loop(&handle, writer, reader).await;
                    // Fin du terminal : la connexion est fermée, canaux SFTP compris.
                    let _ = handle.disconnect(Disconnect::ByApplication, "", "").await;
                    reason
                }
                Ok(Err(reason)) => reason,
                Err(_) => SshError::Timeout,
            },
            Purpose::Deploy(key) => {
                let result = deploy::run(&handle, &key, limit).await;
                let _ = handle.disconnect(Disconnect::ByApplication, "", "").await;
                match result {
                    Ok(outcome) => {
                        self.sink.event(SessionEvent::KeyDeployed {
                            added: outcome == DeployOutcome::Added,
                        });
                        SshError::SessionEnded
                    }
                    Err(reason) => reason,
                }
            }
        }
    }

    /// Connexion, clé d'hôte et authentification.
    async fn establish(&mut self, common: Common) -> Result<Handle<ClientHandler>, SshError> {
        let Common {
            target,
            credentials,
            known_hosts,
            timeouts,
        } = common;

        let stream = timeout(timeouts.connect, connect_tcp(target.host(), target.port()))
            .await
            .map_err(|_| SshError::Timeout)??;

        let verdict = Arc::new(Mutex::new(None));
        let handler = ClientHandler {
            flow: self.flow.clone(),
            banner_budget: MAX_BANNER_TOTAL,
            known_hosts,
            host: target.host().to_owned(),
            port: target.port(),
            sink: self.sink.clone(),
            decision: self.decision.clone(),
            decision_timeout: timeouts.host_key_decision,
            verdict: verdict.clone(),
            refusal_logged: false,
        };
        let config = client_config(&timeouts);
        let handshake = client::connect_stream(config, stream, handler);
        let take_verdict = || verdict.lock().ok().and_then(|mut v| v.take());
        let mut handle =
            match timeout(timeouts.handshake + timeouts.host_key_decision, handshake).await {
                Ok(Ok(handle)) => handle,
                Ok(Err(error)) => {
                    return Err(take_verdict().unwrap_or_else(|| {
                        tracing::debug!(%error, "échec de la poignée de main SSH");
                        SshError::HandshakeFailed
                    }));
                }
                Err(_) => return Err(take_verdict().unwrap_or(SshError::Timeout)),
            };

        self.sink.event(SessionEvent::Authenticating);
        let username = target.username().to_owned();
        self.authenticate(&mut handle, username, credentials, &timeouts)
            .await?;
        Ok(handle)
    }

    /// Authentification : la méthode `none` révèle les méthodes acceptées, puis la clé, le
    /// mot de passe et enfin keyboard-interactive sont essayés selon ce que le serveur
    /// propose, succès partiels compris (`AuthenticationMethods publickey,keyboard-interactive`).
    /// Chaque moyen n'est utilisé qu'une fois ; les secrets sont détruits à la sortie.
    async fn authenticate(
        &self,
        handle: &mut Handle<ClientHandler>,
        username: String,
        credentials: Credentials,
        timeouts: &Timeouts,
    ) -> Result<(), SshError> {
        let (mut key, mut password) = match credentials {
            Credentials::Password(password) => (None, Some(password)),
            Credentials::PrivateKey(key) => (Some(key), None),
            Credentials::KeyThenPassword(key, password) => (Some(key), Some(password)),
            Credentials::Interactive => (None, None),
        };
        let mut budget = NetBudget::new(timeouts);
        let first = budget
            .run(handle.authenticate_none(username.clone()))
            .await?;
        let mut remaining = match AuthStep::from(first) {
            AuthStep::Success => return Ok(()),
            AuthStep::Failure(methods) => methods,
        };
        let mut interactive_tried = false;
        loop {
            let step = if remaining.contains(&MethodKind::PublicKey)
                && let Some(key) = key.take()
            {
                key_auth(handle, username.clone(), key, &mut budget).await?
            } else if remaining.contains(&MethodKind::Password)
                && let Some(password) = password.take()
            {
                // Mot de passe consommé : refusé ici, il ne sera pas resservi plus loin.
                let result = budget
                    .run(handle.authenticate_password(username.clone(), password.expose_secret()))
                    .await?;
                AuthStep::from(result)
            } else if remaining.contains(&MethodKind::KeyboardInteractive) && !interactive_tried {
                interactive_tried = true;
                self.keyboard_interactive(
                    handle,
                    username.clone(),
                    &mut password,
                    &mut budget,
                    timeouts.auth_prompt,
                )
                .await?
            } else {
                return Err(SshError::AuthFailed);
            };
            match step {
                AuthStep::Success => return Ok(()),
                AuthStep::Failure(methods) => remaining = methods,
            }
        }
    }

    /// Authentification keyboard-interactive (AUTH-03). Le mot de passe connu, s'il n'a pas
    /// encore servi, répond une seule fois à une question « Password: » sans écho ; toute
    /// autre question est posée à l'utilisateur.
    async fn keyboard_interactive(
        &self,
        handle: &mut Handle<ClientHandler>,
        username: String,
        password: &mut Option<SecretString>,
        budget: &mut NetBudget,
        answer_limit: Duration,
    ) -> Result<AuthStep, SshError> {
        let mut response = budget
            .run(handle.authenticate_keyboard_interactive_start(username, None::<String>))
            .await?;
        let mut rounds: u32 = 0;
        loop {
            let (name, instructions, prompts) = match response {
                KeyboardInteractiveAuthResponse::Success => return Ok(AuthStep::Success),
                KeyboardInteractiveAuthResponse::Failure {
                    remaining_methods, ..
                } => return Ok(AuthStep::Failure(remaining_methods.to_vec())),
                KeyboardInteractiveAuthResponse::InfoRequest {
                    name,
                    instructions,
                    prompts,
                } => (name, instructions, prompts),
            };
            rounds += 1;
            if rounds as usize > MAX_PROMPT_ROUNDS {
                return Err(SshError::AuthPromptInvalid);
            }
            let prompt = AuthPrompt::from_server(
                &name,
                &instructions,
                prompts.into_iter().map(|p| (p.prompt, p.echo)),
            )?;
            let answers = if prompt.fields.is_empty() {
                PromptAnswers::new(Vec::new())?
            } else if prompt.is_single_password_question()
                && let Some(password) = password.take()
            {
                tracing::debug!("question « mot de passe » : mot de passe enregistré envoyé");
                PromptAnswers::new(vec![password])?
            } else {
                self.ask_user(rounds, prompt, answer_limit).await?
            };
            response = budget
                .run(handle.authenticate_keyboard_interactive_respond(answers.to_russh()))
                .await?;
        }
    }

    /// Pose les questions du serveur à l'utilisateur et attend ses réponses.
    async fn ask_user(
        &self,
        round: u32,
        prompt: AuthPrompt,
        limit: Duration,
    ) -> Result<PromptAnswers, SshError> {
        let (sender, receiver) = oneshot::channel();
        match self.prompt.lock() {
            Ok(mut slot) => {
                *slot = Some(PendingPrompt {
                    round,
                    expected: prompt.fields.len(),
                    sender,
                });
            }
            Err(_) => return Err(SshError::AuthFailed),
        }
        tracing::debug!(questions = prompt.fields.len(), "questions du serveur");
        self.sink.event(SessionEvent::AuthPrompt { round, prompt });
        let answers = match timeout(limit, receiver).await {
            Ok(Ok(Some(answers))) => answers,
            Ok(Ok(None) | Err(_)) => return Err(SshError::AuthCancelled),
            Err(_) => {
                if let Ok(mut slot) = self.prompt.lock() {
                    slot.take();
                }
                return Err(SshError::AuthPromptTimeout);
            }
        };
        self.sink.event(SessionEvent::Authenticating);
        Ok(answers)
    }

    /// Boucle d'entrée/sortie du shell. Renvoie la cause de la fin de session.
    async fn io_loop(
        &mut self,
        handle: &Arc<Handle<ClientHandler>>,
        writer: ChannelWriteHalf<client::Msg>,
        mut reader: russh::ChannelReadHalf,
    ) -> SshError {
        let mut pending: Vec<u8> = Vec::new();
        let mut deadline: Option<Instant> = None;
        let mut ended_by_server = false;

        let reason = loop {
            // Le lot en attente compte aussi : la sortie retenue ne dépasse jamais le seuil
            // de plus d'un paquet SSH.
            let can_read = self.flow.unacked() + pending.len() < HIGH_WATERMARK;
            tokio::select! {
                biased;
                () = self.cancel.cancelled() => {
                    let _ = writer.close().await;
                    let _ = handle.disconnect(Disconnect::ByApplication, "", "").await;
                    break SshError::SessionClosed;
                }
                () = sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => {
                    self.flush(&mut pending);
                    deadline = None;
                }
                control = self.control_rx.recv() => match control {
                    Some(Control::Input(data)) => {
                        if writer.data(&data[..]).await.is_err() {
                            break SshError::ConnectionLost;
                        }
                    }
                    Some(Control::Resize(size)) => {
                        if writer.window_change(size.cols(), size.rows(), 0, 0).await.is_err() {
                            break SshError::ConnectionLost;
                        }
                    }
                    Some(Control::OpenSftp(reply)) => {
                        // Ouverture dans une tâche à part : le terminal reste fluide.
                        let handle = handle.clone();
                        tokio::spawn(async move {
                            let _ = reply.send(open_sftp(&handle).await);
                        });
                    }
                    // Le pilote a disparu : on ferme comme une annulation.
                    None => self.cancel.cancel(),
                },
                () = self.flow.acked.notified(), if !can_read => {}
                message = reader.wait(), if can_read => match message {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        pending.extend_from_slice(&data);
                        if pending.len() >= BATCH_MAX {
                            self.flush(&mut pending);
                            deadline = None;
                        } else if deadline.is_none() {
                            deadline = Some(Instant::now() + BATCH_DELAY);
                        }
                    }
                    Some(ChannelMsg::ExitStatus { .. } | ChannelMsg::ExitSignal { .. } | ChannelMsg::Close) => {
                        ended_by_server = true;
                    }
                    Some(_) => {}
                    // Canal fermé : fin normale si le serveur l'a annoncée, coupure sinon
                    // (keepalives sans réponse, TCP rompu, serveur arrêté).
                    None => break if ended_by_server { SshError::SessionEnded } else { SshError::ConnectionLost },
                },
            }
        };
        self.flush(&mut pending);
        reason
    }

    fn flush(&self, pending: &mut Vec<u8>) {
        if pending.is_empty() {
            return;
        }
        let batch = std::mem::take(pending);
        self.flow.sent(batch.len());
        self.sink.output(batch);
    }
}

/// Résolution DNS puis connexion TCP à la première adresse joignable.
async fn connect_tcp(host: &str, port: u16) -> Result<TcpStream, SshError> {
    let addrs: Vec<_> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| {
            tracing::debug!(%error, "résolution DNS impossible");
            SshError::DnsFailed
        })?
        .collect();
    if addrs.is_empty() {
        return Err(SshError::DnsFailed);
    }
    let mut last = SshError::ConnectFailed;
    for addr in addrs {
        match TcpStream::connect(addr).await {
            Ok(stream) => {
                // Terminal interactif : chaque frappe part immédiatement.
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Err(error) => {
                tracing::debug!(%error, "connexion TCP impossible");
                last = if error.kind() == std::io::ErrorKind::ConnectionRefused {
                    SshError::ConnectionRefused
                } else {
                    SshError::ConnectFailed
                };
            }
        }
    }
    Err(last)
}

async fn key_auth(
    handle: &mut Handle<ClientHandler>,
    username: String,
    key: Arc<PrivateKey>,
    budget: &mut NetBudget,
) -> Result<AuthStep, SshError> {
    // Signature RSA toujours en SHA-2, même si le serveur n'annonce rien : jamais SHA-1.
    let hash = budget
        .run(handle.best_supported_rsa_hash())
        .await?
        .flatten()
        .or(Some(HashAlg::Sha512));
    let key = PrivateKeyWithHashAlg::new(key, hash);
    let result = budget
        .run(handle.authenticate_publickey(username, key))
        .await?;
    Ok(AuthStep::from(result))
}

/// Locale proposée au shell distant (variables `LANG` et `LC_CTYPE`).
const SHELL_LOCALE: &str = "C.UTF-8";

async fn open_shell(
    handle: &Handle<ClientHandler>,
    size: TermSize,
) -> Result<(ChannelWriteHalf<client::Msg>, russh::ChannelReadHalf), SshError> {
    let failed = |error: russh::Error| {
        tracing::debug!(%error, "ouverture du shell impossible");
        SshError::ChannelFailed
    };
    let channel = handle.channel_open_session().await.map_err(failed)?;
    // Locale UTF-8 proposée au serveur, comme le `SendEnv LANG LC_*` d'OpenSSH : sans elle, un
    // serveur sans locale démarre bash en « C », et readline convertit les lettres accentuées
    // tapées au clavier en séquences « Méta » (rien ne s'affiche, ou « (arg: 9) » pour « ù »).
    // `LC_CTYPE` règle seul le traitement des caractères et l'emporte sur `LANG`, que bien des
    // serveurs réécrivent en `LANG=C` après réception (conteneurs Proxmox) ; `LANG` couvre ceux
    // qui n'acceptent que lui. `C.UTF-8` ne révèle rien du poste (pas de langue) ; le serveur
    // peut refuser ces variables (`AcceptEnv`) ou les redéfinir.
    for name in ["LANG", "LC_CTYPE"] {
        channel
            .set_env(false, name, SHELL_LOCALE)
            .await
            .map_err(failed)?;
    }
    channel
        .request_pty(false, TERM, size.cols(), size.rows(), 0, 0, &[])
        .await
        .map_err(failed)?;
    channel.request_shell(false).await.map_err(failed)?;
    let (reader, writer) = channel.split();
    Ok((writer, reader))
}

/// Ouvre deux canaux `sftp` sur la connexion et initialise le client.
async fn open_sftp(handle: &Handle<ClientHandler>) -> Result<SftpClient, SshError> {
    let open = || async {
        let channel = handle.channel_open_session().await.map_err(|error| {
            tracing::debug!(%error, "ouverture d'un canal SFTP impossible");
            SshError::SftpUnavailable
        })?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|error| {
                tracing::debug!(%error, "sous-système SFTP refusé");
                SshError::SftpUnavailable
            })?;
        Ok::<_, SshError>(channel.into_stream())
    };
    let meta = open().await?;
    let files = open().await?;
    SftpClient::from_streams(meta, files).await
}

/// Gestionnaire russh : vérification de la clé d'hôte, bannière, et refus de tout canal ouvert
/// à l'initiative du serveur.
struct ClientHandler {
    flow: Arc<Flow>,
    /// Octets de bannière encore affichables pour cette session.
    banner_budget: usize,
    known_hosts: Arc<KnownHosts>,
    host: String,
    port: u16,
    sink: Arc<dyn SessionSink>,
    decision: DecisionSlot,
    decision_timeout: Duration,
    /// Cause précise d'un refus, russh ne renvoyant qu'une erreur générique.
    verdict: Arc<Mutex<Option<SshError>>>,
    /// Refus d'un canal ouvert par le serveur déjà signalé dans les journaux : une ligne par
    /// connexion, pour qu'un serveur malveillant ne puisse pas les remplir.
    refusal_logged: bool,
}

impl ClientHandler {
    /// Refuse un canal ouvert par le serveur. Younji n'en demande aucun (ni redirection, ni X11,
    /// ni transfert d'agent) ; russh les accepte par défaut et ne ferme jamais ceux qu'on
    /// abandonne.
    async fn refuse_channel(&mut self, kind: &'static str, reply: client::ChannelOpenHandle) {
        if !self.refusal_logged {
            self.refusal_logged = true;
            tracing::warn!(kind, "canal ouvert par le serveur sans demande : refusé");
        }
        reply
            .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
            .await;
    }

    fn refuse(&self, reason: SshError) -> Result<bool, russh::Error> {
        if let Ok(mut verdict) = self.verdict.lock() {
            verdict.get_or_insert(reason);
        }
        Ok(false)
    }

    async fn ask_user(&self, key: &PublicKey) -> Result<bool, russh::Error> {
        let (sender, receiver) = oneshot::channel();
        match self.decision.lock() {
            Ok(mut slot) => *slot = Some(sender),
            Err(_) => return self.refuse(SshError::HostKeyRejected),
        }
        self.sink.event(SessionEvent::HostKeyUnknown {
            key: KeyInfo::of(key),
        });
        let accepted = match timeout(self.decision_timeout, receiver).await {
            Ok(Ok(accepted)) => accepted,
            Ok(Err(_)) => false,
            Err(_) => {
                if let Ok(mut slot) = self.decision.lock() {
                    slot.take();
                }
                return self.refuse(SshError::HostKeyTimeout);
            }
        };
        if !accepted {
            return self.refuse(SshError::HostKeyRejected);
        }
        // Enregistrer ici est sûr : russh vérifie la signature du serveur sur l'échange de clés
        // avant d'appeler `check_server_key` (documentation du trait `client::Handler`). Le
        // serveur a donc prouvé qu'il possède la clé privée de la clé enregistrée.
        match self.known_hosts.add(&self.host, self.port, key) {
            Ok(()) => {
                tracing::info!("nouvelle clé d'hôte enregistrée");
                Ok(true)
            }
            Err(error) => self.refuse(error),
        }
    }
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let russh::keys::PublicKeyOrCertificate::PublicKey { key, .. } = server_public_key else {
            // Les certificats d'hôte ne sont pas annoncés (hors périmètre) : refus.
            return self.refuse(SshError::HostKeyRejected);
        };
        match self.known_hosts.check(&self.host, self.port, key) {
            Ok(HostKeyStatus::Trusted) => Ok(true),
            Ok(HostKeyStatus::Unknown) => self.ask_user(key).await,
            Ok(HostKeyStatus::Changed { known }) => {
                tracing::warn!("clé d'hôte modifiée : connexion refusée");
                self.sink.event(SessionEvent::HostKeyChanged {
                    key: KeyInfo::of(key),
                    known,
                });
                self.refuse(SshError::HostKeyChanged)
            }
            Ok(HostKeyStatus::Revoked) => {
                tracing::warn!("clé d'hôte révoquée : connexion refusée");
                self.refuse(SshError::HostKeyRevoked)
            }
            Err(error) => self.refuse(error),
        }
    }

    // Canaux ouverts à l'initiative du serveur : tous refusés (SECURITE.md §4.1).

    /// Types de canal inconnus : refus explicite, sans dépendre de la valeur par défaut de russh
    /// (qui pourrait changer d'une version à l'autre).
    async fn should_accept_unknown_server_channel(
        &mut self,
        _id: russh::ChannelId,
        _channel_type: &str,
    ) -> bool {
        if !self.refusal_logged {
            self.refusal_logged = true;
            tracing::warn!(
                kind = "inconnu",
                "canal ouvert par le serveur sans demande : refusé"
            );
        }
        false
    }

    async fn server_channel_open_session(
        &mut self,
        _channel: russh::Channel<client::Msg>,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.refuse_channel("session", reply).await;
        Ok(())
    }

    async fn server_channel_open_direct_tcpip(
        &mut self,
        _channel: russh::Channel<client::Msg>,
        _host_to_connect: &str,
        _port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.refuse_channel("direct-tcpip", reply).await;
        Ok(())
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        _channel: russh::Channel<client::Msg>,
        _connected_address: &str,
        _connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.refuse_channel("forwarded-tcpip", reply).await;
        Ok(())
    }

    async fn server_channel_open_x11(
        &mut self,
        _channel: russh::Channel<client::Msg>,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.refuse_channel("x11", reply).await;
        Ok(())
    }

    async fn server_channel_open_agent_forward(
        &mut self,
        _channel: russh::Channel<client::Msg>,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.refuse_channel("auth-agent", reply).await;
        Ok(())
    }

    async fn server_channel_open_forwarded_streamlocal(
        &mut self,
        _channel: russh::Channel<client::Msg>,
        _socket_path: &str,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.refuse_channel("forwarded-streamlocal", reply).await;
        Ok(())
    }

    async fn server_channel_open_direct_streamlocal(
        &mut self,
        _channel: russh::Channel<client::Msg>,
        _socket_path: &str,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.refuse_channel("direct-streamlocal", reply).await;
        Ok(())
    }

    async fn auth_banner(
        &mut self,
        banner: &str,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let text = sanitize_banner(banner);
        if text.is_empty() || text.len() > self.banner_budget {
            return Ok(());
        }
        self.banner_budget -= text.len();
        // Comptée comme le reste de la sortie : l'acquittement de l'interface s'équilibre.
        self.flow.sent(text.len());
        self.sink.output(text.into_bytes());
        Ok(())
    }
}

/// Bannière affichée comme du texte brut : toute séquence de contrôle (échappements ANSI,
/// OSC…) est retirée pour qu'un serveur ne puisse pas piloter le terminal avant
/// l'authentification. Les fins de ligne sont normalisées en `\r\n`.
pub(crate) fn sanitize_banner(banner: &str) -> String {
    let mut out = String::new();
    for c in banner.chars() {
        if out.len() >= MAX_BANNER_LEN {
            break;
        }
        match c {
            '\n' => out.push_str("\r\n"),
            '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_is_stripped_of_control_sequences() {
        let banner = "Bienvenue\x1b[31m rouge\x1b]52;c;ZXZpbA==\x07\r\nfin\t!\n";
        assert_eq!(
            sanitize_banner(banner),
            "Bienvenue[31m rouge]52;c;ZXZpbA==\r\nfin\t!\r\n"
        );
    }

    #[test]
    fn banner_is_bounded() {
        let banner = "a".repeat(MAX_BANNER_LEN * 2);
        assert!(sanitize_banner(&banner).len() <= MAX_BANNER_LEN);
    }

    #[test]
    fn flow_ack_saturates() {
        let flow = Flow::default();
        flow.sent(100);
        flow.ack(40);
        assert_eq!(flow.unacked(), 60);
        flow.ack(1_000);
        assert_eq!(flow.unacked(), 0);
    }
}
