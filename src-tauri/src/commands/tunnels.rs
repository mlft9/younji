//! Commandes des tunnels SSH (TUN-01 à TUN-03, étape 6).
//!
//! Les définitions vivent dans le coffre (validées par `core-vault`). Démarrer un tunnel ouvre
//! une connexion SSH dédiée (`core-ssh`), enregistrée comme une session : elle se pilote avec
//! `ssh_host_key_decision`, `ssh_auth_answer` et `ssh_close`, et elle est fermée au verrouillage
//! (ADR-026). Aucun secret ne passe par ces commandes : l'identité de l'hôte est lue dans le
//! coffre par le cœur.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use core_ssh::{Credentials, ListenScope, SessionHandle, Timeouts, TunnelParams, TunnelSpec};
use core_vault::{BindScope, EntryId, Tunnel, TunnelInput, TunnelKind, VaultData, VaultError};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::commands::ssh::{ChannelSink, SessionEventDto, close_if_locked, host_target};
use crate::error::AppError;
use crate::settings::SettingsStore;
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

/// Une seule confirmation d'exposition ouverte à la fois : une WebView compromise ne peut pas
/// empiler des boîtes de dialogue pour lasser l'utilisateur.
#[derive(Default)]
pub struct ExposureGate(AtomicBool);

/// Tour de confirmation en cours ; libère la place à sa destruction.
struct ExposureTurn<'a>(&'a AtomicBool);

impl ExposureGate {
    fn try_enter(&self) -> Result<ExposureTurn<'_>, AppError> {
        if self.0.swap(true, Ordering::AcqRel) {
            Err(AppError::TUNNEL_CONFIRMATION_PENDING)
        } else {
            Ok(ExposureTurn(&self.0))
        }
    }
}

impl Drop for ExposureTurn<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// État d'un tunnel exposé au moment de sa confirmation, **sans secret** (définition et hôte du
/// carnet) : il est gardé pendant la boîte de dialogue à la place des identifiants.
#[derive(PartialEq, Eq)]
struct Confirmed {
    tunnel: Tunnel,
    host: core_vault::Host,
}

/// Description à confirmer et état confirmé d'un tunnel exposé, ou `None`.
fn confirmation(
    data: &VaultData,
    tunnel_id: EntryId,
) -> Result<Option<(Exposure, Confirmed)>, AppError> {
    let Some(exposure) = exposure(data, tunnel_id)? else {
        return Ok(None);
    };
    let tunnel = data.tunnel(tunnel_id)?.clone();
    let host = data.host(tunnel.host_id)?.clone();
    Ok(Some((exposure, Confirmed { tunnel, host })))
}

/// Paramètres de démarrage, lus après la confirmation. La définition et l'hôte doivent être
/// exactement ceux confirmés : une modification pendant la boîte de dialogue (par une WebView
/// compromise) annule le démarrage.
fn params_after(
    data: &VaultData,
    tunnel_id: EntryId,
    confirmed: Option<&Confirmed>,
    known_hosts: Arc<core_ssh::KnownHosts>,
) -> Result<TunnelParams, AppError> {
    if let Some(confirmed) = confirmed {
        let tunnel = data.tunnel(tunnel_id)?;
        let unchanged =
            *tunnel == confirmed.tunnel && *data.host(tunnel.host_id)? == confirmed.host;
        if !unchanged {
            return Err(AppError::TUNNEL_CHANGED);
        }
    }
    tunnel_params(data, tunnel_id, known_hosts)
}

/// Texte affichable dans la boîte native : sans caractère de contrôle ni caractère invisible
/// (contrôles bidirectionnels, largeur nulle) qui réordonnerait visuellement le message.
fn displayable(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() && !core_vault::model::is_invisible_format(*c))
        .collect()
}

/// Ce que la confirmation native d'un tunnel exposé affiche, lu dans le coffre.
struct Exposure {
    name: String,
    flag: String,
    host: String,
}

