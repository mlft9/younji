//! État du coffre dans l'application : verrouillé ou déverrouillé, tentatives de
//! déverrouillage, activité de l'utilisateur et verrouillage automatique (VLT-03).
//!
//! Le coffre déverrouillé (`core_vault::Vault`) vit derrière un mutex. Le verrouiller, c'est le
//! détruire : la DEK et tous les secrets déchiffrés sont effacés de la mémoire (`zeroize`).
//! Le minuteur de verrouillage automatique tourne **dans le cœur** : l'interface ne peut que
//! signaler une activité de l'utilisateur, elle ne décide pas du verrouillage.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use core_ssh::{KnownHostsStorage, SshError};
use core_vault::{Vault, VaultData, VaultError, VaultFileStatus, file_status};
use secrecy::SecretString;
use serde::Serialize;
use tauri::ipc::Channel;

use crate::error::AppError;

/// Nombre d'échecs tolérés avant d'imposer un délai.
const FREE_ATTEMPTS: u32 = 3;
/// Délai maximal entre deux tentatives.
const MAX_UNLOCK_DELAY: Duration = Duration::from_secs(30);

/// État exposé à l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VaultStatus {
    /// Aucun coffre : il faut le créer.
    Missing,
    /// Coffre présent et verrouillé.
    Locked,
    /// Coffre déverrouillé.
    Unlocked,
}

/// Cause d'un verrouillage, transmise à l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LockReason {
    /// Demandé par l'utilisateur.
    Manual,
    /// Délai d'inactivité écoulé.
    Idle,
    /// Rechargement de l'interface.
    Reload,
}

/// Événement envoyé à l'interface par le `Channel` de `vault_watch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VaultEventDto {
    /// Le coffre vient d'être verrouillé.
    Locked {
        /// Cause du verrouillage.
        reason: LockReason,
    },
}

/// Délai imposé avant une tentative, selon le nombre d'échecs consécutifs :
/// rien pour les 3 premiers, puis 1 s, 2 s, 4 s… jusqu'à 30 s (SECURITE.md §3.4).
pub fn unlock_delay(failures: u32) -> Duration {
    if failures < FREE_ATTEMPTS {
        return Duration::ZERO;
    }
    let exponent = (failures - FREE_ATTEMPTS).min(5);
    Duration::from_secs(1 << exponent).min(MAX_UNLOCK_DELAY)
}

/// Instant de la dernière activité, selon deux horloges.
#[derive(Debug, Clone, Copy)]
pub struct Activity {
    /// Horloge monotone : insensible aux changements d'heure, mais arrêtée pendant la veille
    /// sous Linux.
    pub monotonic: Instant,
    /// Horloge murale : avance pendant la veille.
    pub wall: SystemTime,
}

impl Activity {
    /// Maintenant.
    pub fn now() -> Self {
        Self {
            monotonic: Instant::now(),
            wall: SystemTime::now(),
        }
    }
}

/// Vrai si le délai d'inactivité est écoulé selon **l'une ou l'autre** horloge. L'horloge
/// murale couvre la mise en veille (un portable rouvert après des heures se verrouille tout de
/// suite) ; une horloge murale qui recule n'empêche jamais le verrouillage, car l'horloge
/// monotone s'applique toujours.
pub fn idle_expired(last: Activity, now: Activity, minutes: u32) -> bool {
    let delay = Duration::from_secs(u64::from(minutes) * 60);
    let monotonic = now.monotonic.saturating_duration_since(last.monotonic) >= delay;
    let wall = now
        .wall
        .duration_since(last.wall)
        .is_ok_and(|elapsed| elapsed >= delay);
    monotonic || wall
}

/// Partie partagée avec le stockage des clés d'hôte.
struct Shared {
    vault: Mutex<Option<Vault>>,
}

impl Shared {
    fn lock(&self) -> Result<MutexGuard<'_, Option<Vault>>, AppError> {
        self.vault.lock().map_err(|_| AppError::INTERNAL)
    }
}

/// État du coffre géré par Tauri.
pub struct VaultState {
    path: PathBuf,
    /// Ancien fichier `known_hosts` en clair (étape 1), importé à la création du coffre.
    legacy_known_hosts: PathBuf,
    shared: Arc<Shared>,
    /// Sérialise les tentatives de déverrouillage et compte les échecs consécutifs.
    attempts: tokio::sync::Mutex<u32>,
    last_activity: Mutex<Activity>,
    watcher: Mutex<Option<Channel<VaultEventDto>>>,
}

impl VaultState {
    /// État pour le coffre `path`.
    pub fn new(path: PathBuf, legacy_known_hosts: PathBuf) -> Self {
        Self {
            path,
            legacy_known_hosts,
            shared: Arc::new(Shared {
                vault: Mutex::new(None),
            }),
            attempts: tokio::sync::Mutex::new(0),
            last_activity: Mutex::new(Activity::now()),
            watcher: Mutex::new(None),
        }
    }

