//! Format de fichier v1 du coffre (SECURITE.md §3.3).
//!
//! ```text
//! "YNJV" (4) | version u16 = 1 | kdf_id u8 = 1 (Argon2id) | m_kib u32 | t u32 | p u32 | sel (16)
//! | nonce_dek (24) | DEK chiffrée + tag (48)
//! | nonce_data (24) | longueur u64 | données chiffrées + tag
//! ```
//!
//! Entiers en little-endian. L'**en-tête** (de la magie au sel, 35 octets) sert de données
//! associées (AAD) aux deux chiffrements : modifier la version, les paramètres ou le sel rend le
//! fichier indéchiffrable. La longueur annoncée est vérifiée **avant toute allocation** : elle
//! doit correspondre à la taille réelle du fichier et rester sous [`MAX_PAYLOAD_LEN`].

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crate::VaultError;

/// Magie du fichier de coffre.
pub const MAGIC: [u8; 4] = *b"YNJV";
/// Magie d'une sauvegarde chiffrée (export, VLT-06, SECURITE.md §3.7) : un coffre ne peut pas
/// être lu comme une sauvegarde, ni l'inverse.
pub const BACKUP_MAGIC: [u8; 4] = *b"YNJB";
/// Version du format de fichier.
pub const FORMAT_VERSION: u16 = 1;
/// Identifiant de la fonction de dérivation : Argon2id.
pub const KDF_ARGON2ID: u8 = 1;

/// Longueur du sel.
pub const SALT_LEN: usize = 16;
/// Longueur d'un nonce XChaCha20-Poly1305.
pub const NONCE_LEN: usize = 24;
/// Longueur d'une clé (KEK ou DEK).
pub const KEY_LEN: usize = 32;
/// Longueur du tag Poly1305.
pub const TAG_LEN: usize = 16;
/// Longueur de l'en-tête authentifié (magie → sel).
pub const HEADER_LEN: usize = 4 + 2 + 1 + 4 + 4 + 4 + SALT_LEN;
/// Longueur de la DEK chiffrée, tag compris.
pub const WRAPPED_DEK_LEN: usize = KEY_LEN + TAG_LEN;
/// Longueur de la partie fixe du fichier, avant les données chiffrées.
pub const FIXED_LEN: usize = HEADER_LEN + NONCE_LEN + WRAPPED_DEK_LEN + NONCE_LEN + 8;
/// Taille maximale des données chiffrées (tag compris).
pub const MAX_PAYLOAD_LEN: u64 = 64 * 1024 * 1024;

/// Paramètres Argon2id stockés dans l'en-tête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Mémoire, en Kio.
    pub m_kib: u32,
    /// Nombre de passes.
    pub t: u32,
    /// Parallélisme.
    pub p: u32,
}

impl KdfParams {
    /// Plancher (et valeurs par défaut) : m = 64 Mio, t = 3, p = 4 (SECURITE.md §3.2).
    pub const MIN: Self = Self {
        m_kib: 64 * 1024,
        t: 3,
        p: 4,
    };
    /// Plafond : un coffre piégé ne doit pas pouvoir saturer la mémoire ni le processeur.
    pub const MAX: Self = Self {
        m_kib: 1024 * 1024,
        t: 10,
        p: 16,
    };
    /// Paramètres utilisés pour un nouveau coffre.
    pub const DEFAULT: Self = Self::MIN;

    /// Vérifie que les paramètres sont entre le plancher et le plafond.
    pub fn validate(self) -> Result<Self, VaultError> {
        let within = |value: u32, min: u32, max: u32| (min..=max).contains(&value);
        if within(self.m_kib, Self::MIN.m_kib, Self::MAX.m_kib)
            && within(self.t, Self::MIN.t, Self::MAX.t)
            && within(self.p, Self::MIN.p, Self::MAX.p)
        {
            Ok(self)
        } else {
            Err(VaultError::KdfParamsRejected)
        }
    }
}

