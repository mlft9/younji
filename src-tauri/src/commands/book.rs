//! Commandes du carnet d'hôtes et des clés (HST-01 à HST-03, KEY-01, KEY-02, HKY-04).
//!
//! Les structures renvoyées à l'interface ne contiennent **aucun secret** : une identité indique
//! seulement qu'elle a un mot de passe (`hasPassword`), une clé ne montre que sa partie publique
//! et son empreinte. Les secrets arrivent une fois (`identity_save`, `key_import`) ou naissent
//! dans le cœur (`key_generate`), et restent dans le coffre.

use core_ssh::auth::import_private_key;
use core_ssh::{ImportedKey, KeyInfo, KeyKind, KnownHostEntry, Target};
use core_vault::{
    EntryId, Group, Host, HostInput, Identity, IdentityInput, NewKey, PasswordChange, StoredKey,
    VaultData,
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::commands::snippets::SnippetDto;
use crate::error::AppError;
use crate::state::{KeyFileId, SessionManager};
use crate::vault_state::VaultState;

// ------------------------------------------------------------------ DTO (sans secret)

/// Hôte, tel qu'affiché par l'interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostDto {
    id: EntryId,
    name: String,
    address: String,
    port: u16,
    username: String,
    identity_id: Option<EntryId>,
    group_id: Option<EntryId>,
    tags: Vec<String>,
    notes: String,
}

impl From<&Host> for HostDto {
    fn from(host: &Host) -> Self {
        Self {
            id: host.id,
            name: host.name.clone(),
            address: host.address.clone(),
            port: host.port,
            username: host.username.clone(),
            identity_id: host.identity_id,
            group_id: host.group_id,
            tags: host.tags.clone(),
            notes: host.notes.clone(),
        }
    }
}

/// Groupe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDto {
    id: EntryId,
    name: String,
}

impl From<&Group> for GroupDto {
    fn from(group: &Group) -> Self {
        Self {
            id: group.id,
            name: group.name.clone(),
        }
    }
}

/// Identité, sans son mot de passe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDto {
    id: EntryId,
    name: String,
    username: String,
    has_password: bool,
    key_id: Option<EntryId>,
}

impl From<&Identity> for IdentityDto {
    fn from(identity: &Identity) -> Self {
        Self {
            id: identity.id,
            name: identity.name.clone(),
            username: identity.username.clone(),
            has_password: identity.password.is_some(),
            key_id: identity.key_id,
        }
    }
}

/// Clé importée : partie publique uniquement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyDto {
    id: EntryId,
    name: String,
    algorithm: String,
    fingerprint: String,
    public_key: String,
}

impl From<&StoredKey> for KeyDto {
    fn from(key: &StoredKey) -> Self {
        Self {
            id: key.id,
            name: key.name.clone(),
            algorithm: key.algorithm.clone(),
            fingerprint: key.fingerprint.clone(),
            public_key: key.public_key.clone(),
        }
    }
}

/// Contenu affichable du coffre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDto {
    hosts: Vec<HostDto>,
    groups: Vec<GroupDto>,
    identities: Vec<IdentityDto>,
    keys: Vec<KeyDto>,
    snippets: Vec<SnippetDto>,
    auto_lock_minutes: u32,
}

impl From<&VaultData> for SnapshotDto {
    fn from(data: &VaultData) -> Self {
        Self {
            hosts: data.hosts().iter().map(Into::into).collect(),
            groups: data.groups().iter().map(Into::into).collect(),
            identities: data.identities().iter().map(Into::into).collect(),
            keys: data.keys().iter().map(Into::into).collect(),
            snippets: data.snippets().iter().map(Into::into).collect(),
            auto_lock_minutes: data.settings().auto_lock_minutes,
        }
    }
}

/// Clé d'hôte connue (HKY-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnownHostDto {
    line: usize,
    hosts: String,
    algorithm: String,
    fingerprint: String,
    revoked: bool,
}

impl From<KnownHostEntry> for KnownHostDto {
    fn from(entry: KnownHostEntry) -> Self {
        Self {
            line: entry.line,
            hosts: entry.hosts,
            algorithm: entry.key.algorithm,
            fingerprint: entry.key.fingerprint,
            revoked: entry.revoked,
        }
    }
}

impl From<KnownHostDto> for KnownHostEntry {
    fn from(dto: KnownHostDto) -> Self {
        Self {
            line: dto.line,
            hosts: dto.hosts,
            key: KeyInfo {
                algorithm: dto.algorithm,
                fingerprint: dto.fingerprint,
            },
            revoked: dto.revoked,
        }
    }
}

