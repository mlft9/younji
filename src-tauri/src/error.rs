//! Erreurs renvoyées à l'interface.
//!
//! Une commande ne renvoie jamais de message libre : seulement un code stable, qui correspond
//! à une clé de traduction `errors.<code>` côté interface. Le détail technique part dans les
//! journaux, sans secret (SECURITE.md §6).

use core_ssh::SshError;
use core_vault::VaultError;
use serde::Serialize;

/// Erreur sérialisable à destination de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AppError {
    /// Code stable, par exemple `ssh.auth_failed`.
    code: &'static str,
}

impl AppError {
    /// Session inconnue (fermée ou identifiant invalide).
    pub const SESSION_NOT_FOUND: Self = Self::new("ssh.session_not_found");
    /// Trop de sessions ouvertes en même temps.
    pub const TOO_MANY_SESSIONS: Self = Self::new("ssh.too_many_sessions");
    /// Entrée mal formée (par exemple des octets bruts hors de 0–255).
    pub const INVALID_INPUT: Self = Self::new("ssh.invalid_input");
    /// Fichier de clé non sélectionné via la boîte de dialogue (identifiant inconnu).
    pub const KEY_FILE_UNKNOWN: Self = Self::new("ssh.key_file_unknown");
    /// Boîte de dialogue de sélection indisponible.
    pub const DIALOG_FAILED: Self = Self::new("dialog.failed");
    /// Adresse de lien refusée (schéma, forme ou longueur).
    pub const URL_INVALID: Self = Self::new("url.invalid");
    /// Le navigateur du système n'a pas pu être lancé.
    pub const URL_OPEN_FAILED: Self = Self::new("url.open_failed");
    /// Démarrage d'un tunnel exposé refusé dans la confirmation native.
    pub const TUNNEL_EXPOSURE_REFUSED: Self = Self::new("tunnels.exposure_refused");
    /// Une confirmation d'exposition de tunnel est déjà ouverte.
    pub const TUNNEL_CONFIRMATION_PENDING: Self = Self::new("tunnels.confirmation_pending");
    /// Erreur interne inattendue.
    pub const INTERNAL: Self = Self::new("internal");
    /// Coffre verrouillé (même code que `VaultError::Locked`).
    pub const VAULT_LOCKED: Self = Self::new("vault.locked");
    /// Hôte sans identité : la connexion directe depuis le carnet est impossible.
    pub const HOST_WITHOUT_IDENTITY: Self = Self::new("book.no_identity");
    /// Paramètres hors des valeurs admises.
    pub const SETTINGS_INVALID: Self = Self::new("settings.invalid");
    /// `settings.json` ne peut pas être écrit.
    pub const SETTINGS_IO: Self = Self::new("settings.io");
    /// Écriture du fichier exporté impossible.
    pub const EXPORT_FAILED: Self = Self::new("keys.export_failed");
    /// Client SFTP inconnu (fermé, session terminée ou identifiant invalide).
    pub const SFTP_NOT_FOUND: Self = Self::new("sftp.not_found");
    /// Trop de clients SFTP ouverts.
    pub const TOO_MANY_SFTP: Self = Self::new("sftp.too_many");
    /// Dossier local inconnu (non choisi dans la boîte de dialogue, ou oublié au verrouillage).
    pub const LOCAL_ROOT_UNKNOWN: Self = Self::new("local.root_unknown");
    /// Édition inconnue (terminée ou identifiant invalide).
    pub const EDIT_NOT_FOUND: Self = Self::new("sftp.edit_not_found");
    /// Trop d'éditions simultanées.
    pub const TOO_MANY_EDITS: Self = Self::new("sftp.too_many_edits");
    /// L'éditeur de texte n'a pas pu être lancé.
    pub const EDITOR_FAILED: Self = Self::new("editor.open_failed");

    /// Codes propres à `src-tauri` (hors `SshError`), pour le test des traductions.
    #[cfg(test)]
    pub const OWN_CODES: &'static [Self] = &[
        Self::SESSION_NOT_FOUND,
        Self::TOO_MANY_SESSIONS,
        Self::INVALID_INPUT,
        Self::KEY_FILE_UNKNOWN,
        Self::DIALOG_FAILED,
        Self::URL_INVALID,
        Self::URL_OPEN_FAILED,
        Self::INTERNAL,
        Self::VAULT_LOCKED,
        Self::HOST_WITHOUT_IDENTITY,
        Self::SETTINGS_INVALID,
        Self::SETTINGS_IO,
        Self::EXPORT_FAILED,
        Self::SFTP_NOT_FOUND,
        Self::TOO_MANY_SFTP,
        Self::LOCAL_ROOT_UNKNOWN,
        Self::EDIT_NOT_FOUND,
        Self::TOO_MANY_EDITS,
        Self::EDITOR_FAILED,
        Self::TUNNEL_EXPOSURE_REFUSED,
        Self::TUNNEL_CONFIRMATION_PENDING,
    ];

    /// Crée une erreur à partir d'un code. Le code doit exister dans `src/locales/*.json`.
    pub const fn new(code: &'static str) -> Self {
        Self { code }
    }

    /// Code de l'erreur.
    #[cfg(test)]
    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl From<SshError> for AppError {
    fn from(error: SshError) -> Self {
        Self::new(error.code())
    }
}

impl From<VaultError> for AppError {
    fn from(error: VaultError) -> Self {
        Self::new(error.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_only_the_code() {
        let error = AppError::new("ipc.command_denied");
        assert_eq!(error.code(), "ipc.command_denied");
        let json = serde_json::to_value(error).unwrap();
        assert_eq!(json, serde_json::json!({ "code": "ipc.command_denied" }));
    }

    #[test]
    fn converts_ssh_errors_to_their_code() {
        assert_eq!(
            AppError::from(SshError::AuthFailed).code(),
            "ssh.auth_failed"
        );
    }

    /// Chaque code renvoyé à l'interface a sa traduction en français et en anglais.
    #[test]
    fn every_code_is_translated() {
        let codes: Vec<&str> = SshError::ALL
            .iter()
            .map(|e| e.code())
            .chain(VaultError::ALL.iter().map(|e| e.code()))
            .chain(AppError::OWN_CODES.iter().map(AppError::code))
            .collect();
        for locale in ["fr", "en"] {
            let path = format!(
                "{}/../src/locales/{locale}.json",
                env!("CARGO_MANIFEST_DIR")
            );
            let json: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            for code in &codes {
                let mut node = &json["errors"];
                for part in code.split('.') {
                    node = &node[part];
                }
                assert!(
                    node.is_string(),
                    "{locale} : traduction manquante pour errors.{code}"
                );
            }
        }
    }
}
