//! Commandes IPC de l'étape 9 : import OpenSSH (IMP-01, IMP-02) et sauvegarde chiffrée (VLT-06).
//!
//! - Les fichiers sont lus par le cœur : chemin par défaut (`~/.ssh/config`, `~/.ssh/known_hosts`)
//!   ou fichier choisi dans la boîte de dialogue **ouverte par le cœur** (ADR-020).
//! - Un import se fait en deux temps : aperçu calculé et **gardé dans le cœur**, puis application
//!   de cet aperçu. L'interface n'envoie qu'un identifiant d'aperçu et des indices bornés : elle ne
//!   peut pas glisser d'autres hôtes ni d'autres clés d'hôte.
//! - Sauvegarde : la phrase de passe n'arrive qu'une fois et n'est jamais renvoyée ; la boîte de
//!   dialogue (action humaine) précède toute dérivation, ce qui empêche une interface compromise
//!   de tester des mots de passe maîtres en boucle via le refus « phrase identique ».

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use core_import::{ConfigReport, merge_known_hosts};
use core_vault::model::MAX_KNOWN_HOSTS_LEN;
use core_vault::{HostInput, HostProtocol, MergeCounts, VaultData};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::error::AppError;
use crate::vault_state::VaultState;

/// Taille maximale d'un `known_hosts` importé.
const MAX_KNOWN_HOSTS_FILE: u64 = MAX_KNOWN_HOSTS_LEN as u64;
/// Taille maximale d'une sauvegarde lue (en-tête compris).
const MAX_BACKUP_FILE: u64 = core_vault::format::MAX_PAYLOAD_LEN + 1024;
/// Nom de groupe des hôtes importés.
const IMPORT_GROUP: &str = "Import OpenSSH";

/// Hôte prêt à être ajouté (déjà validé comme une saisie).
#[derive(Clone)]
struct PendingHost {
    alias: String,
    address: String,
    port: u16,
    username: String,
}

/// Aperçus en attente d'application, oubliés au verrouillage.
#[derive(Default)]
pub struct ImportState {
    next_id: AtomicU64,
    config: Mutex<Option<(u64, Vec<PendingHost>)>>,
    known_hosts: Mutex<Option<(u64, String, String)>>,
}

impl ImportState {
    fn id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Oublie les aperçus (verrouillage, rechargement).
    pub fn clear(&self) {
        if let Ok(mut config) = self.config.lock() {
            config.take();
        }
        if let Ok(mut known) = self.known_hosts.lock() {
            known.take();
        }
    }
}

/// Origine du fichier à importer.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportSource {
    /// Emplacement d'OpenSSH dans le dossier personnel.
    Default,
    /// Fichier choisi dans la boîte de dialogue du système.
    Pick,
}

fn ssh_dir(app: &AppHandle) -> Result<(PathBuf, PathBuf), AppError> {
    let home = app.path().home_dir().map_err(|_| AppError::INTERNAL)?;
    Ok((home.join(".ssh"), home))
}

/// Ouvre la boîte de dialogue de choix d'un fichier (dans `~/.ssh` si possible).
async fn pick_file(
    app: &AppHandle,
    window: &WebviewWindow,
    directory: Option<PathBuf>,
) -> Result<Option<PathBuf>, AppError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut dialog = app.dialog().file().set_parent(window);
    if let Some(dir) = directory.filter(|d| d.is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_file(move |picked| {
        let _ = sender.send(picked);
    });
    match receiver.await.map_err(|_| AppError::DIALOG_FAILED)? {
        Some(picked) => Ok(Some(
            picked.into_path().map_err(|_| AppError::DIALOG_FAILED)?,
        )),
        None => Ok(None),
    }
}

/// Utilisateur du poste (valeur par défaut d'OpenSSH quand `User` est absent).
fn local_username() -> Option<String> {
    ["USER", "USERNAME"]
        .iter()
        .find_map(|var| std::env::var(var).ok())
        .filter(|u| !u.is_empty() && u.len() <= 64 && !u.chars().any(char::is_control))
}

// ------------------------------------------------------------------ ~/.ssh/config