// ------------------------------------------------------------------ requêtes

/// Création ou modification d'un hôte.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostRequest {
    id: Option<EntryId>,
    name: String,
    address: String,
    port: u32,
    username: String,
    identity_id: Option<EntryId>,
    group_id: Option<EntryId>,
    tags: Vec<String>,
    notes: String,
}

/// Modification du mot de passe d'une identité. Pas de `Debug` : contient un secret.
#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PasswordRequest {
    /// Garder le mot de passe enregistré. Variante structurée (vide) pour que
    /// `deny_unknown_fields` refuse un champ glissé à côté, ce que serde ne fait pas pour une
    /// variante unitaire.
    Keep {},
    /// Retirer le mot de passe.
    Clear {},
    /// Nouveau mot de passe.
    Set { value: SecretString },
}

/// Création ou modification d'une identité. Pas de `Debug` : contient un secret.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityRequest {
    id: Option<EntryId>,
    name: String,
    username: String,
    password: PasswordRequest,
    key_id: Option<EntryId>,
}

/// Import d'une clé choisie par `ssh_pick_key_file`. Pas de `Debug` : contient un secret.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyImportRequest {
    key_file_id: KeyFileId,
    name: String,
    passphrase: Option<SecretString>,
}

/// Type de clé à générer (KEY-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyKindRequest {
    Ed25519,
    EcdsaP256,
    EcdsaP384,
    EcdsaP521,
    Rsa4096,
}

impl From<KeyKindRequest> for KeyKind {
    fn from(kind: KeyKindRequest) -> Self {
        match kind {
            KeyKindRequest::Ed25519 => Self::Ed25519,
            KeyKindRequest::EcdsaP256 => Self::EcdsaP256,
            KeyKindRequest::EcdsaP384 => Self::EcdsaP384,
            KeyKindRequest::EcdsaP521 => Self::EcdsaP521,
            KeyKindRequest::Rsa4096 => Self::Rsa4096,
        }
    }
}

/// Génération d'une clé dans le coffre.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyGenerateRequest {
    name: String,
    kind: KeyKindRequest,
}

/// Création ou renommage d'un groupe.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupRequest {
    id: Option<EntryId>,
    name: String,
}

// ------------------------------------------------------------------ commandes

/// Contenu affichable du coffre (sans aucun secret).
#[tauri::command]
pub fn vault_snapshot(state: State<'_, VaultState>) -> Result<SnapshotDto, AppError> {
    state.read(|vault| Ok(SnapshotDto::from(vault.data())))
}

/// Crée ou modifie un hôte. L'adresse, le port et l'utilisateur sont validés avec les mêmes
/// règles que la connexion SSH.
#[tauri::command]
pub async fn host_save(
    request: HostRequest,
    state: State<'_, VaultState>,
) -> Result<EntryId, AppError> {
    let probe_user = if request.username.is_empty() {
        "user"
    } else {
        request.username.as_str()
    };
    let target = Target::new(&request.address, request.port, probe_user)?;
    let input = HostInput {
        id: request.id,
        name: request.name,
        address: target.host().to_owned(),
        port: target.port(),
        username: request.username,
        identity_id: request.identity_id,
        group_id: request.group_id,
        tags: request.tags,
        notes: request.notes,
    };
    state
        .update_blocking(move |data| data.upsert_host(input))
        .await
}

/// Supprime un hôte.
#[tauri::command]
pub async fn host_delete(id: EntryId, state: State<'_, VaultState>) -> Result<(), AppError> {
    state
        .update_blocking(move |data| data.delete_host(id))
        .await
}

/// Crée ou renomme un groupe.
#[tauri::command]
pub async fn group_save(
    request: GroupRequest,
    state: State<'_, VaultState>,
) -> Result<EntryId, AppError> {
    state
        .update_blocking(move |data| data.upsert_group(request.id, &request.name))
        .await
}

/// Supprime un groupe (ses hôtes passent « sans groupe »).
#[tauri::command]
pub async fn group_delete(id: EntryId, state: State<'_, VaultState>) -> Result<(), AppError> {
    state
        .update_blocking(move |data| data.delete_group(id))
        .await
}