/// En-tête authentifié du coffre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Paramètres de dérivation.
    pub params: KdfParams,
    /// Sel de la dérivation (aléatoire, renouvelé à chaque changement de mot de passe).
    pub salt: [u8; SALT_LEN],
}

impl Header {
    /// Sérialise l'en-tête (35 octets), qui sert aussi d'AAD.
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        self.encode_with(MAGIC)
    }

    /// Sérialise l'en-tête avec une magie donnée ([`MAGIC`] ou [`BACKUP_MAGIC`]).
    pub fn encode_with(&self, magic: [u8; 4]) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        let mut w = Writer::new(&mut out);
        w.put(&magic);
        w.put(&FORMAT_VERSION.to_le_bytes());
        w.put(&[KDF_ARGON2ID]);
        w.put(&self.params.m_kib.to_le_bytes());
        w.put(&self.params.t.to_le_bytes());
        w.put(&self.params.p.to_le_bytes());
        w.put(&self.salt);
        out
    }

    /// Lit et valide l'en-tête : magie, version, fonction de dérivation, bornes des paramètres.
    pub fn decode(bytes: &[u8; HEADER_LEN]) -> Result<Self, VaultError> {
        Self::decode_with(bytes, MAGIC)
    }

    /// Comme [`Header::decode`], avec une magie attendue donnée.
    pub fn decode_with(bytes: &[u8; HEADER_LEN], magic: [u8; 4]) -> Result<Self, VaultError> {
        let mut r = Reader::new(bytes);
        if r.take::<4>() != magic {
            return Err(VaultError::InvalidFile);
        }
        if u16::from_le_bytes(r.take()) != FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion);
        }
        if r.take::<1>() != [KDF_ARGON2ID] {
            return Err(VaultError::UnsupportedVersion);
        }
        let params = KdfParams {
            m_kib: u32::from_le_bytes(r.take()),
            t: u32::from_le_bytes(r.take()),
            p: u32::from_le_bytes(r.take()),
        }
        .validate()?;
        Ok(Self {
            params,
            salt: r.take(),
        })
    }
}

/// Contenu d'un fichier de coffre, encore chiffré.
pub struct EncryptedVault {
    /// En-tête validé.
    pub header: Header,
    /// Octets exacts de l'en-tête, utilisés comme AAD.
    pub header_bytes: [u8; HEADER_LEN],
    /// Nonce de l'enveloppe de la DEK.
    pub dek_nonce: [u8; NONCE_LEN],
    /// DEK chiffrée par la KEK, tag compris.
    pub wrapped_dek: [u8; WRAPPED_DEK_LEN],
    /// Nonce des données.
    pub data_nonce: [u8; NONCE_LEN],
    /// Données chiffrées par la DEK, tag compris.
    pub ciphertext: Vec<u8>,
}