/// Hôte proposé à l'import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHostDto {
    index: u32,
    alias: String,
    address: String,
    port: u16,
    username: String,
    warnings: Vec<&'static str>,
    identity_files: Vec<String>,
    already_in_vault: bool,
}

/// Entrée ignorée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedDto {
    entry: String,
    reason: &'static str,
}

/// Aperçu de l'import de `~/.ssh/config`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPreviewDto {
    preview_id: u64,
    hosts: Vec<ConfigHostDto>,
    skipped: Vec<SkippedDto>,
}

/// Valide les hôtes du rapport comme une saisie et marque ceux déjà présents dans le carnet.
fn build_config_preview(
    report: ConfigReport,
    data: &VaultData,
    default_user: Option<&str>,
) -> (Vec<PendingHost>, Vec<ConfigHostDto>, Vec<SkippedDto>) {
    let mut pending = Vec::new();
    let mut hosts = Vec::new();
    let mut skipped: Vec<SkippedDto> = report
        .skipped
        .into_iter()
        .map(|s| SkippedDto {
            entry: s.entry,
            reason: s.reason.code(),
        })
        .collect();
    for host in report.hosts {
        let entry = format!("Host {}", host.alias);
        let Some(username) = host.user.clone().or(default_user.map(str::to_owned)) else {
            skipped.push(SkippedDto {
                entry,
                reason: core_import::SkipReason::InvalidValue.code(),
            });
            continue;
        };
        // Mêmes validations qu'une saisie manuelle (adresse, port, utilisateur).
        let Ok(target) = core_ssh::Target::new(&host.hostname, u32::from(host.port), &username)
        else {
            skipped.push(SkippedDto {
                entry,
                reason: core_import::SkipReason::InvalidValue.code(),
            });
            continue;
        };
        let address = target.host().to_owned();
        let already_in_vault = data.hosts().iter().any(|h| {
            !h.protocol.is_rdp()
                && h.address.eq_ignore_ascii_case(&address)
                && h.port == host.port
                && h.username == username
        });
        let index = u32::try_from(pending.len()).unwrap_or(u32::MAX);
        hosts.push(ConfigHostDto {
            index,
            alias: host.alias.clone(),
            address: address.clone(),
            port: host.port,
            username: username.clone(),
            warnings: host.warnings.iter().map(|w| w.code()).collect(),
            identity_files: host.identity_files.clone(),
            already_in_vault,
        });
        pending.push(PendingHost {
            alias: host.alias,
            address,
            port: host.port,
            username,
        });
    }
    (pending, hosts, skipped)
}

/// Lit `~/.ssh/config` (ou un fichier choisi) et renvoie un aperçu. `None` : choix annulé.
#[tauri::command]
pub async fn import_ssh_config_preview(
    source: ImportSource,
    app: AppHandle,
    window: WebviewWindow,
    vault: State<'_, VaultState>,
    state: State<'_, ImportState>,
) -> Result<Option<ConfigPreviewDto>, AppError> {
    if !vault.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    let (ssh, home) = ssh_dir(&app)?;
    let path = match source {
        ImportSource::Default => ssh.join("config"),
        ImportSource::Pick => match pick_file(&app, &window, Some(ssh.clone())).await? {
            Some(path) => path,
            None => return Ok(None),
        },
    };
    let report = tauri::async_runtime::spawn_blocking(move || {
        core_import::parse_ssh_config(&path, &ssh, &home)
    })
    .await
    .map_err(|_| AppError::INTERNAL)?;
    let default_user = local_username();
    let (pending, hosts, skipped) = vault.read(|vault| {
        Ok(build_config_preview(
            report,
            vault.data(),
            default_user.as_deref(),
        ))
    })?;
    let preview_id = state.id();
    *state.config.lock().map_err(|_| AppError::INTERNAL)? = Some((preview_id, pending));
    tracing::debug!(
        hosts = hosts.len(),
        skipped = skipped.len(),
        "aperçu de l'import ssh_config"
    );
    Ok(Some(ConfigPreviewDto {
        preview_id,
        hosts,
        skipped,
    }))
}