    /// Stockage des clés d'hôte dans le coffre, pour `core-ssh`.
    pub fn known_hosts_storage(&self) -> VaultKnownHosts {
        VaultKnownHosts {
            shared: self.shared.clone(),
        }
    }

    /// État actuel.
    pub fn status(&self) -> Result<VaultStatus, AppError> {
        if self.shared.lock()?.is_some() {
            return Ok(VaultStatus::Unlocked);
        }
        Ok(match file_status(&self.path) {
            VaultFileStatus::Missing => VaultStatus::Missing,
            VaultFileStatus::Present => VaultStatus::Locked,
        })
    }

    /// Vrai si le coffre est déverrouillé.
    pub fn is_unlocked(&self) -> bool {
        self.shared.lock().is_ok_and(|vault| vault.is_some())
    }

    /// Crée le coffre et le laisse déverrouillé. L'ancien `known_hosts` en clair, s'il existe,
    /// est repris dans le coffre puis supprimé.
    pub async fn create(&self, password: SecretString) -> Result<(), AppError> {
        if self.is_unlocked() {
            return Err(VaultError::AlreadyExists.into());
        }
        let path = self.path.clone();
        let legacy = self.legacy_known_hosts.clone();
        let vault = tauri::async_runtime::spawn_blocking(move || {
            let mut data = VaultData::default();
            let imported = match core_ssh::known_hosts::FileStorage::new(&legacy).load() {
                Ok(content) if !content.is_empty() => data.set_known_hosts(content).is_ok(),
                Ok(_) => false,
                Err(_) => {
                    tracing::warn!("ancien known_hosts illisible : non repris");
                    false
                }
            };
            let vault = Vault::create(path, &password, data)?;
            if imported {
                match std::fs::remove_file(&legacy) {
                    Ok(()) => tracing::info!("clés d'hôte reprises dans le coffre"),
                    Err(error) => tracing::warn!(%error, "ancien known_hosts non supprimé"),
                }
            }
            Ok::<_, VaultError>(vault)
        })
        .await
        .map_err(|_| AppError::INTERNAL)??;
        *self.shared.lock()? = Some(vault);
        self.touch();
        Ok(())
    }

    /// Déverrouille le coffre. Les tentatives sont traitées une à une, avec un délai croissant
    /// après plusieurs échecs, pour qu'une interface compromise ne puisse pas enchaîner les essais.
    pub async fn unlock(&self, password: SecretString) -> Result<(), AppError> {
        let mut failures = self.attempts.lock().await;
        if self.is_unlocked() {
            return Ok(());
        }
        let delay = unlock_delay(*failures);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let path = self.path.clone();
        let result = tauri::async_runtime::spawn_blocking(move || Vault::unlock(path, &password))
            .await
            .map_err(|_| AppError::INTERNAL)?;
        match result {
            Ok(vault) => {
                *failures = 0;
                *self.shared.lock()? = Some(vault);
                self.touch();
                Ok(())
            }
            Err(error) => {
                *failures = failures.saturating_add(1);
                tracing::info!(failures = *failures, "échec du déverrouillage");
                Err(error.into())
            }
        }
    }

