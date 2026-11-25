//! Fichier du coffre : création, déverrouillage, sauvegarde et changement de mot de passe
//! (VLT-01, VLT-02, VLT-04 ; SECURITE.md §3.1, §3.5, §3.6).
//!
//! Un [`Vault`] n'existe que **déverrouillé** : il détient la DEK et les données en clair. Le
//! verrouiller, c'est le détruire : la DEK (`SecretBox`) et les secrets (`SecretString`) sont
//! effacés de la mémoire à la destruction.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use secrecy::SecretString;
use zeroize::Zeroizing;

use crate::crypto::{self, SecretKey};
use crate::format::{
    EncryptedVault, FIXED_LEN, HEADER_LEN, Header, KdfParams, MAX_PAYLOAD_LEN, NONCE_LEN, TAG_LEN,
    WRAPPED_DEK_LEN,
};
use crate::model::VaultData;
use crate::{VaultError, policy};

/// État du fichier du coffre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultFileStatus {
    /// Aucun coffre : il faut le créer.
    Missing,
    /// Un coffre existe : il faut le déverrouiller.
    Present,
}

/// État du fichier à l'emplacement donné.
pub fn file_status(path: &Path) -> VaultFileStatus {
    if path.exists() {
        VaultFileStatus::Present
    } else {
        VaultFileStatus::Missing
    }
}

/// Chemin de la copie de sauvegarde (`vault.younji.bak`).
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// Coffre déverrouillé.
pub struct Vault {
    path: PathBuf,
    header_bytes: [u8; HEADER_LEN],
    header: Header,
    dek_nonce: [u8; NONCE_LEN],
    wrapped_dek: [u8; WRAPPED_DEK_LEN],
    dek: SecretKey,
    data: VaultData,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("data", &self.data)
            .finish_non_exhaustive()
    }
}

/// Clés et en-tête d'un coffre, avant écriture.
struct Keys {
    header: Header,
    header_bytes: [u8; HEADER_LEN],
    dek_nonce: [u8; NONCE_LEN],
    wrapped_dek: [u8; WRAPPED_DEK_LEN],
    dek: SecretKey,
}

/// Nouveau sel, nouvelle DEK, DEK chiffrée par la KEK dérivée du mot de passe.
fn fresh_keys(password: &SecretString) -> Result<Keys, VaultError> {
    let header = Header {
        params: KdfParams::DEFAULT,
        salt: crypto::random()?,
    };
    let header_bytes = header.encode();
    let kek = crypto::derive_kek(password, &header)?;
    let dek = crypto::random_key()?;
    let dek_nonce = crypto::random()?;
    let wrapped_dek = crypto::wrap_key(&kek, &dek_nonce, &header_bytes, &dek)?;
    Ok(Keys {
        header,
        header_bytes,
        dek_nonce,
        wrapped_dek,
        dek,
    })
}

impl Vault {
    /// Crée un coffre (Argon2id : opération lente, à appeler hors de l'exécuteur async).
    /// N'écrase jamais un fichier existant.
    pub fn create(
        path: impl Into<PathBuf>,
        password: &SecretString,
        data: VaultData,
    ) -> Result<Self, VaultError> {
        let path = path.into();
        policy::check_master_password(password)?;
        if file_status(&path) == VaultFileStatus::Present {
            return Err(VaultError::AlreadyExists);
        }
        // Coffre absent mais copie de sauvegarde présente : créer un coffre neuf écraserait
        // bientôt cette copie, peut-être la seule restante. L'utilisateur doit la restaurer.
        if backup_path(&path).exists() {
            return Err(VaultError::BackupOnly);
        }
        let keys = fresh_keys(password)?;
        let vault = Self::from_keys(path, keys, data);
        vault.write(WriteMode::CreateNew)?;
        tracing::info!("coffre créé");
        Ok(vault)
    }