/// Ajoute au carnet les hôtes choisis de l'aperçu (groupe « Import OpenSSH », sans identité).
/// Renvoie le nombre d'hôtes ajoutés.
#[tauri::command]
pub async fn import_ssh_config_apply(
    preview_id: u64,
    indices: Vec<u32>,
    vault: State<'_, VaultState>,
    state: State<'_, ImportState>,
) -> Result<usize, AppError> {
    let pending = {
        let mut slot = state.config.lock().map_err(|_| AppError::INTERNAL)?;
        match slot.take() {
            Some((id, pending)) if id == preview_id => pending,
            other => {
                *slot = other;
                return Err(AppError::IMPORT_STALE);
            }
        }
    };
    if indices.len() > pending.len() {
        return Err(AppError::INVALID_INPUT);
    }
    let mut chosen: Vec<PendingHost> = Vec::with_capacity(indices.len());
    let mut seen = std::collections::HashSet::new();
    for index in indices {
        let host = pending.get(index as usize).ok_or(AppError::INVALID_INPUT)?;
        if !seen.insert(index) {
            return Err(AppError::INVALID_INPUT);
        }
        chosen.push(host.clone());
    }
    if chosen.is_empty() {
        return Ok(0);
    }
    vault
        .update_blocking(move |data| {
            let group = match data.groups().iter().find(|g| g.name == IMPORT_GROUP) {
                Some(group) => group.id,
                None => data.upsert_group(None, IMPORT_GROUP)?,
            };
            for host in &chosen {
                data.upsert_host(HostInput {
                    id: None,
                    name: host.alias.clone(),
                    address: host.address.clone(),
                    port: host.port,
                    username: host.username.clone(),
                    identity_id: None,
                    group_id: Some(group),
                    tags: vec!["ssh-config".into()],
                    notes: String::new(),
                    protocol: HostProtocol::default(),
                })?;
            }
            Ok(chosen.len())
        })
        .await
}

// ------------------------------------------------------------------ known_hosts

/// Ligne ajoutée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHostAddedDto {
    hosts: String,
    algorithm: String,
}

/// Ligne refusée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHostSkippedDto {
    line: usize,
    hosts: String,
    reason: &'static str,
}

/// Aperçu de l'import de `known_hosts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHostsPreviewDto {
    preview_id: u64,
    added_count: usize,
    added: Vec<KnownHostAddedDto>,
    duplicates: usize,
    skipped_count: usize,
    skipped: Vec<KnownHostSkippedDto>,
}

/// Lit au plus `max` octets d'un fichier **ordinaire** : type vérifié avant l'ouverture (un
/// FIFO bloquerait) puis sur le descripteur ouvert, et lecture bornée quelle que soit la taille
/// annoncée (un périphérique annonce 0, un fichier peut grossir entre-temps).
fn read_bytes_bounded(path: &std::path::Path, max: u64) -> Result<Vec<u8>, AppError> {
    use std::io::Read;
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return Err(AppError::IMPORT_READ_FAILED);
    }
    let file = std::fs::File::open(path).map_err(|_| AppError::IMPORT_READ_FAILED)?;
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err(AppError::IMPORT_READ_FAILED);
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::IMPORT_READ_FAILED)?;
    if bytes.len() as u64 > max {
        return Err(AppError::IMPORT_TOO_LARGE);
    }
    Ok(bytes)
}

fn read_text_bounded(path: &std::path::Path, max: u64) -> Result<String, AppError> {
    String::from_utf8(read_bytes_bounded(path, max)?).map_err(|_| AppError::IMPORT_READ_FAILED)
}

