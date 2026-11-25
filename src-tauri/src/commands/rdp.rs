//! Commande IPC RDP (étape 8, RDP-02). L'interface ne désigne qu'un hôte du carnet : adresse,
//! utilisateur, domaine et mot de passe sont lus dans le coffre par le cœur. Le mot de passe ne
//! revient jamais vers l'interface et ne passe jamais en argument de processus.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use core_rdp::{RdpProcess, RdpTarget, Resolution};
use core_vault::{EntryId, HostProtocol, RdpResolution, VaultData};
use secrecy::SecretString;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::error::AppError;
use crate::vault_state::VaultState;

/// Clients RDP lancés en même temps au plus.
pub const MAX_RDP_CLIENTS: usize = 16;

/// Clients RDP lancés par Younji, arrêtés au verrouillage (ADR-026).
#[derive(Default)]
pub struct RdpClients {
    clients: Mutex<Vec<Arc<RdpProcess>>>,
}

impl RdpClients {
    fn add(&self, process: RdpProcess) -> Result<Arc<RdpProcess>, AppError> {
        let mut clients = self.clients.lock().map_err(|_| AppError::INTERNAL)?;
        clients.retain(|c| c.is_running());
        if clients.len() >= MAX_RDP_CLIENTS {
            process.terminate();
            return Err(AppError::TOO_MANY_SESSIONS);
        }
        let process = Arc::new(process);
        clients.push(process.clone());
        Ok(process)
    }

    /// Arrête tous les clients lancés et supprime leurs traces (identifiants, fichiers).
    pub fn terminate_all(&self) {
        let clients: Vec<_> = match self.clients.lock() {
            Ok(mut clients) => clients.drain(..).collect(),
            Err(_) => return,
        };
        for client in clients {
            client.terminate();
        }
    }
}

/// Résultat d'un lancement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RdpLaunchDto {
    /// Windows : un identifiant enregistré par l'utilisateur pour ce serveur a été utilisé
    /// (Younji ne l'a ni remplacé ni supprimé).
    existing_credential: bool,
}

/// Dossier privé des fichiers `.rdp` temporaires.
pub fn rdp_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|_| AppError::INTERNAL)?
        .join("rdp"))
}

/// Cible et mot de passe d'un hôte RDP du carnet.
fn rdp_target(
    data: &VaultData,
    host_id: EntryId,
) -> Result<(RdpTarget, Option<SecretString>), AppError> {
    let host = data.host(host_id)?;
    let HostProtocol::Rdp { domain, resolution } = &host.protocol else {
        return Err(AppError::HOST_NOT_RDP);
    };
    let identity = host.identity_id.map(|id| data.identity(id)).transpose()?;
    let username = if host.username.is_empty() {
        identity.map(|i| i.username.as_str()).unwrap_or_default()
    } else {
        host.username.as_str()
    };
    let resolution = match *resolution {
        RdpResolution::Fullscreen {} => Resolution::Fullscreen,
        RdpResolution::Fixed { width, height } => Resolution::Fixed { width, height },
    };
    let target = RdpTarget::new(&host.address, host.port, username, domain, resolution)?;
    Ok((target, identity.and_then(|i| i.password.clone())))
}

/// Ouvre un hôte RDP du carnet dans le client du système.
#[tauri::command]
pub async fn rdp_launch(
    host_id: EntryId,
    app: AppHandle,
    vault: State<'_, VaultState>,
    clients: State<'_, RdpClients>,
) -> Result<RdpLaunchDto, AppError> {
    let (target, password) = vault.read(|vault| rdp_target(vault.data(), host_id))?;
    vault.touch();
    let dir = rdp_dir(&app)?;
    // Recherche du client, écriture du fichier et de l'identifiant : hors de l'exécuteur async.
    let process = tauri::async_runtime::spawn_blocking(move || {
        let client = core_rdp::find_client()?;
        core_rdp::launch(&client, &target, password.as_ref(), &dir)
    })
    .await
    .map_err(|_| AppError::INTERNAL)??;
    let process = clients.add(process)?;
    // Un verrouillage pendant le lancement : le client est arrêté aussitôt (ADR-026).
    if !vault.is_unlocked() {
        process.terminate();
        return Err(AppError::VAULT_LOCKED);
    }
    tracing::debug!("client RDP lancé depuis le carnet");
    Ok(RdpLaunchDto {
        existing_credential: process.used_existing_credential(),
    })
}

#[cfg(test)]
mod tests {
    use core_vault::{HostInput, IdentityInput, PasswordChange};
    use secrecy::ExposeSecret;

    use super::*;

    fn host(data: &mut VaultData, protocol: HostProtocol, identity: Option<EntryId>) -> EntryId {
        data.upsert_host(HostInput {
            id: None,
            name: "w".into(),
            address: "win.lab".into(),
            port: 3389,
            username: String::new(),
            identity_id: identity,
            group_id: None,
            tags: vec![],
            notes: String::new(),
            protocol,
        })
        .unwrap()
    }

    #[test]
    fn target_and_password_come_from_the_vault() {
        let mut data = VaultData::default();
        let identity = data
            .upsert_identity(IdentityInput {
                id: None,
                name: "admin".into(),
                username: "Administrateur".into(),
                password: PasswordChange::Set(SecretString::from("pw-rdp")),
                key_id: None,
            })
            .unwrap();
        let rdp = HostProtocol::Rdp {
            domain: "LAB".into(),
            resolution: RdpResolution::Fixed {
                width: 1920,
                height: 1080,
            },
        };
        let id = host(&mut data, rdp, Some(identity));
        let (target, password) = rdp_target(&data, id).unwrap();
        assert_eq!(target.username(), "Administrateur");
        assert_eq!(target.domain(), "LAB");
        assert_eq!(target.host_port(), "win.lab:3389");
        assert_eq!(password.unwrap().expose_secret(), "pw-rdp");

        let ssh = host(&mut data, HostProtocol::default(), Some(identity));
        assert_eq!(rdp_target(&data, ssh).unwrap_err(), AppError::HOST_NOT_RDP);
    }
}
