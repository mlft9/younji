//! Coffre chiffré de Younji (ADR-004, ADR-005) : format de fichier v1, chiffrement
//! (Argon2id → KEK, DEK aléatoire, XChaCha20-Poly1305, en-tête authentifié), modèle de données
//! et politique du mot de passe maître. Cette crate ne dépend ni de Tauri ni de `core-ssh`.
//!
//! Référence : `docs/SECURITE.md` §3.

pub mod crypto;
mod error;
pub mod format;
pub mod model;
pub mod policy;
mod store;

pub use error::VaultError;
pub use model::{
    AUTO_LOCK_CHOICES, EntryId, Group, Host, HostInput, HostProtocol, Identity, IdentityInput,
    NewKey, PasswordChange, RdpResolution, Snippet, SnippetInput, StoredKey, VaultData,
    VaultSettings, snippet_variables,
};
pub use store::{Vault, VaultFileStatus, backup_path, file_status};