    /// Déverrouille le coffre. Toute erreur de déchiffrement donne
    /// [`VaultError::WrongPasswordOrCorrupted`], sans distinguer les cas.
    pub fn unlock(path: impl Into<PathBuf>, password: &SecretString) -> Result<Self, VaultError> {
        let path = path.into();
        policy::check_unlock_password(password)?;
        let file = EncryptedVault::read(&path)?;
        let kek = crypto::derive_kek(password, &file.header)?;
        let dek = crypto::unwrap_key(&kek, &file.dek_nonce, &file.header_bytes, &file.wrapped_dek)?;
        let plaintext = crypto::open(&dek, &file.data_nonce, &file.header_bytes, file.ciphertext)?;
        // Les données sont authentifiées : un JSON illisible ne peut venir que d'un fichier
        // fabriqué avec la bonne clé, ou d'un bogue. Même code, pour ne rien distinguer.
        let data: VaultData =
            serde_json::from_slice(&plaintext).map_err(|_| VaultError::WrongPasswordOrCorrupted)?;
        data.check_loaded()?;
        tracing::info!("coffre déverrouillé");
        Ok(Self {
            path,
            header_bytes: file.header_bytes,
            header: file.header,
            dek_nonce: file.dek_nonce,
            wrapped_dek: file.wrapped_dek,
            dek,
            data,
        })
    }

    fn from_keys(path: PathBuf, keys: Keys, data: VaultData) -> Self {
        Self {
            path,
            header_bytes: keys.header_bytes,
            header: keys.header,
            dek_nonce: keys.dek_nonce,
            wrapped_dek: keys.wrapped_dek,
            dek: keys.dek,
            data,
        }
    }

    /// Données déchiffrées.
    pub fn data(&self) -> &VaultData {
        &self.data
    }

    /// Données déchiffrées, modifiables. Penser à [`Vault::save`] ensuite.
    pub fn data_mut(&mut self) -> &mut VaultData {
        &mut self.data
    }

    /// Chemin du fichier.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Exporte une sauvegarde chiffrée (VLT-06). La phrase de passe doit être **distincte** du
    /// mot de passe maître : refusée si elle ouvre ce coffre. Deux dérivations Argon2id : à
    /// appeler hors de l'exécuteur async.
    pub fn export_backup(&self, passphrase: &SecretString) -> Result<Vec<u8>, VaultError> {
        policy::check_master_password(passphrase)?;
        let kek = crypto::derive_kek(passphrase, &self.header)?;
        if crypto::unwrap_key(&kek, &self.dek_nonce, &self.header_bytes, &self.wrapped_dek).is_ok()
        {
            return Err(VaultError::BackupPassphraseReused);
        }
        crate::backup::export_backup(&self.data, passphrase)
    }

    /// Rechiffre les données avec un nouveau nonce et réécrit le fichier de façon atomique
    /// (la version précédente devient `.bak`).
    pub fn save(&self) -> Result<(), VaultError> {
        self.write(WriteMode::Replace)
    }

    /// Modifie les données, puis sauvegarde. En cas d'échec (validation ou écriture), les
    /// données en mémoire reviennent à leur état précédent.
    pub fn update<T>(
        &mut self,
        change: impl FnOnce(&mut VaultData) -> Result<T, VaultError>,
    ) -> Result<T, VaultError> {
        let before = self.data.clone();
        let result = change(&mut self.data).and_then(|value| self.save().map(|()| value));
        if result.is_err() {
            self.data = before;
        }
        result
    }

