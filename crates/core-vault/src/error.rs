//! Erreurs du coffre. Chaque variante correspond à un code stable (`vault.*`), traduit côté
//! interface par la clé `errors.<code>`.
//!
//! Règle (SECURITE.md §3.3 et §6) : aucune variante ne transporte de secret ni d'entrée
//! utilisateur. Toute erreur de déchiffrement donne **le même** code
//! (`vault.wrong_password_or_corrupted`), sans distinguer un mauvais mot de passe d'un fichier
//! altéré.

/// Erreur du coffre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VaultError {
    /// Aucun coffre n'existe encore à cet emplacement.
    #[error("coffre absent")]
    NotFound,
    /// Un coffre existe déjà : la création n'écrase jamais un fichier.
    #[error("coffre déjà existant")]
    AlreadyExists,
    /// Le coffre est absent mais sa copie de sauvegarde existe : à restaurer avant tout.
    #[error("seule la copie de sauvegarde du coffre existe")]
    BackupOnly,
    /// Le coffre est verrouillé.
    #[error("coffre verrouillé")]
    Locked,
    /// Mauvais mot de passe **ou** fichier altéré : les deux cas sont volontairement confondus.
    #[error("mot de passe incorrect ou coffre endommagé")]
    WrongPasswordOrCorrupted,
    /// Structure du fichier invalide (magie, taille, longueur annoncée).
    #[error("fichier de coffre invalide")]
    InvalidFile,
    /// Version de format ou de schéma inconnue.
    #[error("version de coffre non prise en charge")]
    UnsupportedVersion,
    /// Paramètres de dérivation hors des bornes admises (plancher ou plafond).
    #[error("paramètres de dérivation refusés")]
    KdfParamsRejected,
    /// Lecture ou écriture du fichier impossible.
    #[error("accès au fichier du coffre impossible")]
    Io,
    /// L'aléa du système est indisponible.
    #[error("aléa du système indisponible")]
    RandomUnavailable,

    /// Mot de passe maître trop court (moins de 12 caractères) ou trop long.
    #[error("mot de passe maître trop court")]
    PasswordTooShort,
    /// Mot de passe maître trop facile à deviner (zxcvbn < 3).
    #[error("mot de passe maître trop faible")]
    PasswordTooWeak,

    /// Nom vide, trop long ou contenant des caractères de contrôle.
    #[error("nom invalide")]
    InvalidName,
    /// Adresse d'hôte invalide.
    #[error("adresse invalide")]
    InvalidAddress,
    /// Port hors de 1–65535.
    #[error("port invalide")]
    InvalidPort,
    /// Nom d'utilisateur invalide.
    #[error("utilisateur invalide")]
    InvalidUsername,
    /// Étiquettes invalides (trop nombreuses, trop longues).
    #[error("étiquettes invalides")]
    InvalidTags,
    /// Notes trop longues ou contenant des caractères de contrôle.
    #[error("notes invalides")]
    InvalidNotes,
    /// Mot de passe d'identité vide ou trop long.
    #[error("secret invalide")]
    InvalidSecret,
    /// Réglages RDP invalides (domaine, résolution).
    #[error("réglages RDP invalides")]
    InvalidRdpSettings,
    /// Une identité doit avoir un mot de passe, une clé, ou les deux.
    #[error("identité sans moyen d'authentification")]
    IdentityIncomplete,
    /// Délai de verrouillage hors des valeurs proposées.
    #[error("délai de verrouillage invalide")]
    InvalidAutoLock,
    /// Trop d'entrées de ce type.
    #[error("trop d'entrées")]
    TooManyEntries,
    /// Entrée introuvable (identifiant inconnu).
    #[error("entrée introuvable")]
    EntryNotFound,
    /// Clé utilisée par au moins une identité : suppression refusée.
    #[error("clé utilisée")]
    KeyInUse,
    /// Identité utilisée par au moins un hôte : suppression refusée.
    #[error("identité utilisée")]
    IdentityInUse,
    /// Snippet vide, trop long, avec des caractères de contrôle ou trop de variables.
    #[error("snippet invalide")]
    InvalidSnippet,
    /// Données de clés d'hôte trop volumineuses.
    #[error("clés d'hôte trop volumineuses")]
    KnownHostsTooLarge,
}

impl VaultError {
    /// Code stable, clé de traduction `errors.<code>` côté interface.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotFound => "vault.not_found",
            Self::AlreadyExists => "vault.already_exists",
            Self::BackupOnly => "vault.backup_only",
            Self::Locked => "vault.locked",
            Self::WrongPasswordOrCorrupted => "vault.wrong_password_or_corrupted",
            Self::InvalidFile => "vault.invalid_file",
            Self::UnsupportedVersion => "vault.unsupported_version",
            Self::KdfParamsRejected => "vault.kdf_params_rejected",
            Self::Io => "vault.io",
            Self::RandomUnavailable => "vault.random_unavailable",
            Self::PasswordTooShort => "vault.password_too_short",
            Self::PasswordTooWeak => "vault.password_too_weak",
            Self::InvalidName => "vault.invalid_name",
            Self::InvalidAddress => "vault.invalid_address",
            Self::InvalidPort => "vault.invalid_port",
            Self::InvalidUsername => "vault.invalid_username",
            Self::InvalidTags => "vault.invalid_tags",
            Self::InvalidNotes => "vault.invalid_notes",
            Self::InvalidSecret => "vault.invalid_secret",
            Self::InvalidRdpSettings => "vault.invalid_rdp",
            Self::IdentityIncomplete => "vault.identity_incomplete",
            Self::InvalidAutoLock => "vault.invalid_auto_lock",
            Self::TooManyEntries => "vault.too_many_entries",
            Self::EntryNotFound => "vault.entry_not_found",
            Self::KeyInUse => "vault.key_in_use",
            Self::IdentityInUse => "vault.identity_in_use",
            Self::KnownHostsTooLarge => "vault.known_hosts_too_large",
            Self::InvalidSnippet => "vault.invalid_snippet",
        }
    }

    /// Toutes les variantes, pour vérifier que chaque code est traduit.
    pub const ALL: &'static [Self] = &[
        Self::NotFound,
        Self::AlreadyExists,
        Self::BackupOnly,
        Self::Locked,
        Self::WrongPasswordOrCorrupted,
        Self::InvalidFile,
        Self::UnsupportedVersion,
        Self::KdfParamsRejected,
        Self::Io,
        Self::RandomUnavailable,
        Self::PasswordTooShort,
        Self::PasswordTooWeak,
        Self::InvalidName,
        Self::InvalidAddress,
        Self::InvalidPort,
        Self::InvalidUsername,
        Self::InvalidTags,
        Self::InvalidNotes,
        Self::InvalidSecret,
        Self::InvalidRdpSettings,
        Self::IdentityIncomplete,
        Self::InvalidAutoLock,
        Self::TooManyEntries,
        Self::EntryNotFound,
        Self::KeyInUse,
        Self::IdentityInUse,
        Self::KnownHostsTooLarge,
        Self::InvalidSnippet,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_namespaced() {
        let mut codes: Vec<_> = VaultError::ALL.iter().map(|e| e.code()).collect();
        assert!(codes.iter().all(|c| c.starts_with("vault.")));
        let len = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), len);
    }
}