/// Crée ou modifie une identité.
#[tauri::command]
pub async fn identity_save(
    request: IdentityRequest,
    state: State<'_, VaultState>,
) -> Result<EntryId, AppError> {
    let password = match request.password {
        PasswordRequest::Keep {} => PasswordChange::Keep,
        PasswordRequest::Clear {} => PasswordChange::Clear,
        PasswordRequest::Set { value } => PasswordChange::Set(value),
    };
    let input = IdentityInput {
        id: request.id,
        name: request.name,
        username: request.username,
        password,
        key_id: request.key_id,
    };
    state
        .update_blocking(move |data| data.upsert_identity(input))
        .await
}

/// Supprime une identité (refusé si un hôte l'utilise).
#[tauri::command]
pub async fn identity_delete(id: EntryId, state: State<'_, VaultState>) -> Result<(), AppError> {
    state
        .update_blocking(move |data| data.delete_identity(id))
        .await
}

/// Importe dans le coffre une clé privée choisie par la boîte de dialogue du système (le
/// chemin reste dans le cœur, ADR-020). La clé est déchiffrée avec sa passphrase puis stockée
/// sans elle : c'est le coffre qui la protège.
#[tauri::command]
pub async fn key_import(
    request: KeyImportRequest,
    sessions: State<'_, SessionManager>,
    state: State<'_, VaultState>,
) -> Result<KeyDto, AppError> {
    if !state.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    let path = sessions.key_file(request.key_file_id)?;
    let passphrase = request.passphrase;
    let imported = tauri::async_runtime::spawn_blocking(move || {
        import_private_key(&path, passphrase.as_ref())
    })
    .await
    .map_err(|_| AppError::INTERNAL)??;
    store_key(request.name, imported, &state).await
}

/// Enregistre une clé (importée ou générée) dans le coffre et renvoie sa partie publique.
async fn store_key(
    name: String,
    imported: ImportedKey,
    state: &VaultState,
) -> Result<KeyDto, AppError> {
    let key = NewKey {
        name,
        algorithm: imported.algorithm,
        fingerprint: imported.fingerprint,
        public_key: imported.public_key,
        private_key: imported.private_key,
    };
    let id = state.update_blocking(move |data| data.add_key(key)).await?;
    state.read(|vault| Ok(KeyDto::from(vault.data().key(id)?)))
}

/// Génère une clé (KEY-01) directement dans le coffre : la partie privée naît dans le cœur et
/// n'en sort jamais ; seule la partie publique est renvoyée.
#[tauri::command]
pub async fn key_generate(
    request: KeyGenerateRequest,
    state: State<'_, VaultState>,
) -> Result<KeyDto, AppError> {
    if !state.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    // Nom vérifié avant la génération (RSA 4096 prend du temps) ; revérifié à l'enregistrement.
    core_vault::model::clean_name(&request.name)?;
    let kind = KeyKind::from(request.kind);
    let generated = tauri::async_runtime::spawn_blocking(move || core_ssh::generate_key(kind))
        .await
        .map_err(|_| AppError::INTERNAL)??;
    state.touch();
    store_key(request.name, generated, &state).await
}

/// Nom de fichier proposé pour l'export d'une clé publique : caractères sûrs uniquement.
fn public_key_file_name(name: &str) -> String {
    let base: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    let base = base.trim_matches('_');
    if base.is_empty() {
        "younji_key.pub".to_owned()
    } else {
        format!("{base}.pub")
    }
}

/// Exporte la clé publique dans un fichier `.pub` (KEY-02). Le chemin est choisi dans la boîte
/// de dialogue du système, ouverte par le cœur, et ne remonte jamais à l'interface (ADR-020).
/// Renvoie `false` si l'utilisateur annule.
#[tauri::command]
pub async fn key_export_public(
    id: EntryId,
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, VaultState>,
) -> Result<bool, AppError> {
    let (name, public_key) = state.read(|vault| {
        let key = vault.data().key(id)?;
        Ok((key.name.clone(), key.public_key.clone()))
    })?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_file_name(public_key_file_name(&name))
        .add_filter("OpenSSH (.pub)", &["pub"])
        .save_file(move |picked| {
            let _ = sender.send(picked);
        });
    let Some(picked) = receiver.await.map_err(|_| AppError::DIALOG_FAILED)? else {
        return Ok(false);
    };
    let chosen = picked.into_path().map_err(|_| AppError::DIALOG_FAILED)?;
    let path = with_pub_extension(chosen.clone());
    // Écraser n'est permis que pour le fichier confirmé dans la boîte de dialogue : si
    // l'extension a été ajoutée, le fichier visé n'a pas été confirmé.
    let overwrite = path == chosen;
    tauri::async_runtime::spawn_blocking(move || write_atomically(&path, &public_key, overwrite))
        .await
        .map_err(|_| AppError::INTERNAL)??;
    Ok(true)
}

