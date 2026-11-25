//! Gestionnaire d'identifiants de Windows (SECURITE.md §4.3, ADR-039).
//!
//! Seul module de Younji autorisé à utiliser `unsafe` : trois appels Win32 (`CredWriteW`,
//! `CredReadW` + `CredFree`, `CredDeleteW`). L'identifiant est de type
//! `CRED_TYPE_DOMAIN_PASSWORD` : son mot de passe n'est utilisable que par LSA (mstsc via
//! CredSSP) et **n'est pas relisible** par `CredRead` depuis un autre programme. Persistance
//! `CRED_PERSIST_SESSION` : jamais écrit sur disque ni itinérant, perdu à la déconnexion de la
//! session Windows au plus tard.
#![allow(unsafe_code)]

use secrecy::{ExposeSecret, SecretString};
use windows_sys::Win32::Security::Credentials::{
    CRED_PERSIST_SESSION, CRED_TYPE_DOMAIN_PASSWORD, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};
use zeroize::Zeroizing;

use crate::RdpError;

/// Chaîne UTF-16 terminée par un zéro.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Cible utilisée par mstsc pour un serveur : `TERMSRV/<hôte>`.
pub fn target_name(address: &str) -> String {
    format!("TERMSRV/{address}")
}

/// Vrai si un identifiant de ce type existe déjà pour cette cible (enregistré par l'utilisateur
/// dans mstsc, par exemple). Le mot de passe n'est pas lu.
pub fn exists(target: &str) -> bool {
    let name = wide(target);
    let mut found: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `name` est une chaîne UTF-16 terminée par zéro qui vit jusqu'à la fin de
    // l'appel ; `found` est un pointeur de sortie valide. En cas de succès, le tampon alloué
    // par Windows est libéré par `CredFree`, une seule fois, sans être déréférencé.
    unsafe {
        if CredReadW(name.as_ptr(), CRED_TYPE_DOMAIN_PASSWORD, 0, &mut found) != 0 {
            CredFree(found.cast_const().cast());
            true
        } else {
            false
        }
    }
}

/// Écrit l'identifiant `TERMSRV/<hôte>`.
pub fn write(target: &str, username: &str, password: &SecretString) -> Result<(), RdpError> {
    let mut name = wide(target);
    let mut user = wide(username);
    // Mot de passe en UTF-16LE, sans zéro final (format attendu par CredSSP), effacé ensuite.
    let blob: Zeroizing<Vec<u8>> = Zeroizing::new(
        password
            .expose_secret()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    );
    let size = u32::try_from(blob.len()).map_err(|_| RdpError::InvalidPassword)?;
    let credential = CREDENTIALW {
        Type: CRED_TYPE_DOMAIN_PASSWORD,
        TargetName: name.as_mut_ptr(),
        CredentialBlobSize: size,
        CredentialBlob: blob.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_SESSION,
        UserName: user.as_mut_ptr(),
        ..CREDENTIALW::default()
    };
    // SAFETY: tous les pointeurs de `credential` désignent des tampons (`name`, `user`,
    // `blob`) qui restent vivants et inchangés pendant l'appel ; Windows les copie et ne les
    // conserve pas. Les autres champs sont nuls (aucun attribut).
    let ok = unsafe { CredWriteW(&credential, 0) };
    if ok == 0 {
        tracing::warn!("écriture de l'identifiant RDP refusée par Windows");
        return Err(RdpError::CredentialFailed);
    }
    Ok(())
}

/// Supprime l'identifiant `TERMSRV/<hôte>` (sans effet s'il n'existe plus).
pub fn delete(target: &str) {
    let name = wide(target);
    // SAFETY: `name` est une chaîne UTF-16 terminée par zéro, vivante pendant l'appel.
    let ok = unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_DOMAIN_PASSWORD, 0) };
    if ok == 0 {
        tracing::debug!("identifiant RDP déjà absent");
    }
}