/// Tunnel exposé sur toutes les interfaces : description à faire confirmer, sinon `None`.
fn exposure(data: &VaultData, tunnel_id: EntryId) -> Result<Option<Exposure>, AppError> {
    let tunnel = data.tunnel(tunnel_id)?;
    if tunnel.bind_scope != BindScope::AllInterfaces {
        return Ok(None);
    }
    let host = data.host(tunnel.host_id)?;
    let flag = match tunnel.kind {
        TunnelKind::Local => format!(
            "-L 0.0.0.0:{}:{}:{}",
            tunnel.bind_port, tunnel.dest_host, tunnel.dest_port
        ),
        TunnelKind::Remote => format!(
            "-R 0.0.0.0:{}:{}:{}",
            tunnel.bind_port, tunnel.dest_host, tunnel.dest_port
        ),
        TunnelKind::Dynamic => format!("-D 0.0.0.0:{} (SOCKS5)", tunnel.bind_port),
    };
    Ok(Some(Exposure {
        name: displayable(&tunnel.name),
        flag: displayable(&flag),
        host: displayable(&format!("{} ({})", host.name, host.address)),
    }))
}

/// Texte de la confirmation native, dans la langue choisie (les deux si « système » : le cœur
/// ne connaît pas la langue détectée par l'interface).
fn exposure_message(exposure: &Exposure, language: &str) -> (String, String, String, String) {
    let fr = format!(
        "Le tunnel « {} » va écouter sur 0.0.0.0 : toute machine qui peut joindre {} pourra \
         l'utiliser, sans authentification.\n\n{}\nvia {}",
        exposure.name,
        if exposure.flag.starts_with("-R") {
            "le serveur"
        } else {
            "ce PC"
        },
        exposure.flag,
        exposure.host
    );
    let en = format!(
        "The tunnel \"{}\" will listen on 0.0.0.0: any machine that can reach {} will be able \
         to use it, without authentication.\n\n{}\nvia {}",
        exposure.name,
        if exposure.flag.starts_with("-R") {
            "the server"
        } else {
            "this PC"
        },
        exposure.flag,
        exposure.host
    );
    match language {
        "en" => (
            "Expose a tunnel to the network?".into(),
            en,
            "Start".into(),
            "Cancel".into(),
        ),
        "fr" => (
            "Exposer un tunnel au réseau ?".into(),
            fr,
            "Démarrer".into(),
            "Annuler".into(),
        ),
        _ => (
            "Exposer un tunnel au réseau ? / Expose a tunnel to the network?".into(),
            format!("{fr}\n\n—\n\n{en}"),
            "Démarrer / Start".into(),
            "Annuler / Cancel".into(),
        ),
    }
}

/// Confirmation de l'exposition par une boîte de dialogue **native**, ouverte par le cœur : un
/// script de la WebView (dépendance compromise, M4) ne peut ni la remplir ni la contourner,
/// contrairement au drapeau `exposureConfirmed` de l'IPC, qui ne protège que d'un bogue.
async fn confirm_exposure(
    window: &WebviewWindow,
    exposure: &Exposure,
    language: &str,
) -> Result<(), AppError> {
    let (title, message, ok, cancel) = exposure_message(exposure, language);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(ok, cancel))
        .parent(window)
        .show(move |accepted| {
            let _ = sender.send(accepted);
        });
    if receiver.await.map_err(|_| AppError::DIALOG_FAILED)? {
        Ok(())
    } else {
        Err(AppError::TUNNEL_EXPOSURE_REFUSED)
    }
}