/// Impose l'extension `.pub` : sous Linux, le filtre de la boîte de dialogue ne l'ajoute pas,
/// et un nom tapé sans extension (`id_ed25519`) écraserait sinon la clé **privée** voisine.
fn with_pub_extension(path: std::path::PathBuf) -> std::path::PathBuf {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pub"))
    {
        path
    } else {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".pub");
        path.with_file_name(name)
    }
}

/// Écrit la clé publique par fichier temporaire du même dossier puis renommage : une écriture
/// interrompue ne laisse jamais un fichier tronqué.
fn write_atomically(
    path: &std::path::Path,
    public_key: &str,
    overwrite: bool,
) -> Result<(), AppError> {
    use std::io::Write;
    let failed = |error: std::io::Error| {
        tracing::debug!(%error, "écriture de la clé publique impossible");
        AppError::EXPORT_FAILED
    };
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or(AppError::EXPORT_FAILED)?;
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(failed)?;
    file.write_all(format!("{public_key}\n").as_bytes())
        .map_err(failed)?;
    file.as_file().sync_all().map_err(failed)?;
    if overwrite {
        file.persist(path).map_err(|error| failed(error.error))?;
    } else {
        file.persist_noclobber(path)
            .map_err(|error| failed(error.error))?;
    }
    Ok(())
}

/// Renomme une clé.
#[tauri::command]
pub async fn key_rename(
    id: EntryId,
    name: String,
    state: State<'_, VaultState>,
) -> Result<(), AppError> {
    state
        .update_blocking(move |data| data.rename_key(id, &name))
        .await
}

/// Supprime une clé (refusé si une identité l'utilise).
#[tauri::command]
pub async fn key_delete(id: EntryId, state: State<'_, VaultState>) -> Result<(), AppError> {
    state.update_blocking(move |data| data.delete_key(id)).await
}

/// Clés d'hôte connues (HKY-04).
#[tauri::command]
pub fn known_host_list(
    sessions: State<'_, SessionManager>,
    state: State<'_, VaultState>,
) -> Result<Vec<KnownHostDto>, AppError> {
    if !state.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    let entries = sessions.known_hosts().entries()?;
    Ok(entries.into_iter().map(Into::into).collect())
}

/// Supprime une clé d'hôte connue (après avertissement dans l'interface). Renvoie le nombre de
/// lignes retirées.
#[tauri::command]
pub fn known_host_delete(
    entry: KnownHostDto,
    sessions: State<'_, SessionManager>,
    state: State<'_, VaultState>,
) -> Result<usize, AppError> {
    if !state.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    Ok(sessions.known_hosts().remove(&entry.into())?)
}

#[cfg(test)]
mod tests {
    use core_vault::{IdentityInput, NewKey, PasswordChange};
    use secrecy::SecretString;

    use super::*;

    #[test]
    fn snapshot_never_contains_secrets() {
        let mut data = VaultData::default();
        let key = data
            .add_key(NewKey {
                name: "Clé".into(),
                algorithm: "ssh-ed25519".into(),
                fingerprint: "SHA256:abc".into(),
                public_key: "ssh-ed25519 AAAA".into(),
                private_key: SecretString::from("-----BEGIN OPENSSH PRIVATE KEY----- secret"),
            })
            .unwrap();
        data.upsert_identity(IdentityInput {
            id: None,
            name: "Admin".into(),
            username: "admin".into(),
            password: PasswordChange::Set(SecretString::from("hunter2")),
            key_id: Some(key),
        })
        .unwrap();
        let json = serde_json::to_string(&SnapshotDto::from(&data)).unwrap();
        assert!(!json.contains("hunter2"));
        assert!(!json.contains("PRIVATE"));
        assert!(json.contains("\"hasPassword\":true"));
        assert!(json.contains("\"publicKey\":\"ssh-ed25519 AAAA\""));
        assert!(json.contains("\"autoLockMinutes\":15"));
    }

