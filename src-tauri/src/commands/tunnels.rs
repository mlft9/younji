//! Commandes des tunnels SSH (TUN-01 à TUN-03, étape 6).
//!
//! Les définitions vivent dans le coffre (validées par `core-vault`). Démarrer un tunnel ouvre
//! une connexion SSH dédiée (`core-ssh`), enregistrée comme une session : elle se pilote avec
//! `ssh_host_key_decision`, `ssh_auth_answer` et `ssh_close`, et elle est fermée au verrouillage
//! (ADR-026). Aucun secret ne passe par ces commandes : l'identité de l'hôte est lue dans le
//! coffre par le cœur.

use std::sync::Arc;

use core_ssh::{Credentials, ListenScope, SessionHandle, Timeouts, TunnelParams, TunnelSpec};
use core_vault::{BindScope, EntryId, Tunnel, TunnelInput, TunnelKind, VaultData, VaultError};
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Channel;

use crate::commands::ssh::{ChannelSink, SessionEventDto, close_if_locked, host_target};
use crate::error::AppError;
use crate::state::{SessionId, SessionManager};
use crate::vault_state::VaultState;

/// Tunnel, tel qu'affiché par l'interface (aucun secret).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelDto {
    id: EntryId,
    name: String,
    host_id: EntryId,
    kind: TunnelKind,
    bind_scope: BindScope,
    bind_port: u16,
    dest_host: String,
    dest_port: u16,
    auto_start: bool,
}

impl From<&Tunnel> for TunnelDto {
    fn from(tunnel: &Tunnel) -> Self {
        Self {
            id: tunnel.id,
            name: tunnel.name.clone(),
            host_id: tunnel.host_id,
            kind: tunnel.kind,
            bind_scope: tunnel.bind_scope,
            bind_port: tunnel.bind_port,
            dest_host: tunnel.dest_host.clone(),
            dest_port: tunnel.dest_port,
            auto_start: tunnel.auto_start,
        }
    }
}

/// Création ou modification d'un tunnel.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TunnelRequest {
    id: Option<EntryId>,
    name: String,
    host_id: EntryId,
    kind: TunnelKind,
    bind_scope: BindScope,
    bind_port: u32,
    dest_host: String,
    dest_port: u32,
    auto_start: bool,
    /// Confirmation explicite de l'écoute sur toutes les interfaces (TUN-03), exigée par le
    /// cœur à chaque enregistrement d'un tunnel exposé.
    exposure_confirmed: bool,
}

impl TryFrom<TunnelRequest> for TunnelInput {
    type Error = VaultError;

    fn try_from(request: TunnelRequest) -> Result<Self, Self::Error> {
        let port = |value: u32| u16::try_from(value).map_err(|_| VaultError::InvalidTunnel);
        Ok(Self {
            id: request.id,
            name: request.name,
            host_id: request.host_id,
            kind: request.kind,
            bind_scope: request.bind_scope,
            bind_port: port(request.bind_port)?,
            dest_host: request.dest_host,
            dest_port: port(request.dest_port)?,
            auto_start: request.auto_start,
            exposure_confirmed: request.exposure_confirmed,
        })
    }
}

/// Crée ou modifie un tunnel. Renvoie son identifiant.
#[tauri::command]
pub async fn tunnel_save(
    request: TunnelRequest,
    state: State<'_, VaultState>,
) -> Result<EntryId, AppError> {
    let input = TunnelInput::try_from(request)?;
    state
        .update_blocking(move |data| data.upsert_tunnel(input))
        .await
}

/// Supprime un tunnel. S'il est démarré, l'interface l'arrête d'abord (`ssh_close`).
#[tauri::command]
pub async fn tunnel_delete(id: EntryId, state: State<'_, VaultState>) -> Result<(), AppError> {
    state
        .update_blocking(move |data| data.delete_tunnel(id))
        .await
}

/// Paramètres de démarrage lus dans le coffre : cible et identité de l'hôte, redirection
/// revalidée par `core-ssh`. Sans identité, le serveur pose ses questions (keyboard-interactive).
fn tunnel_params(
    data: &VaultData,
    tunnel_id: EntryId,
    known_hosts: Arc<core_ssh::KnownHosts>,
) -> Result<TunnelParams, AppError> {
    let tunnel = data.tunnel(tunnel_id)?;
    let (target, credentials) = host_target(data, tunnel.host_id)?;
    let scope = match tunnel.bind_scope {
        BindScope::Loopback => ListenScope::Loopback,
        BindScope::AllInterfaces => ListenScope::AllInterfaces,
    };
    let port = u32::from(tunnel.bind_port);
    let dest_port = u32::from(tunnel.dest_port);
    let spec = match tunnel.kind {
        TunnelKind::Local => TunnelSpec::local(scope, port, &tunnel.dest_host, dest_port)?,
        TunnelKind::Remote => TunnelSpec::remote(scope, port, &tunnel.dest_host, dest_port)?,
        TunnelKind::Dynamic => TunnelSpec::dynamic(scope, port)?,
    };
    Ok(TunnelParams {
        target,
        credentials: credentials.unwrap_or(Credentials::Interactive),
        spec,
        known_hosts,
        timeouts: Timeouts::default(),
    })
}