    /// Change le mot de passe maître (VLT-04) : vérifie l'actuel, puis **nouveau sel, nouvelle
    /// DEK** et rechiffrement complet (SECURITE.md §3.1). En cas d'échec, rien ne change.
    pub fn change_password(
        &mut self,
        current: &SecretString,
        new: &SecretString,
    ) -> Result<(), VaultError> {
        policy::check_unlock_password(current)?;
        let kek = crypto::derive_kek(current, &self.header)?;
        crypto::unwrap_key(&kek, &self.dek_nonce, &self.header_bytes, &self.wrapped_dek)?;
        policy::check_master_password(new)?;
        let keys = fresh_keys(new)?;
        let candidate = Self::from_keys(self.path.clone(), keys, self.data.clone());
        candidate.write(WriteMode::Replace)?;
        // Le fichier principal utilise désormais les nouvelles clés : le coffre en mémoire aussi,
        // sinon la prochaine sauvegarde reviendrait silencieusement à l'ancien mot de passe.
        *self = candidate;
        tracing::info!("mot de passe maître changé");
        // La copie de sauvegarde contient encore l'ancienne version, chiffrée avec l'ancien sel et
        // l'ancienne DEK : elle s'ouvrirait avec l'ancien mot de passe. On la remplace par une
        // copie du nouveau fichier ; à défaut, on la supprime : mieux vaut aucune copie qu'une
        // copie lisible avec l'ancien mot de passe.
        if let Err(error) = self.write_backup_of_current() {
            tracing::warn!(code = error.code(), "copie de sauvegarde non renouvelée");
            match fs::remove_file(backup_path(&self.path)) {
                Ok(()) => tracing::warn!("ancienne copie de sauvegarde supprimée"),
                Err(error) => tracing::error!(%error, "ancienne copie de sauvegarde conservée"),
            }
        }
        Ok(())
    }

    fn write_backup_of_current(&self) -> Result<(), VaultError> {
        let dir = parent_dir(&self.path);
        backup(&self.path, dir)?;
        sync_dir(dir);
        Ok(())
    }

    fn write(&self, mode: WriteMode) -> Result<(), VaultError> {
        let plaintext = serialize(&self.data)?;
        let data_nonce = crypto::random()?;
        let ciphertext = crypto::seal(&self.dek, &data_nonce, &self.header_bytes, plaintext)?;
        if ciphertext.len() as u64 > MAX_PAYLOAD_LEN {
            tracing::error!("coffre trop volumineux pour être écrit");
            return Err(VaultError::Io);
        }
        let file = EncryptedVault {
            header: self.header,
            header_bytes: self.header_bytes,
            dek_nonce: self.dek_nonce,
            wrapped_dek: self.wrapped_dek,
            data_nonce,
            ciphertext,
        };
        write_file(&self.path, &file.to_bytes(), mode)
    }
}

/// Compte les octets écrits, sans les garder.
struct Counter(usize);

impl Write for Counter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Sérialise les données dans un tampon effacé à sa destruction. La taille exacte est mesurée
/// d'abord, pour qu'aucune réallocation ne laisse de copie partielle du clair en mémoire.
pub(crate) fn serialize(data: &VaultData) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    let failed = |_| VaultError::Io;
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, data).map_err(failed)?;
    let mut buffer = Zeroizing::new(Vec::with_capacity(counter.0 + TAG_LEN));
    serde_json::to_writer(&mut *buffer, data).map_err(failed)?;
    Ok(buffer)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteMode {
    /// Création : échoue si le fichier existe.
    CreateNew,
    /// Remplacement, avec copie de la version précédente en `.bak`.
    Replace,
}

fn io_error(error: &io::Error) -> VaultError {
    tracing::warn!(%error, "écriture du coffre impossible");
    VaultError::Io
}

/// Écriture atomique : fichier temporaire (0600) dans le même dossier, `fsync`, renommage.
fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn write_file(path: &Path, bytes: &[u8], mode: WriteMode) -> Result<(), VaultError> {
    let dir = parent_dir(path);
    create_private_dir(dir).map_err(|e| io_error(&e))?;
    if mode == WriteMode::Replace {
        backup(path, dir)?;
    }
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|e| io_error(&e))?;
    temp.write_all(bytes).map_err(|e| io_error(&e))?;
    temp.as_file().sync_all().map_err(|e| io_error(&e))?;
    match mode {
        WriteMode::CreateNew => temp.persist_noclobber(path).map_err(|e| {
            if e.error.kind() == io::ErrorKind::AlreadyExists {
                VaultError::AlreadyExists
            } else {
                io_error(&e.error)
            }
        })?,
        WriteMode::Replace => temp.persist(path).map_err(|e| io_error(&e.error))?,
    };
    sync_dir(dir);
    Ok(())
}

