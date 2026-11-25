//! Erreurs de la crate. Chaque variante correspond à un code stable (`term.*`), traduit côté
//! interface par la clé `errors.<code>`. Aucune variante ne transporte de contenu du terminal.

/// Erreur du terminal local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TermError {
    /// Profil inconnu (absent de la détection).
    #[error("profil inconnu")]
    UnknownProfile,
    /// Taille de terminal hors bornes.
    #[error("taille de terminal invalide")]
    InvalidSize,
    /// Données à envoyer trop volumineuses pour un seul appel.
    #[error("données trop volumineuses")]
    InputTooLarge,
    /// Le PTY ou le processus n'a pas pu être créé.
    #[error("lancement du shell impossible")]
    SpawnFailed,
    /// La session est terminée.
    #[error("session terminée")]
    Closed,
}

impl TermError {
    /// Toutes les variantes (test des traductions).
    pub const ALL: [Self; 5] = [
        Self::UnknownProfile,
        Self::InvalidSize,
        Self::InputTooLarge,
        Self::SpawnFailed,
        Self::Closed,
    ];

    /// Code stable, clé i18n `errors.<code>`.
    pub fn code(self) -> &'static str {
        match self {
            Self::UnknownProfile => "term.unknown_profile",
            Self::InvalidSize => "term.invalid_size",
            Self::InputTooLarge => "term.input_too_large",
            Self::SpawnFailed => "term.spawn_failed",
            Self::Closed => "term.closed",
        }
    }
}