    #[test]
    fn requests_reject_unknown_fields_and_bad_ids() {
        let host = serde_json::json!({
            "id": null, "name": "web", "address": "srv", "port": 22, "username": "u",
            "identityId": null, "groupId": null, "tags": [], "notes": ""
        });
        assert!(serde_json::from_value::<HostRequest>(host.clone()).is_ok());
        let mut extra = host.clone();
        extra["path"] = "C:/secret".into();
        assert!(serde_json::from_value::<HostRequest>(extra).is_err());
        let mut bad_id = host;
        bad_id["identityId"] = "../../etc".into();
        assert!(serde_json::from_value::<HostRequest>(bad_id).is_err());

        let identity = serde_json::json!({
            "id": null, "name": "a", "username": "u", "keyId": null,
            "password": { "action": "set", "value": "pw" }
        });
        let parsed = serde_json::from_value::<IdentityRequest>(identity).unwrap();
        assert!(matches!(parsed.password, PasswordRequest::Set { .. }));
        let keep = serde_json::json!({
            "id": null, "name": "a", "username": "u", "keyId": null,
            "password": { "action": "keep", "value": "glissé" }
        });
        assert!(serde_json::from_value::<IdentityRequest>(keep).is_err());
        let keep = serde_json::json!({
            "id": null, "name": "a", "username": "u", "keyId": null,
            "password": { "action": "keep" }
        });
        let parsed = serde_json::from_value::<IdentityRequest>(keep).unwrap();
        assert!(matches!(parsed.password, PasswordRequest::Keep {}));
    }

    #[test]
    fn key_generate_request_is_strict() {
        let parsed: KeyGenerateRequest =
            serde_json::from_value(serde_json::json!({ "name": "k", "kind": "ecdsaP384" }))
                .unwrap();
        assert_eq!(KeyKind::from(parsed.kind), KeyKind::EcdsaP384);
        for kind in ["ed25519", "ecdsaP256", "ecdsaP521", "rsa4096"] {
            assert!(
                serde_json::from_value::<KeyGenerateRequest>(
                    serde_json::json!({ "name": "k", "kind": kind })
                )
                .is_ok()
            );
        }
        for kind in ["rsa1024", "dsa", "RSA4096", ""] {
            assert!(
                serde_json::from_value::<KeyGenerateRequest>(
                    serde_json::json!({ "name": "k", "kind": kind })
                )
                .is_err(),
                "{kind}"
            );
        }
        assert!(
            serde_json::from_value::<KeyGenerateRequest>(
                serde_json::json!({ "name": "k", "kind": "ed25519", "passphrase": "x" })
            )
            .is_err()
        );
    }

    #[test]
    fn exported_file_name_is_safe() {
        assert_eq!(public_key_file_name("Ma clé lab"), "Ma_cl__lab.pub");
        assert_eq!(public_key_file_name("../../etc/passwd"), "etc_passwd.pub");
        assert_eq!(public_key_file_name("C:\\x"), "C__x.pub");
        assert_eq!(public_key_file_name(""), "younji_key.pub");
        assert_eq!(public_key_file_name("..."), "younji_key.pub");
        assert!(public_key_file_name(&"a".repeat(500)).len() <= 68);
    }

    #[test]
    fn exported_file_always_ends_in_pub_and_is_written_whole() {
        use std::path::PathBuf;
        assert_eq!(
            with_pub_extension(PathBuf::from("/h/.ssh/id_ed25519")),
            PathBuf::from("/h/.ssh/id_ed25519.pub")
        );
        assert_eq!(
            with_pub_extension(PathBuf::from("/h/cle.PUB")),
            PathBuf::from("/h/cle.PUB")
        );
        assert_eq!(
            with_pub_extension(PathBuf::from("/h/cle.txt")),
            PathBuf::from("/h/cle.txt.pub")
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cle.pub");
        std::fs::write(&path, "ancien contenu plus long que la clé").unwrap();
        write_atomically(&path, "ssh-ed25519 AAAA", true).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ssh-ed25519 AAAA\n"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        // Fichier non confirmé par l'utilisateur : jamais écrasé.
        assert_eq!(
            write_atomically(&path, "ssh-ed25519 BBBB", false),
            Err(AppError::EXPORT_FAILED)
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ssh-ed25519 AAAA\n"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn known_host_dto_round_trips() {
        let entry = KnownHostEntry {
            line: 3,
            hosts: "[srv]:2222".into(),
            key: KeyInfo {
                algorithm: "ssh-ed25519".into(),
                fingerprint: "SHA256:abc".into(),
            },
            revoked: false,
        };
        let dto = KnownHostDto::from(entry.clone());
        assert_eq!(KnownHostEntry::from(dto), entry);
    }
}