/// Lit `~/.ssh/known_hosts` (ou un fichier choisi) et calcule la fusion avec les clés d'hôte
/// du coffre, sans rien enregistrer. `None` : choix annulé.
#[tauri::command]
pub async fn import_known_hosts_preview(
    source: ImportSource,
    app: AppHandle,
    window: WebviewWindow,
    vault: State<'_, VaultState>,
    state: State<'_, ImportState>,
) -> Result<Option<KnownHostsPreviewDto>, AppError> {
    if !vault.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    let (ssh, _) = ssh_dir(&app)?;
    let path = match source {
        ImportSource::Default => ssh.join("known_hosts"),
        ImportSource::Pick => match pick_file(&app, &window, Some(ssh)).await? {
            Some(path) => path,
            None => return Ok(None),
        },
    };
    let imported = tauri::async_runtime::spawn_blocking(move || {
        read_text_bounded(&path, MAX_KNOWN_HOSTS_FILE)
    })
    .await
    .map_err(|_| AppError::INTERNAL)??;
    let current = vault.read(|vault| Ok(vault.data().known_hosts().to_owned()))?;
    let merge = merge_known_hosts(
        &current,
        &imported,
        &core_ssh::known_hosts::is_valid_public_key,
        MAX_KNOWN_HOSTS_LEN,
    );
    let preview_id = state.id();
    let dto = KnownHostsPreviewDto {
        preview_id,
        added_count: merge.added_count,
        added: merge
            .added
            .into_iter()
            .map(|(hosts, algorithm)| KnownHostAddedDto { hosts, algorithm })
            .collect(),
        duplicates: merge.duplicates,
        skipped_count: merge.skipped_count,
        skipped: merge
            .skipped
            .into_iter()
            .map(|(line, hosts, reason)| KnownHostSkippedDto {
                line,
                hosts,
                reason: reason.code(),
            })
            .collect(),
    };
    *state.known_hosts.lock().map_err(|_| AppError::INTERNAL)? =
        Some((preview_id, current, merge.content));
    Ok(Some(dto))
}

/// Enregistre la fusion de l'aperçu, si les clés d'hôte du coffre n'ont pas changé depuis.
#[tauri::command]
pub async fn import_known_hosts_apply(
    preview_id: u64,
    vault: State<'_, VaultState>,
    state: State<'_, ImportState>,
) -> Result<(), AppError> {
    let (base, merged) = {
        let mut slot = state.known_hosts.lock().map_err(|_| AppError::INTERNAL)?;
        match slot.take() {
            Some((id, base, merged)) if id == preview_id => (base, merged),
            other => {
                *slot = other;
                return Err(AppError::IMPORT_STALE);
            }
        }
    };
    vault
        .update_blocking(move |data| {
            // Une clé acceptée entre l'aperçu et l'application : l'aperçu ne vaut plus.
            if data.known_hosts() != base {
                return Err(core_vault::VaultError::EntryNotFound);
            }
            data.set_known_hosts(merged)
        })
        .await
        .map_err(|error| {
            if error == AppError::from(core_vault::VaultError::EntryNotFound) {
                AppError::IMPORT_STALE
            } else {
                error
            }
        })
}

// ------------------------------------------------------------------ sauvegarde

/// Exporte une sauvegarde chiffrée dans un fichier choisi par l'utilisateur. Renvoie `false`
/// si le choix est annulé. La phrase de passe doit être robuste et distincte du mot de passe
/// maître.
#[tauri::command]
pub async fn backup_export(
    passphrase: SecretString,
    app: AppHandle,
    window: WebviewWindow,
    vault: State<'_, VaultState>,
) -> Result<bool, AppError> {
    // Contrôle immédiat de la robustesse, avant la boîte de dialogue.
    core_vault::policy::check_master_password(&passphrase)?;
    if !vault.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_file_name("younji-sauvegarde.younji-backup")
        .add_filter("Younji", &["younji-backup"])
        .save_file(move |picked| {
            let _ = sender.send(picked);
        });
    let Some(picked) = receiver.await.map_err(|_| AppError::DIALOG_FAILED)? else {
        return Ok(false);
    };
    let path = picked.into_path().map_err(|_| AppError::DIALOG_FAILED)?;
    let bytes = vault.export_backup(passphrase).await?;
    tauri::async_runtime::spawn_blocking(move || write_private(&path, &bytes))
        .await
        .map_err(|_| AppError::INTERNAL)??;
    tracing::info!("sauvegarde chiffrée exportée");
    Ok(true)
}