impl EncryptedVault {
    /// Assemble le fichier complet.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(FIXED_LEN + self.ciphertext.len());
        out.extend_from_slice(&self.header_bytes);
        out.extend_from_slice(&self.dek_nonce);
        out.extend_from_slice(&self.wrapped_dek);
        out.extend_from_slice(&self.data_nonce);
        out.extend_from_slice(&(self.ciphertext.len() as u64).to_le_bytes());
        out.extend_from_slice(&self.ciphertext);
        out
    }

    /// Lit un fichier de coffre. La taille réelle et la longueur annoncée sont contrôlées avant
    /// d'allouer le tampon des données.
    pub fn read(path: &Path) -> Result<Self, VaultError> {
        let mut file = File::open(path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => VaultError::NotFound,
            _ => io_error(&error),
        })?;
        let size = file.metadata().map_err(|e| io_error(&e))?.len();
        Self::read_from(&mut file, size)
    }

    /// Lit un coffre depuis un flux dont la taille totale est `size`.
    pub fn read_from(reader: &mut impl Read, size: u64) -> Result<Self, VaultError> {
        Self::read_from_with(reader, size, MAGIC)
    }

    /// Comme [`EncryptedVault::read_from`], avec une magie attendue donnée.
    pub fn read_from_with(
        reader: &mut impl Read,
        size: u64,
        magic: [u8; 4],
    ) -> Result<Self, VaultError> {
        if size < FIXED_LEN as u64 || size > FIXED_LEN as u64 + MAX_PAYLOAD_LEN {
            return Err(VaultError::InvalidFile);
        }
        let mut fixed = [0u8; FIXED_LEN];
        reader.read_exact(&mut fixed).map_err(|e| io_error(&e))?;
        let mut r = Reader::new(&fixed);
        let header_bytes: [u8; HEADER_LEN] = r.take();
        let header = Header::decode_with(&header_bytes, magic)?;
        let dek_nonce = r.take();
        let wrapped_dek = r.take();
        let data_nonce = r.take();
        let announced = u64::from_le_bytes(r.take());
        // Contrôle avant allocation : la longueur annoncée doit être exactement le reste du
        // fichier, au moins un tag, et sous le plafond.
        if announced != size - FIXED_LEN as u64
            || announced < TAG_LEN as u64
            || announced > MAX_PAYLOAD_LEN
        {
            return Err(VaultError::InvalidFile);
        }
        let len = usize::try_from(announced).map_err(|_| VaultError::InvalidFile)?;
        let mut ciphertext = vec![0u8; len];
        reader
            .read_exact(&mut ciphertext)
            .map_err(|e| io_error(&e))?;
        // Le fichier ne doit pas avoir grossi entre la mesure et la lecture.
        let mut extra = [0u8; 1];
        if reader.read(&mut extra).map_err(|e| io_error(&e))? != 0 {
            return Err(VaultError::InvalidFile);
        }
        Ok(Self {
            header,
            header_bytes,
            dek_nonce,
            wrapped_dek,
            data_nonce,
            ciphertext,
        })
    }
}

fn io_error(error: &io::Error) -> VaultError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        return VaultError::InvalidFile;
    }
    tracing::warn!(%error, "lecture du coffre impossible");
    VaultError::Io
}

/// Écriture séquentielle dans un tableau de taille connue.
struct Writer<'a> {
    out: &'a mut [u8],
    pos: usize,
}

impl<'a> Writer<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Self { out, pos: 0 }
    }

    fn put(&mut self, bytes: &[u8]) {
        // Les tailles sont des constantes du format : `encode` écrit exactement HEADER_LEN.
        if let Some(dst) = self.out.get_mut(self.pos..self.pos + bytes.len()) {
            dst.copy_from_slice(bytes);
        }
        self.pos += bytes.len();
    }
}

