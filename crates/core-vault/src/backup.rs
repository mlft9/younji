//! Sauvegarde chiffrée du coffre (VLT-06, SECURITE.md §3.7, ADR-011).
//!
//! Même format que le coffre (`format.rs`) avec la magie `YNJB`, un **sel et une DEK neufs** et
//! une phrase de passe **distincte** du mot de passe maître :
//!
//! ```text
//! "YNJB" | version | kdf | m | t | p | sel | nonce_dek | DEK chiffrée | nonce_data | longueur | données
//! ```
//!
//! L'en-tête sert d'AAD : un octet modifié n'importe où (en-tête, nonces, DEK, données, tag)
//! fait échouer l'ouverture avec [`VaultError::WrongPasswordOrCorrupted`]. Les paramètres
//! Argon2 et la longueur annoncée sont bornés avant toute allocation, comme pour le coffre.

use secrecy::SecretString;

use crate::crypto;
use crate::format::{BACKUP_MAGIC, EncryptedVault, Header, KdfParams, MAX_PAYLOAD_LEN};
use crate::model::VaultData;
use crate::{VaultError, policy};

/// Chiffre les données en sauvegarde (Argon2id : opération lente, hors exécuteur async).
/// La phrase de passe doit respecter la politique du mot de passe maître.
pub fn export_backup(data: &VaultData, passphrase: &SecretString) -> Result<Vec<u8>, VaultError> {
    policy::check_master_password(passphrase)?;
    let header = Header {
        params: KdfParams::DEFAULT,
        salt: crypto::random()?,
    };
    let header_bytes = header.encode_with(BACKUP_MAGIC);
    let kek = crypto::derive_kek(passphrase, &header)?;
    let dek = crypto::random_key()?;
    let dek_nonce = crypto::random()?;
    let wrapped_dek = crypto::wrap_key(&kek, &dek_nonce, &header_bytes, &dek)?;
    let data_nonce = crypto::random()?;
    let plaintext = crate::store::serialize(data)?;
    let ciphertext = crypto::seal(&dek, &data_nonce, &header_bytes, plaintext)?;
    if ciphertext.len() as u64 > MAX_PAYLOAD_LEN {
        return Err(VaultError::Io);
    }
    Ok(EncryptedVault {
        header,
        header_bytes,
        dek_nonce,
        wrapped_dek,
        data_nonce,
        ciphertext,
    }
    .to_bytes())
}

