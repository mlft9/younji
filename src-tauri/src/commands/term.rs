//! Commandes IPC du terminal local (étape 7). L'interface choisit un profil **par identifiant**
//! dans la liste détectée par le cœur ; aucun programme, argument ni chemin ne vient d'elle.
//! Flux identiques à SSH : octets bruts pour la sortie, JSON pour les événements (ADR-024).

use std::sync::Arc;

use core_term::{LocalSession, TermEvent, TermSink, TermSize, detect_profiles};
use serde::Serialize;
use tauri::State;
use tauri::ipc::{Channel, InvokeResponseBody};

use crate::commands::ssh::SessionEventDto;
use crate::error::AppError;
use crate::term_state::{LocalTerminals, TermId, TermRemover};
use crate::vault_state::VaultState;

/// Longueur maximale d'un identifiant de profil reçu.
const MAX_PROFILE_ID_LEN: usize = 512;

/// Profil affichable (sans le chemin du programme ni ses arguments).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    id: String,
    name: String,
    kind: &'static str,
    is_default: bool,
}

/// Détecte les shells installés et les mémorise comme seuls profils lançables.
#[tauri::command]
pub async fn term_profiles(
    state: State<'_, LocalTerminals>,
    vault: State<'_, VaultState>,
) -> Result<Vec<ProfileDto>, AppError> {
    if !vault.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    // Sous Windows, la détection lance `wsl.exe -l -q` : hors de l'exécuteur async.
    let profiles = tauri::async_runtime::spawn_blocking(detect_profiles)
        .await
        .map_err(|_| AppError::INTERNAL)?;
    let dtos = profiles
        .iter()
        .map(|p| ProfileDto {
            id: p.id.clone(),
            name: p.name.clone(),
            kind: p.kind.as_str(),
            is_default: p.is_default,
        })
        .collect();
    state.set_profiles(profiles)?;
    Ok(dtos)
}

/// Relie un terminal local aux `Channel` de l'interface.
struct ChannelSink {
    events: Channel<SessionEventDto>,
    output: Channel<InvokeResponseBody>,
    remover: TermRemover,
}

impl TermSink for ChannelSink {
    fn output(&self, data: Vec<u8>) {
        if self.output.send(InvokeResponseBody::Raw(data)).is_err() {
            tracing::debug!("sortie du terminal local non délivrée (interface fermée)");
        }
    }

    fn event(&self, event: TermEvent) {
        let TermEvent::Exited { .. } = event;
        let dto = SessionEventDto::Disconnected {
            code: "term.exited",
        };
        if self.events.send(dto).is_err() {
            tracing::debug!("fin du terminal local non délivrée (interface fermée)");
        }
        self.remover.remove();
    }
}

/// Ouvre un terminal local avec un profil de la dernière détection. Le terminal est utilisable
/// dès que l'identifiant est renvoyé ; sa fin arrive par `on_event` (`disconnected`).
#[tauri::command]
pub fn term_open(
    profile_id: String,
    cols: u32,
    rows: u32,
    on_event: Channel<SessionEventDto>,
    on_output: Channel<InvokeResponseBody>,
    state: State<'_, LocalTerminals>,
    vault: State<'_, VaultState>,
) -> Result<TermId, AppError> {
    // Toute l'application est derrière le déverrouillage (ADR-028).
    if !vault.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    if profile_id.len() > MAX_PROFILE_ID_LEN {
        return Err(AppError::INVALID_INPUT);
    }
    let size = TermSize::new(cols, rows)?;
    let profile = state.profile(&profile_id)?;
    let id = state.insert(|_, remover| {
        let sink = ChannelSink {
            events: on_event,
            output: on_output,
            remover,
        };
        LocalSession::spawn(&profile, size, Arc::new(sink)).map_err(Into::into)
    })?;
    // Un verrouillage pendant le lancement : le terminal est refermé aussitôt (ADR-026).
    if !vault.is_unlocked() {
        let _ = state.close(id);
        return Err(AppError::VAULT_LOCKED);
    }
    tracing::debug!(terminal = id, "terminal local ouvert");
    Ok(id)
}

/// Envoie la saisie de l'utilisateur (même convention que `ssh_write`).
#[tauri::command]
pub async fn term_write(
    terminal_id: TermId,
    data: String,
    binary: bool,
    state: State<'_, LocalTerminals>,
) -> Result<(), AppError> {
    let bytes = if binary {
        data.chars()
            .map(|c| u8::try_from(u32::from(c)).map_err(|_| AppError::INVALID_INPUT))
            .collect::<Result<Vec<u8>, _>>()?
    } else {
        data.into_bytes()
    };
    let session = state.session(terminal_id)?;
    // L'écriture peut attendre (programme qui ne lit plus) : hors de l'exécuteur async.
    tauri::async_runtime::spawn_blocking(move || session.write(bytes))
        .await
        .map_err(|_| AppError::INTERNAL)?
        .map_err(Into::into)
}

/// Répercute la nouvelle taille du terminal.
#[tauri::command]
pub fn term_resize(
    terminal_id: TermId,
    cols: u32,
    rows: u32,
    state: State<'_, LocalTerminals>,
) -> Result<(), AppError> {
    let size = TermSize::new(cols, rows)?;
    state.session(terminal_id)?.resize(size).map_err(Into::into)
}

/// Acquitte des octets de sortie affichés (contrôle de flux).
#[tauri::command]
pub fn term_ack(
    terminal_id: TermId,
    bytes: u32,
    state: State<'_, LocalTerminals>,
) -> Result<(), AppError> {
    state.session(terminal_id)?.ack(bytes as usize);
    Ok(())
}

/// Ferme un terminal local (le processus est tué).
#[tauri::command]
pub fn term_close(terminal_id: TermId, state: State<'_, LocalTerminals>) -> Result<(), AppError> {
    state.close(terminal_id)
}