/// Écriture atomique (fichier temporaire du même dossier, `fsync`, renommage), 0600 sous Unix.
/// Le fichier confirmé dans la boîte de dialogue peut être remplacé.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), AppError> {
    use std::io::Write;
    let failed = |error: std::io::Error| {
        tracing::debug!(%error, "écriture de la sauvegarde impossible");
        AppError::EXPORT_FAILED
    };
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or(AppError::EXPORT_FAILED)?;
    // `NamedTempFile` crée le fichier en 0600 sous Unix.
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(failed)?;
    file.write_all(bytes).map_err(failed)?;
    file.as_file().sync_all().map_err(failed)?;
    file.persist(path).map_err(|error| failed(error.error))?;
    Ok(())
}

/// Mode d'import d'une sauvegarde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupMode {
    /// Ajoute les entrées absentes, sans rien écraser.
    Merge,
    /// Remplace tout le contenu du coffre (le mot de passe maître ne change pas).
    Replace,
}

/// Bilan d'un import de sauvegarde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupImportDto {
    hosts: usize,
    groups: usize,
    identities: usize,
    keys: usize,
    snippets: usize,
    known_hosts: usize,
    skipped: usize,
}

fn apply_backup(
    data: &mut VaultData,
    backup: VaultData,
    mode: BackupMode,
) -> Result<BackupImportDto, core_vault::VaultError> {
    match mode {
        BackupMode::Replace => {
            let dto = BackupImportDto {
                hosts: backup.hosts().len(),
                groups: backup.groups().len(),
                identities: backup.identities().len(),
                keys: backup.keys().len(),
                snippets: backup.snippets().len(),
                known_hosts: backup.known_hosts().lines().count(),
                skipped: 0,
            };
            *data = backup;
            Ok(dto)
        }
        BackupMode::Merge => {
            let MergeCounts {
                hosts,
                groups,
                identities,
                keys,
                snippets,
                skipped,
            } = data.merge_from(&backup)?;
            // Clés d'hôte : même règle que l'import de known_hosts (jamais une autre clé pour un
            // hôte déjà connu).
            let merge = merge_known_hosts(
                data.known_hosts(),
                backup.known_hosts(),
                &core_ssh::known_hosts::is_valid_public_key,
                MAX_KNOWN_HOSTS_LEN,
            );
            data.set_known_hosts(merge.content)?;
            Ok(BackupImportDto {
                hosts,
                groups,
                identities,
                keys,
                snippets,
                known_hosts: merge.added_count,
                skipped: skipped + merge.skipped_count,
            })
        }
    }
}

/// Vérifie que l'algorithme, l'empreinte et la clé publique affichés de chaque clé de la
/// sauvegarde correspondent bien à sa clé privée : une sauvegarde fabriquée ne doit pas pouvoir
/// présenter une empreinte trompeuse (déploiement, comparaison avec `ssh-keygen -l`).
fn check_backup_keys(backup: &VaultData) -> Result<(), AppError> {
    for key in backup.keys() {
        let private = core_ssh::auth::decode_stored_key(&key.private_key)
            .map_err(|_| core_vault::VaultError::InvalidFile)?;
        let derived = core_ssh::auth::prepare_import(&private)
            .map_err(|_| core_vault::VaultError::InvalidFile)?;
        if derived.algorithm != key.algorithm
            || derived.fingerprint != key.fingerprint
            || derived.public_key != key.public_key
        {
            return Err(core_vault::VaultError::InvalidFile.into());
        }
    }
    Ok(())
}

/// Importe une sauvegarde choisie par l'utilisateur. `None` : choix annulé.
#[tauri::command]
pub async fn backup_import(
    passphrase: SecretString,
    mode: BackupMode,
    app: AppHandle,
    window: WebviewWindow,
    vault: State<'_, VaultState>,
) -> Result<Option<BackupImportDto>, AppError> {
    if !vault.is_unlocked() {
        return Err(AppError::VAULT_LOCKED);
    }
    let Some(path) = pick_file(&app, &window, None).await? else {
        return Ok(None);
    };
    let backup = tauri::async_runtime::spawn_blocking(move || {
        let bytes = read_bytes_bounded(&path, MAX_BACKUP_FILE)?;
        let backup = core_vault::backup::read_backup(&bytes, &passphrase)?;
        check_backup_keys(&backup)?;
        Ok::<_, AppError>(backup)
    })
    .await
    .map_err(|_| AppError::INTERNAL)??;
    let dto = vault
        .update_blocking(move |data| apply_backup(data, backup, mode))
        .await?;
    tracing::info!(?mode, "sauvegarde chiffrée importée");
    Ok(Some(dto))
}

