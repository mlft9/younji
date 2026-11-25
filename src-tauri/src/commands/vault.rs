//! Commandes du coffre (VLT-01 à VLT-04) : création, déverrouillage, verrouillage, mot de passe
//! maître, activité et verrouillage automatique. Les mots de passe n'arrivent qu'ici, une fois,
//! et ne repartent jamais vers l'interface.

use secrecy::SecretString;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::error::AppError;
use crate::state::SessionManager;
use crate::vault_state::{LockReason, VaultEventDto, VaultState, VaultStatus};

/// Verrouille le coffre et ferme toutes les sessions SSH (ADR-026) : un poste laissé sans
/// surveillance ne doit exposer ni les secrets du coffre ni un shell ouvert.
///
/// Ordre volontaire : le coffre d'abord, les sessions ensuite. Une connexion en cours de
/// création vérifie, une fois sa session enregistrée, que le coffre est toujours déverrouillé
/// (`spawn_session`) : aucune session ne peut donc survivre au verrouillage.
pub fn lock_everything(app: &AppHandle, reason: LockReason) {
    app.state::<VaultState>().lock(reason);
    app.state::<SessionManager>().close_all_sessions();
    app.state::<crate::term_state::LocalTerminals>().close_all();
    app.state::<crate::commands::rdp::RdpClients>()
        .terminate_all();
    app.state::<crate::commands::import::ImportState>().clear();
}

/// État du coffre : absent, verrouillé ou déverrouillé.
#[tauri::command]
pub fn vault_status(state: State<'_, VaultState>) -> Result<VaultStatus, AppError> {
    state.status()
}

/// Crée le coffre avec le mot de passe maître (politique vérifiée ici, pas seulement dans
/// l'interface) et le laisse déverrouillé.
#[tauri::command]
pub async fn vault_create(
    password: SecretString,
    state: State<'_, VaultState>,
) -> Result<(), AppError> {
    state.create(password).await
}

/// Déverrouille le coffre. Un échec renvoie toujours le même code, sans indice.
#[tauri::command]
pub async fn vault_unlock(
    password: SecretString,
    state: State<'_, VaultState>,
) -> Result<(), AppError> {
    state.unlock(password).await
}

/// Verrouille le coffre (et ferme les sessions).
#[tauri::command]
pub fn vault_lock(app: AppHandle) {
    lock_everything(&app, LockReason::Manual);
}

/// Change le mot de passe maître : l'actuel est exigé, le nouveau doit respecter la politique.
#[tauri::command]
pub async fn vault_change_password(
    current: SecretString,
    new: SecretString,
    state: State<'_, VaultState>,
) -> Result<(), AppError> {
    state.change_password(current, new).await
}

/// Signale une activité de l'utilisateur (clavier, souris) : remet le minuteur à zéro.
#[tauri::command]
pub fn vault_touch(state: State<'_, VaultState>) {
    state.touch();
}

/// Enregistre le `Channel` qui recevra les événements du coffre (verrouillage automatique).
#[tauri::command]
pub fn vault_watch(
    on_event: Channel<VaultEventDto>,
    state: State<'_, VaultState>,
) -> Result<(), AppError> {
    state.watch(on_event)
}

/// Change le délai de verrouillage automatique (valeurs proposées uniquement).
#[tauri::command]
pub async fn vault_set_auto_lock(
    minutes: u32,
    state: State<'_, VaultState>,
) -> Result<(), AppError> {
    state
        .update_blocking(move |data| data.set_auto_lock(minutes))
        .await?;
    state.touch();
    Ok(())
}
