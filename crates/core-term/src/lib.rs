//! Terminal local de Younji (LOC-01, LOC-02), sans dépendance à Tauri (ADR-004).
//!
//! - [`profiles`] : détection des shells installés. Un profil est désigné par un identifiant
//!   stable ; le programme et ses arguments sont toujours reconstruits ici, jamais fournis par
//!   l'interface.
//! - [`session`] : session PTY (lecture par lots avec contrôle de flux, écriture, taille, fin).

pub mod error;
pub mod profiles;
pub mod session;

pub use error::TermError;
pub use profiles::{LocalProfile, ProfileKind, detect_profiles};
pub use session::{LocalSession, TermEvent, TermSink, TermSize};