/// Copie atomique de la version actuelle vers `.bak` (s'il y a une version actuelle).
fn backup(path: &Path, dir: &Path) -> Result<(), VaultError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(&error)),
    };
    let limit = FIXED_LEN as u64 + MAX_PAYLOAD_LEN;
    let mut current = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut current)
        .map_err(|e| io_error(&e))?;
    if current.len() as u64 > limit {
        return Err(VaultError::InvalidFile);
    }
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|e| io_error(&e))?;
    temp.write_all(&current).map_err(|e| io_error(&e))?;
    temp.as_file().sync_all().map_err(|e| io_error(&e))?;
    temp.persist(backup_path(path))
        .map_err(|e| io_error(&e.error))?;
    Ok(())
}

/// Crée le dossier avec des droits restrictifs (0700 sous Linux). Un dossier déjà présent
/// (créé par exemple par la WebView en 0755) est ramené à 0700 : c'est le dossier de données de
/// l'application, personne d'autre n'a à y lire.
fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        builder.mode(0o700);
        builder.create(dir)?;
        let current = fs::metadata(dir)?.permissions().mode() & 0o777;
        if current != 0o700 {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    builder.create(dir)
}

/// Rend le renommage durable (Linux). Sans effet sous Windows.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(handle) = fs::File::open(dir) {
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;
    use crate::format::SALT_LEN;
    use crate::model::{HostInput, HostProtocol, IdentityInput, PasswordChange};

    const PASSWORD: &str = "cheval agrafe batterie correct 42";
    const OTHER: &str = "Tq9#mZ2!vLp8@xR-nouveau";

    fn secret(s: &str) -> SecretString {
        SecretString::from(s)
    }

    fn sample_data() -> VaultData {
        let mut data = VaultData::default();
        let identity = data
            .upsert_identity(IdentityInput {
                id: None,
                name: "Compte du lab".into(),
                username: "utilisateur-confidentiel".into(),
                password: PasswordChange::Set(secret("mot-de-passe-ssh")),
                key_id: None,
            })
            .unwrap();
        data.upsert_host(HostInput {
            id: None,
            name: "Serveur secret".into(),
            address: "serveur-secret.example".into(),
            port: 2222,
            username: String::new(),
            identity_id: Some(identity),
            group_id: None,
            tags: vec![],
            notes: "notes privées".into(),
            protocol: HostProtocol::default(),
        })
        .unwrap();
        data
    }

    fn create(dir: &Path) -> (PathBuf, Vault) {
        let path = dir.join("vault.younji");
        let vault = Vault::create(&path, &secret(PASSWORD), sample_data()).unwrap();
        (path, vault)
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn backup_passphrase_must_differ_from_the_master_password() {
        let dir = tempfile::tempdir().unwrap();
        let (_, vault) = create(dir.path());
        assert_eq!(
            vault.export_backup(&secret(PASSWORD)).unwrap_err(),
            VaultError::BackupPassphraseReused
        );
        let bytes = vault.export_backup(&secret(OTHER)).unwrap();
        let back = crate::backup::read_backup(&bytes, &secret(OTHER)).unwrap();
        assert_eq!(back.hosts().len(), vault.data().hosts().len());
        // Une sauvegarde ne s'ouvre pas comme un coffre, ni l'inverse.
        let path = dir.path().join("copie.younji");
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            Vault::unlock(&path, &secret(OTHER)).unwrap_err(),
            VaultError::InvalidFile
        );
        let vault_bytes = std::fs::read(vault.path()).unwrap();
        assert_eq!(
            crate::backup::read_backup(&vault_bytes, &secret(PASSWORD)).unwrap_err(),
            VaultError::InvalidFile
        );
    }

    #[test]
    fn create_then_unlock_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("vault.younji");
        assert_eq!(file_status(&path), VaultFileStatus::Missing);
        Vault::create(&path, &secret(PASSWORD), sample_data()).unwrap();
        assert_eq!(file_status(&path), VaultFileStatus::Present);

        let vault = Vault::unlock(&path, &secret(PASSWORD)).unwrap();
        assert_eq!(vault.data().hosts()[0].address, "serveur-secret.example");
        let password = vault.data().identities()[0].password.as_ref().unwrap();
        assert_eq!(password.expose_secret(), "mot-de-passe-ssh");
    }

    #[test]
    fn creation_never_overwrites_and_enforces_the_policy() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _) = create(dir.path());
        assert_eq!(
            Vault::create(&path, &secret(PASSWORD), VaultData::default()).err(),
            Some(VaultError::AlreadyExists)
        );
        let other = dir.path().join("autre.younji");
        assert_eq!(
            Vault::create(&other, &secret("motdepasse12"), VaultData::default()).err(),
            Some(VaultError::PasswordTooWeak)
        );
        assert_eq!(
            Vault::create(&other, &secret("court"), VaultData::default()).err(),
            Some(VaultError::PasswordTooShort)
        );
        assert_eq!(file_status(&other), VaultFileStatus::Missing);
    }

    #[test]
    fn creation_refuses_to_bury_an_orphan_backup() {
        let dir = tempfile::tempdir().unwrap();
        let (path, vault) = create(dir.path());
        vault.save().unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(
            Vault::create(&path, &secret(PASSWORD), VaultData::default()).err(),
            Some(VaultError::BackupOnly)
        );
        // La copie reste intacte et restaurable.
        assert!(Vault::unlock(backup_path(&path), &secret(PASSWORD)).is_ok());
    }

    #[test]
    fn wrong_password_gives_the_generic_error() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _) = create(dir.path());
        assert_eq!(
            Vault::unlock(&path, &secret("pas le bon mot de passe")).err(),
            Some(VaultError::WrongPasswordOrCorrupted)
        );
        assert_eq!(
            Vault::unlock(dir.path().join("absent"), &secret(PASSWORD)).err(),
            Some(VaultError::NotFound)
        );
    }

    #[test]
    fn file_contains_no_readable_data() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _) = create(dir.path());
        let bytes = fs::read(&path).unwrap();
        for needle in [
            "serveur-secret.example",
            "Serveur secret",
            "utilisateur-confidentiel",
            "mot-de-passe-ssh",
            "notes privées",
            "hosts",
            "username",
        ] {
            assert!(!contains(&bytes, needle.as_bytes()), "{needle} lisible");
        }
    }

    #[test]
    fn any_modified_byte_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _) = create(dir.path());
        let original = fs::read(&path).unwrap();
        let len = original.len();
        // Une position dans chaque champ : sel, nonce et DEK chiffrée, nonce des données,
        // début, milieu et fin (tag) du chiffré.
        let positions = [
            HEADER_LEN - SALT_LEN,
            HEADER_LEN - 1,
            HEADER_LEN,
            HEADER_LEN + NONCE_LEN + 5,
            HEADER_LEN + NONCE_LEN + WRAPPED_DEK_LEN + 3,
            FIXED_LEN,
            FIXED_LEN + (len - FIXED_LEN) / 2,
            len - 1,
        ];
        for position in positions {
            let mut bytes = original.clone();
            bytes[position] ^= 0x01;
            fs::write(&path, &bytes).unwrap();
            assert_eq!(
                Vault::unlock(&path, &secret(PASSWORD)).err(),
                Some(VaultError::WrongPasswordOrCorrupted),
                "octet {position}"
            );
        }
        // Paramètres modifiés, mais toujours dans les bornes : la KEK change, l'AAD aussi.
        let mut bytes = original.clone();
        bytes[7] ^= 0x01;
        fs::write(&path, &bytes).unwrap();
        assert_eq!(
            Vault::unlock(&path, &secret(PASSWORD)).err(),
            Some(VaultError::WrongPasswordOrCorrupted)
        );
        // Champs de structure : refus explicite, avant toute dérivation.
        for (position, expected) in [
            (0, VaultError::InvalidFile),
            (4, VaultError::UnsupportedVersion),
            (FIXED_LEN - 8, VaultError::InvalidFile),
        ] {
            let mut bytes = original.clone();
            bytes[position] ^= 0x01;
            fs::write(&path, &bytes).unwrap();
            assert_eq!(
                Vault::unlock(&path, &secret(PASSWORD)).err(),
                Some(expected)
            );
        }
    }

    #[test]
    fn each_save_uses_a_new_nonce_and_keeps_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut vault) = create(dir.path());
        let first = fs::read(&path).unwrap();
        vault.save().unwrap();
        let second = fs::read(&path).unwrap();
        let nonce = HEADER_LEN + NONCE_LEN + WRAPPED_DEK_LEN;
        // En-tête et DEK chiffrée identiques, nonce et chiffré des données différents.
        assert_eq!(first[..nonce], second[..nonce]);
        assert_ne!(
            first[nonce..nonce + NONCE_LEN],
            second[nonce..nonce + NONCE_LEN]
        );
        assert_ne!(first[FIXED_LEN..], second[FIXED_LEN..]);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), first);

        vault
            .update(|data| data.upsert_group(None, "Groupe"))
            .unwrap();
        assert_eq!(fs::read(backup_path(&path)).unwrap(), second);
        let reopened = Vault::unlock(&path, &secret(PASSWORD)).unwrap();
        assert_eq!(reopened.data().groups()[0].name, "Groupe");
    }

    #[test]
    fn failed_update_leaves_data_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let (_, mut vault) = create(dir.path());
        let result = vault.update(|data| {
            data.upsert_group(None, "Ajouté")?;
            data.upsert_group(None, "")
        });
        assert_eq!(result, Err(VaultError::InvalidName));
        assert!(vault.data().groups().is_empty());
    }

    #[test]
    fn changing_the_password_rotates_salt_and_dek() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut vault) = create(dir.path());
        let before = fs::read(&path).unwrap();

        assert_eq!(
            vault.change_password(&secret("mauvais actuel"), &secret(OTHER)),
            Err(VaultError::WrongPasswordOrCorrupted)
        );
        assert_eq!(
            vault.change_password(&secret(PASSWORD), &secret("faible")),
            Err(VaultError::PasswordTooShort)
        );
        assert_eq!(fs::read(&path).unwrap(), before);

        vault
            .change_password(&secret(PASSWORD), &secret(OTHER))
            .unwrap();
        let after = fs::read(&path).unwrap();
        // Nouveau sel, nouvelle DEK chiffrée.
        assert_ne!(
            before[HEADER_LEN - SALT_LEN..HEADER_LEN],
            after[HEADER_LEN - SALT_LEN..HEADER_LEN]
        );
        assert_ne!(
            before[HEADER_LEN + NONCE_LEN..HEADER_LEN + NONCE_LEN + WRAPPED_DEK_LEN],
            after[HEADER_LEN + NONCE_LEN..HEADER_LEN + NONCE_LEN + WRAPPED_DEK_LEN]
        );
        assert_eq!(
            Vault::unlock(&path, &secret(PASSWORD)).err(),
            Some(VaultError::WrongPasswordOrCorrupted)
        );
        let reopened = Vault::unlock(&path, &secret(OTHER)).unwrap();
        assert_eq!(reopened.data().hosts().len(), 1);
        // La copie de sauvegarde est rechiffrée aussi : l'ancien mot de passe n'ouvre plus rien.
        assert_eq!(
            Vault::unlock(backup_path(&path), &secret(PASSWORD)).err(),
            Some(VaultError::WrongPasswordOrCorrupted)
        );
        assert!(Vault::unlock(backup_path(&path), &secret(OTHER)).is_ok());
    }

    #[test]
    fn debug_shows_no_secret() {
        let dir = tempfile::tempdir().unwrap();
        let (_, vault) = create(dir.path());
        let shown = format!("{vault:?}");
        assert!(!shown.contains("mot-de-passe-ssh"));
        assert!(!shown.contains("serveur-secret"));
    }

    #[cfg(unix)]
    #[test]
    fn existing_directory_is_made_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("donnees");
        fs::create_dir(&data).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).unwrap();
        create(&data);
        let mode = fs::metadata(&data).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn files_are_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (path, vault) = create(&dir.path().join("donnees"));
        vault.save().unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&backup_path(&path)), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }
}
