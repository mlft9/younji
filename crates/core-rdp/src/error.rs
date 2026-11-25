//! Erreurs de la crate. Chaque variante correspond à un code stable (`rdp.*`), traduit côté
//! interface par la clé `errors.<code>`. Aucune variante ne transporte de secret ni d'entrée.

/// Erreur du lancement RDP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RdpError {
    /// Adresse invalide.
    #[error("adresse invalide")]
    InvalidAddress,
    /// Port invalide.
    #[error("port invalide")]
    InvalidPort,
    /// Utilisateur invalide (vide, trop long, caractères interdits).
    #[error("utilisateur invalide")]
    InvalidUsername,
    /// Domaine invalide.
    #[error("domaine invalide")]
    InvalidDomain,
    /// Résolution hors bornes.
    #[error("résolution invalide")]
    InvalidResolution,
    /// Mot de passe trop long ou contenant un retour à la ligne.
    #[error("mot de passe inutilisable")]
    InvalidPassword,
    /// Aucun client RDP trouvé (mstsc, xfreerdp3, xfreerdp).
    #[error("client RDP introuvable")]
    ClientNotFound,
    /// Mot de passe requis (FreeRDP ne peut pas le demander sans terminal).
    #[error("mot de passe requis")]
    PasswordRequired,
    /// Écriture de l'identifiant dans le gestionnaire d'identifiants impossible.
    #[error("identifiant temporaire impossible à créer")]
    CredentialFailed,
    /// Fichier temporaire impossible à créer.
    #[error("fichier temporaire impossible à créer")]
    TempFileFailed,
    /// Le client n'a pas pu être lancé.
    #[error("lancement du client impossible")]
    LaunchFailed,
}

impl RdpError {
    /// Toutes les variantes (test des traductions).
    pub const ALL: [Self; 11] = [
        Self::InvalidAddress,
        Self::InvalidPort,
        Self::InvalidUsername,
        Self::InvalidDomain,
        Self::InvalidResolution,
        Self::InvalidPassword,
        Self::ClientNotFound,
        Self::PasswordRequired,
        Self::CredentialFailed,
        Self::TempFileFailed,
        Self::LaunchFailed,
    ];

    /// Code stable, clé i18n `errors.<code>`.
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidAddress => "rdp.invalid_address",
            Self::InvalidPort => "rdp.invalid_port",
            Self::InvalidUsername => "rdp.invalid_username",
            Self::InvalidDomain => "rdp.invalid_domain",
            Self::InvalidResolution => "rdp.invalid_resolution",
            Self::InvalidPassword => "rdp.invalid_password",
            Self::ClientNotFound => "rdp.client_not_found",
            Self::PasswordRequired => "rdp.password_required",
            Self::CredentialFailed => "rdp.credential_failed",
            Self::TempFileFailed => "rdp.temp_file_failed",
            Self::LaunchFailed => "rdp.launch_failed",
        }
    }
}