/// Démarre un tunnel du coffre. Renvoie l'identifiant de sa session ; l'état arrive par
/// `on_event` (clé d'hôte, questions du serveur, `tunnelReady`, `tunnelStats`, fin). Un tunnel
/// exposé sur toutes les interfaces n'est démarré qu'après une confirmation native, à chaque
/// démarrage (démarrage automatique compris).
#[tauri::command]
pub async fn tunnel_start(
    tunnel_id: EntryId,
    on_event: Channel<SessionEventDto>,
    window: WebviewWindow,
    state: State<'_, SessionManager>,
    vault: State<'_, VaultState>,
    settings: State<'_, SettingsStore>,
    gate: State<'_, ExposureGate>,
) -> Result<SessionId, AppError> {
    // Aucun secret n'est gardé pendant la confirmation (elle peut durer, et le coffre se
    // verrouiller entre-temps) ; ce qui démarre doit être exactement ce qui a été confirmé.
    let confirmed = match vault.read(|vault| confirmation(vault.data(), tunnel_id))? {
        Some((exposure, confirmed)) => {
            let _turn = gate.try_enter()?;
            let language = settings.load().settings.language;
            confirm_exposure(&window, &exposure, &language).await?;
            Some(confirmed)
        }
        None => None,
    };
    let known_hosts = state.known_hosts();
    let params = vault
        .read(|vault| params_after(vault.data(), tunnel_id, confirmed.as_ref(), known_hosts))?;
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
    fn only_exposed_tunnels_need_a_native_confirmation() {
        let (mut data, id) = data_with_tunnel(true, TunnelKind::Dynamic);
        assert!(exposure(&data, id).unwrap().is_none());
        let mut tunnel = data.tunnel(id).unwrap().clone();
        tunnel.bind_scope = BindScope::AllInterfaces;
        data.upsert_tunnel(TunnelInput {
            id: Some(id),
            name: tunnel.name,
            host_id: tunnel.host_id,
            kind: tunnel.kind,
            bind_scope: tunnel.bind_scope,
            bind_port: tunnel.bind_port,
            dest_host: tunnel.dest_host,
            dest_port: tunnel.dest_port,
            auto_start: true,
            exposure_confirmed: true,
        })
        .unwrap();
        let exposure = exposure(&data, id).unwrap().unwrap();
        assert_eq!(exposure.flag, "-D 0.0.0.0:1080 (SOCKS5)");
        let (_, message, ok, _) = exposure_message(&exposure, "fr");
        assert!(message.contains("0.0.0.0"));
        assert!(message.contains("srv.lab"));
        assert_eq!(ok, "Démarrer");
        let (title, ..) = exposure_message(&exposure, "system");
        assert!(title.contains("Expose"));
    }

    #[test]
    fn start_uses_the_definition_that_was_confirmed() {
        let dir = tempfile::tempdir().unwrap();
        let known = Arc::new(core_ssh::KnownHosts::new(dir.path().join("k")));
        let (mut data, id) = data_with_tunnel(true, TunnelKind::Local);
        let original = data.tunnel(id).unwrap().clone();
        let expose = |data: &mut VaultData, kind, port| {
            data.upsert_tunnel(TunnelInput {
                id: Some(id),
                name: original.name.clone(),
                host_id: original.host_id,
                kind,
                bind_scope: BindScope::AllInterfaces,
                bind_port: port,
                dest_host: original.dest_host.clone(),
                dest_port: original.dest_port,
                auto_start: false,
                exposure_confirmed: true,
            })
            .unwrap();
        };
        expose(&mut data, TunnelKind::Local, 8080);
        let (shown, confirmed) = confirmation(&data, id).unwrap().unwrap();
        assert!(shown.flag.starts_with("-L 0.0.0.0:8080:"));
        // Définition inchangée : démarrage de ce qui a été confirmé.
        let params = params_after(&data, id, Some(&confirmed), known.clone()).unwrap();
        assert_eq!(params.spec.port(), 8080);
        // Échange de la définition pendant la confirmation : démarrage refusé.
        expose(&mut data, TunnelKind::Dynamic, 1080);
        assert_eq!(
            params_after(&data, id, Some(&confirmed), known.clone())
                .err()
                .map(|e| e.code()),
            Some("tunnels.changed")
        );
        // Tunnel non exposé : rien à confirmer.
        let (data, id) = data_with_tunnel(true, TunnelKind::Local);
        assert!(confirmation(&data, id).unwrap().is_none());
        assert!(params_after(&data, id, None, known).is_ok());
    }

    #[test]
    fn displayed_text_drops_invisible_characters() {
        assert_eq!(displayable("ab\u{202E}cd\u{200B}e\n"), "abcde");
    }

    #[test]
    fn only_one_exposure_confirmation_at_a_time() {
        let gate = ExposureGate::default();
        let turn = gate.try_enter().unwrap();
        assert_eq!(
            gate.try_enter().err().map(|e| e.code()),
            Some("tunnels.confirmation_pending")
        );
        drop(turn);
        assert!(gate.try_enter().is_ok());
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