/// Démarre un tunnel du coffre. Renvoie l'identifiant de sa session ; l'état arrive par
/// `on_event` (clé d'hôte, questions du serveur, `tunnelReady`, `tunnelStats`, fin).
#[tauri::command]
pub fn tunnel_start(
    tunnel_id: EntryId,
    on_event: Channel<SessionEventDto>,
    state: State<'_, SessionManager>,
    vault: State<'_, VaultState>,
) -> Result<SessionId, AppError> {
    let known_hosts = state.known_hosts();
    let params = vault.read(|vault| tunnel_params(vault.data(), tunnel_id, known_hosts))?;
    vault.touch();
    let host_pattern =
        core_ssh::known_hosts::host_pattern(params.target.host(), params.target.port());
    let id = state.insert_session(|_, remover| {
        let sink = ChannelSink {
            events: on_event,
            output: None,
            remover,
            host_pattern,
        };
        SessionHandle::spawn_tunnel(params, Arc::new(sink))
    })?;
    close_if_locked(id, &state, &vault)?;
    tracing::debug!(session = id, "tunnel démarré");
    Ok(id)
}

#[cfg(test)]
mod tests {
    use core_vault::HostInput;

    use super::*;

    fn request() -> serde_json::Value {
        serde_json::json!({
            "id": null,
            "name": "Web",
            "hostId": "6f9619ff-8b86-d011-b42d-00c04fc964ff",
            "kind": "local",
            "bindScope": "loopback",
            "bindPort": 8080,
            "destHost": "127.0.0.1",
            "destPort": 80,
            "autoStart": false,
            "exposureConfirmed": false
        })
    }

    #[test]
    fn request_is_strict() {
        assert!(serde_json::from_value::<TunnelRequest>(request()).is_ok());
        let mut extra = request();
        extra["bindAddress"] = serde_json::json!("0.0.0.0");
        assert!(serde_json::from_value::<TunnelRequest>(extra).is_err());
        let mut scope = request();
        scope["bindScope"] = serde_json::json!("0.0.0.0");
        assert!(serde_json::from_value::<TunnelRequest>(scope).is_err());
        let mut kind = request();
        kind["kind"] = serde_json::json!("x11");
        assert!(serde_json::from_value::<TunnelRequest>(kind).is_err());
        let mut missing = request();
        missing.as_object_mut().unwrap().remove("exposureConfirmed");
        assert!(serde_json::from_value::<TunnelRequest>(missing).is_err());
    }

    #[test]
    fn out_of_range_ports_are_refused() {
        let mut big = request();
        big["bindPort"] = serde_json::json!(70_000);
        let request: TunnelRequest = serde_json::from_value(big).unwrap();
        assert_eq!(
            TunnelInput::try_from(request).err(),
            Some(VaultError::InvalidTunnel)
        );
    }

    fn data_with_tunnel(identity: bool, kind: TunnelKind) -> (VaultData, EntryId) {
        let mut data = VaultData::default();
        let identity_id = identity.then(|| {
            data.upsert_identity(core_vault::IdentityInput {
                id: None,
                name: "admin".into(),
                username: "admin".into(),
                password: core_vault::PasswordChange::Set(secrecy::SecretString::from(
                    "secret".to_owned(),
                )),
                key_id: None,
            })
            .unwrap()
        });
        let host = data
            .upsert_host(HostInput {
                id: None,
                name: "srv".into(),
                address: "srv.lab".into(),
                port: 22,
                username: "admin".into(),
                identity_id,
                group_id: None,
                tags: Vec::new(),
                notes: String::new(),
            })
            .unwrap();
        let tunnel = data
            .upsert_tunnel(TunnelInput {
                id: None,
                name: "t".into(),
                host_id: host,
                kind,
                bind_scope: BindScope::Loopback,
                bind_port: 1080,
                dest_host: "intranet.lab".into(),
                dest_port: 443,
                auto_start: false,
                exposure_confirmed: false,
            })
            .unwrap();
        (data, tunnel)
    }

    #[test]
    fn params_come_from_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        let known = Arc::new(core_ssh::KnownHosts::new(dir.path().join("k")));
        let (data, id) = data_with_tunnel(true, TunnelKind::Local);
        let params = tunnel_params(&data, id, known.clone()).unwrap();
        assert_eq!(params.target.host(), "srv.lab");
        assert_eq!(params.spec.port(), 1080);
        assert_eq!(params.spec.scope(), ListenScope::Loopback);
        assert!(matches!(params.credentials, Credentials::Password(_)));

        // Sans identité : le serveur posera ses questions.
        let (data, id) = data_with_tunnel(false, TunnelKind::Dynamic);
        let params = tunnel_params(&data, id, known.clone()).unwrap();
        assert!(matches!(params.credentials, Credentials::Interactive));
        assert_eq!(params.spec.mode(), &core_ssh::tunnel::TunnelMode::Dynamic);

        assert_eq!(
            tunnel_params(
                &data,
                serde_json::from_value(serde_json::json!("6f9619ff-8b86-d011-b42d-00c04fc964ff"))
                    .unwrap(),
                known
            )
            .err()
            .map(|e| e.code()),
            Some("vault.entry_not_found")
        );
    }

    #[test]
    fn dto_serializes_in_camel_case() {
        let (data, _) = data_with_tunnel(true, TunnelKind::Remote);
        let json = serde_json::to_value(TunnelDto::from(&data.tunnels()[0])).unwrap();
        assert_eq!(json["kind"], "remote");
        assert_eq!(json["bindScope"], "loopback");
        assert_eq!(json["destHost"], "intranet.lab");
        assert_eq!(json["autoStart"], false);
    }
}