    /// Exporte une sauvegarde chiffrée (VLT-06). Le refus d'une phrase de passe qui ouvre le
    /// coffre révèle si elle **est** le mot de passe maître : chaque export compte donc comme une
    /// tentative et partage le compteur et le délai progressif du déverrouillage (SECURITE.md
    /// §3.4), remis à zéro par le prochain déverrouillage réussi.
    pub async fn export_backup(&self, passphrase: SecretString) -> Result<Vec<u8>, AppError> {
        let mut failures = self.attempts.lock().await;
        let delay = unlock_delay(*failures);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        *failures = failures.saturating_add(1);
        let shared = self.shared.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let guard = shared.lock()?;
            let vault = guard.as_ref().ok_or(AppError::VAULT_LOCKED)?;
            vault.export_backup(&passphrase).map_err(AppError::from)
        })
        .await
        .map_err(|_| AppError::INTERNAL)?
    }

    /// Verrouille : le coffre déchiffré est détruit (DEK et secrets effacés). Renvoie `true`
    /// s'il était déverrouillé.
    pub fn lock(&self, reason: LockReason) -> bool {
        let taken = match self.shared.vault.lock() {
            Ok(mut vault) => vault.take(),
            // Un mutex empoisonné ne doit pas empêcher le verrouillage : on récupère la valeur.
            Err(poisoned) => poisoned.into_inner().take(),
        };
        let was_unlocked = taken.is_some();
        drop(taken);
        if was_unlocked {
            tracing::info!(?reason, "coffre verrouillé");
            if let Ok(watcher) = self.watcher.lock()
                && let Some(channel) = watcher.as_ref()
                && channel.send(VaultEventDto::Locked { reason }).is_err()
            {
                tracing::debug!("événement de verrouillage non délivré");
            }
        }
        was_unlocked
    }

    /// Lit le coffre déverrouillé.
    pub fn read<T>(&self, f: impl FnOnce(&Vault) -> Result<T, AppError>) -> Result<T, AppError> {
        let guard = self.shared.lock()?;
        let vault = guard.as_ref().ok_or(AppError::VAULT_LOCKED)?;
        f(vault)
    }

    /// Modifie les données et sauvegarde (rien ne change en cas d'échec), hors du fil appelant :
    /// le chiffrement et l'écriture atomique (`fsync`, copie `.bak`) ne bloquent ni l'interface
    /// ni l'exécuteur async.
    pub async fn update_blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut VaultData) -> Result<T, VaultError> + Send + 'static,
    ) -> Result<T, AppError> {
        let shared = self.shared.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut guard = shared.lock()?;
            let vault = guard.as_mut().ok_or(AppError::VAULT_LOCKED)?;
            vault.update(f).map_err(AppError::from)
        })
        .await
        .map_err(|_| AppError::INTERNAL)?
    }

    /// Change le mot de passe maître (deux dérivations Argon2id, hors de l'exécuteur async).
    /// La vérification du mot de passe actuel partage le compteur d'échecs et le délai du
    /// déverrouillage : sinon, une interface compromise pourrait s'en servir pour deviner le
    /// mot de passe maître sans ralentissement.
    pub async fn change_password(
        &self,
        current: SecretString,
        new: SecretString,
    ) -> Result<(), AppError> {
        let mut failures = self.attempts.lock().await;
        let delay = unlock_delay(*failures);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let shared = self.shared.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let mut guard = shared.lock()?;
            let vault = guard.as_mut().ok_or(AppError::VAULT_LOCKED)?;
            vault
                .change_password(&current, &new)
                .map_err(AppError::from)
        })
        .await
        .map_err(|_| AppError::INTERNAL)?;
        match result {
            Err(error) if error == AppError::from(VaultError::WrongPasswordOrCorrupted) => {
                *failures = failures.saturating_add(1);
                tracing::info!(failures = *failures, "mot de passe actuel refusé");
                Err(error)
            }
            Ok(()) => {
                *failures = 0;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Note une activité de l'utilisateur (remet le minuteur à zéro).
    pub fn touch(&self) {
        if let Ok(mut last) = self.last_activity.lock() {
            *last = Activity::now();
        }
    }

    /// Vrai si le coffre est déverrouillé et inactif depuis plus que le délai configuré.
    pub fn idle_lock_due(&self, now: Activity) -> bool {
        let Ok(guard) = self.shared.vault.lock() else {
            return false;
        };
        let Some(vault) = guard.as_ref() else {
            return false;
        };
        let minutes = vault.data().settings().auto_lock_minutes;
        self.last_activity
            .lock()
            .is_ok_and(|last| idle_expired(*last, now, minutes))
    }

    /// Enregistre le destinataire des événements (remplace le précédent).
    pub fn watch(&self, channel: Channel<VaultEventDto>) -> Result<(), AppError> {
        *self.watcher.lock().map_err(|_| AppError::INTERNAL)? = Some(channel);
        Ok(())
    }
}

/// Clés d'hôte connues stockées dans le coffre (ADR à l'étape 2) : chiffrées et authentifiées,
/// donc ni lisibles ni modifiables hors de l'application.
pub struct VaultKnownHosts {
    shared: Arc<Shared>,
}

impl KnownHostsStorage for VaultKnownHosts {
    fn load(&self) -> Result<String, SshError> {
        let guard = self
            .shared
            .vault
            .lock()
            .map_err(|_| SshError::KnownHostsIo)?;
        match guard.as_ref() {
            Some(vault) => Ok(vault.data().known_hosts().to_owned()),
            None => Err(SshError::KnownHostsIo),
        }
    }

    fn store(&self, content: String) -> Result<(), SshError> {
        let mut guard = self
            .shared
            .vault
            .lock()
            .map_err(|_| SshError::KnownHostsIo)?;
        let vault = guard.as_mut().ok_or(SshError::KnownHostsIo)?;
        vault
            .update(|data| data.set_known_hosts(content))
            .map_err(|error| {
                tracing::warn!(code = error.code(), "clés d'hôte non enregistrées");
                SshError::KnownHostsIo
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_grows_after_three_failures_and_is_capped() {
        assert_eq!(unlock_delay(0), Duration::ZERO);
        assert_eq!(unlock_delay(2), Duration::ZERO);
        assert_eq!(unlock_delay(3), Duration::from_secs(1));
        assert_eq!(unlock_delay(4), Duration::from_secs(2));
        assert_eq!(unlock_delay(6), Duration::from_secs(8));
        assert_eq!(unlock_delay(8), Duration::from_secs(30));
        assert_eq!(unlock_delay(u32::MAX), Duration::from_secs(30));
    }

    fn after(start: Activity, monotonic: u64, wall: i64) -> Activity {
        let wall = if wall >= 0 {
            start.wall + Duration::from_secs(wall.unsigned_abs())
        } else {
            start.wall - Duration::from_secs(wall.unsigned_abs())
        };
        Activity {
            monotonic: start.monotonic + Duration::from_secs(monotonic),
            wall,
        }
    }

    #[test]
    fn idle_expiry_follows_the_configured_minutes() {
        let start = Activity::now();
        assert!(!idle_expired(start, after(start, 59, 59), 1));
        assert!(idle_expired(start, after(start, 60, 60), 1));
        assert!(!idle_expired(start, after(start, 14 * 60, 14 * 60), 15));
    }

    #[test]
    fn sleep_is_counted_by_the_wall_clock() {
        // Mise en veille : l'horloge monotone (Linux) n'a presque pas avancé, l'horloge murale
        // de plusieurs heures. Le coffre doit se verrouiller.
        let start = Activity::now();
        assert!(idle_expired(start, after(start, 60, 4 * 3600), 15));
    }

    #[test]
    fn a_wall_clock_going_back_never_prevents_locking() {
        let start = Activity::now();
        assert!(!idle_expired(start, after(start, 10, -3600), 1));
        assert!(idle_expired(start, after(start, 60, -3600), 1));
    }

    #[tokio::test]
    async fn locked_state_refuses_access_and_known_hosts() {
        let dir = tempfile::tempdir().unwrap();
        let state = VaultState::new(dir.path().join("vault.younji"), dir.path().join("kh"));
        assert_eq!(state.status().unwrap(), VaultStatus::Missing);
        assert!(!state.is_unlocked());
        assert_eq!(state.read(|_| Ok(())).unwrap_err(), AppError::VAULT_LOCKED);
        assert_eq!(
            state.update_blocking(|_| Ok(())).await.unwrap_err(),
            AppError::VAULT_LOCKED
        );
        let storage = state.known_hosts_storage();
        assert_eq!(storage.load(), Err(SshError::KnownHostsIo));
        assert!(!state.lock(LockReason::Manual));
        assert!(!state.idle_lock_due(Activity::now()));
    }

    #[tokio::test]
    async fn create_imports_legacy_known_hosts_then_lock_and_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("known_hosts");
        std::fs::write(&legacy, "srv.lab ssh-ed25519 AAAA\n").unwrap();
        let state = VaultState::new(dir.path().join("vault.younji"), legacy.clone());
        let password = "cheval agrafe batterie correct 42";

        assert_eq!(
            state
                .create(SecretString::from("faible"))
                .await
                .unwrap_err(),
            AppError::from(VaultError::PasswordTooShort)
        );
        state.create(SecretString::from(password)).await.unwrap();
        assert_eq!(state.status().unwrap(), VaultStatus::Unlocked);
        assert!(
            !legacy.exists(),
            "l'ancien fichier en clair doit disparaître"
        );
        let storage = state.known_hosts_storage();
        assert_eq!(storage.load().unwrap(), "srv.lab ssh-ed25519 AAAA\n");
        storage.store(String::new()).unwrap();

        assert!(state.lock(LockReason::Manual));
        assert_eq!(state.status().unwrap(), VaultStatus::Locked);
        assert_eq!(
            state
                .unlock(SecretString::from("mauvais"))
                .await
                .unwrap_err(),
            AppError::from(VaultError::WrongPasswordOrCorrupted)
        );
        state.unlock(SecretString::from(password)).await.unwrap();
        assert_eq!(storage.load().unwrap(), "");
        // Juste après une activité, pas de verrouillage automatique.
        assert!(!state.idle_lock_due(Activity::now()));
        assert!(state.idle_lock_due(after(Activity::now(), 15 * 60, 15 * 60)));

        // Un mauvais mot de passe actuel compte comme un échec de déverrouillage.
        let wrong = AppError::from(VaultError::WrongPasswordOrCorrupted);
        let new = || SecretString::from("Tq9#mZ2!vLp8@xR-nouveau");
        assert_eq!(
            state
                .change_password(SecretString::from("mauvais"), new())
                .await
                .unwrap_err(),
            wrong
        );
        assert_eq!(*state.attempts.lock().await, 1);
        state
            .change_password(SecretString::from(password), new())
            .await
            .unwrap();
        assert_eq!(*state.attempts.lock().await, 0);
    }
}