/// Ouvre une sauvegarde : format et bornes vérifiés avant toute allocation, déchiffrement
/// authentifié, puis revalidation de chaque entrée comme une saisie.
pub fn read_backup(bytes: &[u8], passphrase: &SecretString) -> Result<VaultData, VaultError> {
    policy::check_unlock_password(passphrase)?;
    let mut reader = bytes;
    let file = EncryptedVault::read_from_with(&mut reader, bytes.len() as u64, BACKUP_MAGIC)?;
    let kek = crypto::derive_kek(passphrase, &file.header)?;
    let dek = crypto::unwrap_key(&kek, &file.dek_nonce, &file.header_bytes, &file.wrapped_dek)?;
    let plaintext = crypto::open(&dek, &file.data_nonce, &file.header_bytes, file.ciphertext)?;
    let data: VaultData =
        serde_json::from_slice(&plaintext).map_err(|_| VaultError::WrongPasswordOrCorrupted)?;
    data.validate_all()?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;
    use crate::model::{HostInput, HostProtocol, IdentityInput, PasswordChange, SnippetInput};

    fn secret(text: &str) -> SecretString {
        SecretString::from(text.to_owned())
    }

    const PASS: &str = "Phrase de sauvegarde tres longue 2026 !";

    fn sample() -> VaultData {
        let mut data = VaultData::default();
        let group = data.upsert_group(None, "Lab").unwrap();
        let identity = data
            .upsert_identity(IdentityInput {
                id: None,
                name: "admin".into(),
                username: "root".into(),
                password: PasswordChange::Set(secret("mot-de-passe-ssh")),
                key_id: None,
            })
            .unwrap();
        data.upsert_host(HostInput {
            id: None,
            name: "pve".into(),
            address: "pve.lab".into(),
            port: 22,
            username: String::new(),
            identity_id: Some(identity),
            group_id: Some(group),
            tags: vec!["lab".into()],
            notes: "notes".into(),
            protocol: HostProtocol::default(),
        })
        .unwrap();
        data.upsert_snippet(SnippetInput {
            id: None,
            name: "uptime".into(),
            content: "uptime".into(),
            description: String::new(),
        })
        .unwrap();
        data.set_known_hosts("pve.lab ssh-ed25519 AAAA\n".into())
            .unwrap();
        data
    }

    fn json(data: &VaultData) -> String {
        String::from_utf8(crate::store::serialize(data).unwrap().to_vec()).unwrap()
    }

    #[test]
    fn round_trip_gives_identical_data() {
        let data = sample();
        let bytes = export_backup(&data, &secret(PASS)).unwrap();
        assert_eq!(&bytes[..4], b"YNJB");
        let back = read_backup(&bytes, &secret(PASS)).unwrap();
        assert_eq!(json(&back), json(&data));
        let password = back.identities()[0].password.as_ref().unwrap();
        assert_eq!(password.expose_secret(), "mot-de-passe-ssh");
        // Le clair n'apparaît pas dans le fichier.
        assert!(!bytes.windows(16).any(|w| w == b"mot-de-passe-ssh"));
    }

    #[test]
    fn any_modified_byte_is_rejected() {
        let bytes = export_backup(&sample(), &secret(PASS)).unwrap();
        // Un octet par zone : magie, paramètres, sel, nonce DEK, DEK, nonce données, longueur,
        // données, tag final.
        for index in [0, 8, 20, 40, 70, 110, 140, 160, bytes.len() - 1] {
            let mut tampered = bytes.clone();
            tampered[index] ^= 0x01;
            assert!(
                read_backup(&tampered, &secret(PASS)).is_err(),
                "octet {index}"
            );
        }
        let mut truncated = bytes.clone();
        truncated.pop();
        assert_eq!(
            read_backup(&truncated, &secret(PASS)).unwrap_err(),
            VaultError::InvalidFile
        );
    }

    #[test]
    fn wrong_passphrase_gives_the_generic_error() {
        let bytes = export_backup(&sample(), &secret(PASS)).unwrap();
        assert_eq!(
            read_backup(&bytes, &secret("Une autre phrase bien longue 42")).unwrap_err(),
            VaultError::WrongPasswordOrCorrupted
        );
    }

    #[test]
    fn weak_passphrases_are_refused() {
        assert_eq!(
            export_backup(&sample(), &secret("court")).unwrap_err(),
            VaultError::PasswordTooShort
        );
        assert_eq!(
            export_backup(&sample(), &secret("aaaaaaaaaaaaaaaa")).unwrap_err(),
            VaultError::PasswordTooWeak
        );
    }

    #[test]
    fn merge_adds_without_overwriting() {
        let source = sample();
        let mut target = source.clone();
        // Mêmes identifiants : rien n'est ajouté ni modifié.
        let counts = target.merge_from(&source).unwrap();
        assert_eq!(
            counts.hosts + counts.groups + counts.identities + counts.snippets,
            0
        );
        assert_eq!(json(&target), json(&source));

        let mut empty = VaultData::default();
        let counts = empty.merge_from(&source).unwrap();
        assert_eq!(
            (
                counts.hosts,
                counts.groups,
                counts.identities,
                counts.snippets
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(
            empty.hosts()[0].identity_id,
            Some(source.identities()[0].id)
        );
        // Les clés d'hôte ne sont pas fusionnées ici (règle de conflit appliquée par l'appelant).
        assert_eq!(empty.known_hosts(), "");

        // Autre coffre avec le même hôte créé à la main (autre identifiant) : pas de doublon.
        let mut other = sample();
        let counts = other.merge_from(&source).unwrap();
        assert_eq!(counts.hosts, 0);
        assert_eq!(other.hosts().len(), 1);
    }

    #[test]
    fn invalid_entries_in_a_backup_are_refused() {
        let mut data = sample();
        // Contenu fabriqué hors de l'application (avec la bonne phrase de passe) : hôte invalide.
        data.hosts_mut()[0].address = "a b".into();
        let bytes = export_backup(&data, &secret(PASS)).unwrap();
        assert_eq!(
            read_backup(&bytes, &secret(PASS)).unwrap_err(),
            VaultError::InvalidFile
        );
        // Verrouillage automatique désactivé de fait (délai de 8 000 ans) ou permanent (0).
        for minutes in [u32::MAX, 0, 7] {
            let mut data = sample();
            data.settings_mut().auto_lock_minutes = minutes;
            let bytes = export_backup(&data, &secret(PASS)).unwrap();
            assert_eq!(
                read_backup(&bytes, &secret(PASS)).unwrap_err(),
                VaultError::InvalidFile,
                "{minutes}"
            );
        }
        let mut data = sample();
        data.hosts_mut()[0].identity_id = Some(uuid::Uuid::new_v4());
        let bytes = export_backup(&data, &secret(PASS)).unwrap();
        assert_eq!(
            read_backup(&bytes, &secret(PASS)).unwrap_err(),
            VaultError::InvalidFile
        );
    }
}