#[cfg(test)]
mod tests {
    use core_import::{ImportedHost, SkipReason, Skipped};

    use super::*;

    fn imported(alias: &str, hostname: &str, user: Option<&str>) -> ImportedHost {
        ImportedHost {
            alias: alias.into(),
            hostname: hostname.into(),
            port: 22,
            user: user.map(str::to_owned),
            identity_files: vec![],
            warnings: vec![],
        }
    }

    #[test]
    fn preview_validates_hosts_and_marks_existing_ones() {
        let mut data = VaultData::default();
        data.upsert_host(HostInput {
            id: None,
            name: "déjà".into(),
            address: "srv.lab".into(),
            port: 22,
            username: "root".into(),
            identity_id: None,
            group_id: None,
            tags: vec![],
            notes: String::new(),
            protocol: HostProtocol::default(),
        })
        .unwrap();
        let report = ConfigReport {
            hosts: vec![
                imported("srv", "srv.lab", Some("root")),
                imported("neuf", "neuf.lab", None),
                imported("bad", "-oProxyCommand=x", Some("u")),
            ],
            skipped: vec![Skipped {
                entry: "Host *".into(),
                reason: SkipReason::Wildcard,
            }],
        };
        let (pending, hosts, skipped) = build_config_preview(report, &data, Some("moi"));
        assert_eq!(pending.len(), 2);
        assert!(hosts[0].already_in_vault);
        assert_eq!(hosts[1].username, "moi");
        assert!(!hosts[1].already_in_vault);
        assert_eq!(skipped.len(), 2);
        assert_eq!(skipped[1].entry, "Host bad");
    }

    #[cfg(unix)]
    #[test]
    fn reads_are_bounded_and_limited_to_regular_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, b"0123456789").unwrap();
        assert_eq!(read_bytes_bounded(&path, 10).unwrap().len(), 10);
        assert_eq!(
            read_bytes_bounded(&path, 9),
            Err(AppError::IMPORT_TOO_LARGE)
        );
        assert_eq!(
            read_bytes_bounded(std::path::Path::new("/dev/zero"), 10),
            Err(AppError::IMPORT_READ_FAILED)
        );
        assert_eq!(
            read_bytes_bounded(dir.path(), 10),
            Err(AppError::IMPORT_READ_FAILED)
        );
    }

    #[test]
    fn backup_keys_must_match_their_private_key() {
        let mut data = VaultData::default();
        let generated = core_ssh::generate_key(core_ssh::KeyKind::Ed25519).unwrap();
        let other = core_ssh::generate_key(core_ssh::KeyKind::Ed25519).unwrap();
        data.add_key(core_vault::NewKey {
            name: "ok".into(),
            algorithm: generated.algorithm.clone(),
            fingerprint: generated.fingerprint.clone(),
            public_key: generated.public_key.clone(),
            private_key: generated.private_key,
        })
        .unwrap();
        assert!(check_backup_keys(&data).is_ok());
        data.add_key(core_vault::NewKey {
            name: "menteuse".into(),
            algorithm: generated.algorithm,
            fingerprint: generated.fingerprint,
            public_key: generated.public_key,
            private_key: other.private_key,
        })
        .unwrap();
        assert_eq!(
            check_backup_keys(&data),
            Err(AppError::from(core_vault::VaultError::InvalidFile))
        );
    }

    #[test]
    fn requests_are_strict() {
        assert!(serde_json::from_value::<ImportSource>(serde_json::json!("default")).is_ok());
        assert!(serde_json::from_value::<ImportSource>(serde_json::json!("/etc/passwd")).is_err());
        assert!(serde_json::from_value::<BackupMode>(serde_json::json!("replace")).is_ok());
        assert!(serde_json::from_value::<BackupMode>(serde_json::json!("other")).is_err());
    }
}
