//! Client SSH de Younji (ADR-002, ADR-004) : connexion, authentification, PTY distant et
//! vérification des clés d'hôte, keyboard-interactive (2FA), génération et déploiement de clés,
//! SFTP (opérations, transferts, édition).
//! Cette crate ne dépend pas de Tauri.

pub mod auth;
pub mod config;
pub mod deploy;
mod error;
pub mod keygen;
pub mod known_hosts;
pub mod prompt;
mod session;
pub mod sftp;
mod target;
pub mod tunnel;

pub use auth::{Credentials, ImportedKey};
pub use config::Timeouts;
pub use deploy::{AuthorizedKeyLine, DeployOutcome};
pub use error::SshError;
pub use keygen::{KeyKind, generate_key};
pub use known_hosts::{HostKeyStatus, KeyInfo, KnownHostEntry, KnownHosts, KnownHostsStorage};
pub use prompt::{AuthPrompt, PromptAnswers, PromptField};
pub use session::{
    ConnectParams, DeployParams, HIGH_WATERMARK, MAX_INPUT_CHUNK, SessionEvent, SessionHandle,
    SessionSink, TunnelParams,
};
pub use target::{MAX_COLS, MAX_ROWS, Target, TermSize};
pub use tunnel::{ListenScope, TunnelSpec};