/// Lecture séquentielle de champs de taille fixe.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn take<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        // Les appels lisent exactement la taille du tableau source (constantes du format).
        if let Some(src) = self.bytes.get(self.pos..self.pos + N) {
            out.copy_from_slice(src);
        }
        self.pos += N;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Header {
        Header {
            params: KdfParams::DEFAULT,
            salt: [7; SALT_LEN],
        }
    }

    fn sample(payload_len: usize) -> EncryptedVault {
        let header = header();
        EncryptedVault {
            header,
            header_bytes: header.encode(),
            dek_nonce: [1; NONCE_LEN],
            wrapped_dek: [2; WRAPPED_DEK_LEN],
            data_nonce: [3; NONCE_LEN],
            ciphertext: vec![4; payload_len],
        }
    }

    fn parse(bytes: &[u8]) -> Result<EncryptedVault, VaultError> {
        EncryptedVault::read_from(&mut &bytes[..], bytes.len() as u64)
    }

    #[test]
    fn layout_matches_the_specification() {
        assert_eq!(HEADER_LEN, 35);
        assert_eq!(FIXED_LEN, 35 + 24 + 48 + 24 + 8);
        let bytes = sample(20).to_bytes();
        assert_eq!(&bytes[..4], b"YNJV");
        assert_eq!(&bytes[4..6], &1u16.to_le_bytes());
        assert_eq!(bytes[6], 1);
        assert_eq!(&bytes[7..11], &(64 * 1024u32).to_le_bytes());
        assert_eq!(&bytes[11..15], &3u32.to_le_bytes());
        assert_eq!(&bytes[15..19], &4u32.to_le_bytes());
        assert_eq!(&bytes[FIXED_LEN - 8..FIXED_LEN], &20u64.to_le_bytes());
        assert_eq!(bytes.len(), FIXED_LEN + 20);
    }

    #[test]
    fn round_trips() {
        let parsed = parse(&sample(32).to_bytes()).unwrap();
        assert_eq!(parsed.header, header());
        assert_eq!(parsed.ciphertext, vec![4; 32]);
        assert_eq!(parsed.wrapped_dek, [2; WRAPPED_DEK_LEN]);
    }

    #[test]
    fn rejects_bad_magic_version_and_kdf() {
        let mut bytes = sample(32).to_bytes();
        bytes[0] = b'X';
        assert_eq!(parse(&bytes).err(), Some(VaultError::InvalidFile));

        let mut bytes = sample(32).to_bytes();
        bytes[4] = 2;
        assert_eq!(parse(&bytes).err(), Some(VaultError::UnsupportedVersion));

        let mut bytes = sample(32).to_bytes();
        bytes[6] = 2;
        assert_eq!(parse(&bytes).err(), Some(VaultError::UnsupportedVersion));
    }

    #[test]
    fn rejects_params_below_floor_or_above_ceiling() {
        for params in [
            KdfParams {
                m_kib: 1024,
                ..KdfParams::MIN
            },
            KdfParams {
                t: 1,
                ..KdfParams::MIN
            },
            KdfParams {
                p: 1,
                ..KdfParams::MIN
            },
            KdfParams {
                m_kib: 4 * 1024 * 1024,
                ..KdfParams::MIN
            },
            KdfParams {
                t: 1000,
                ..KdfParams::MIN
            },
            KdfParams {
                p: 64,
                ..KdfParams::MIN
            },
        ] {
            let mut vault = sample(32);
            vault.header_bytes = Header {
                params,
                salt: [0; SALT_LEN],
            }
            .encode();
            assert_eq!(
                parse(&vault.to_bytes()).err(),
                Some(VaultError::KdfParamsRejected),
                "{params:?}"
            );
        }
        assert!(KdfParams::MAX.validate().is_ok());
    }

    #[test]
    fn rejects_inconsistent_lengths_before_allocating() {
        // Longueur annoncée énorme, fichier minuscule : refus sans allouer 2^60 octets.
        let mut bytes = sample(32).to_bytes();
        bytes[FIXED_LEN - 8..FIXED_LEN].copy_from_slice(&(1u64 << 60).to_le_bytes());
        assert_eq!(parse(&bytes).err(), Some(VaultError::InvalidFile));

        // Fichier tronqué ou rallongé.
        let bytes = sample(32).to_bytes();
        assert_eq!(
            parse(&bytes[..bytes.len() - 1]).err(),
            Some(VaultError::InvalidFile)
        );
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(parse(&longer).err(), Some(VaultError::InvalidFile));

        // Fichier trop court pour la partie fixe, ou sans place pour un tag.
        assert_eq!(parse(&bytes[..10]).err(), Some(VaultError::InvalidFile));
        assert_eq!(
            parse(&sample(8).to_bytes()).err(),
            Some(VaultError::InvalidFile)
        );

        // Taille totale au-delà du plafond, refusée avant toute lecture.
        let mut empty: &[u8] = &[];
        assert_eq!(
            EncryptedVault::read_from(&mut empty, FIXED_LEN as u64 + MAX_PAYLOAD_LEN + 1).err(),
            Some(VaultError::InvalidFile)
        );
    }

    #[test]
    fn a_stream_that_grows_is_rejected() {
        let bytes = sample(32).to_bytes();
        let mut longer = bytes.clone();
        longer.extend_from_slice(b"surplus");
        // La taille mesurée est l'ancienne, le flux contient davantage.
        assert_eq!(
            EncryptedVault::read_from(&mut &longer[..], bytes.len() as u64).err(),
            Some(VaultError::InvalidFile)
        );
    }
}
