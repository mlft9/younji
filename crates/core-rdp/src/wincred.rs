//! Gestionnaire d'identifiants de Windows (SECURITE.md §4.3, ADR-040).
//!
//! Seul module de Younji autorisé à utiliser `unsafe` : appels Win32 `CredWriteW`,
//! `CredReadW`, `CredEnumerateW` (+ `CredFree`) et `CredDeleteW`. L'identifiant est de type
//! `CRED_TYPE_DOMAIN_PASSWORD` : son mot de passe n'est utilisable que par LSA (mstsc via
//! CredSSP) et **n'est pas relisible** par `CredRead` depuis un autre programme. Persistance
//! `CRED_PERSIST_SESSION` : jamais écrit sur disque ni itinérant, perdu à la déconnexion de la
//! session Windows au plus tard.
#![allow(unsafe_code)]

use secrecy::{ExposeSecret, SecretString};
use windows_sys::Win32::Security::Credentials::{
    CRED_PERSIST_SESSION, CRED_TYPE_DOMAIN_PASSWORD, CREDENTIALW, CredDeleteW, CredEnumerateW,
    CredFree, CredReadW, CredWriteW,
};
use zeroize::Zeroizing;

use crate::RdpError;

/// Commentaire posé sur les identifiants écrits par Younji (non secret) : il distingue un reste
/// de Younji (à supprimer) d'un identifiant enregistré par l'utilisateur (jamais touché).
pub const MARKER: &str = "Younji (temporaire)";

/// Propriétaire d'un identifiant `TERMSRV/<hôte>` existant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Existing {
    /// Aucun identifiant.
    None,
    /// Identifiant de l'utilisateur.
    User,
    /// Identifiant temporaire écrit par Younji.
    Younji,
}

/// Lit une chaîne UTF-16 terminée par zéro (pointeur nul : chaîne vide).
///
/// # Safety
/// `ptr` est nul ou désigne une chaîne UTF-16 terminée par zéro, valide pendant l'appel.
unsafe fn read_wide(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    // SAFETY: la chaîne est terminée par zéro (contrat de la fonction) ; on s'arrête au zéro.
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` éléments lisibles avant le zéro final.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Chaîne UTF-16 terminée par un zéro.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Cible utilisée par mstsc pour un serveur : `TERMSRV/<hôte>`.
pub fn target_name(address: &str) -> String {
    format!("TERMSRV/{address}")
}

/// Identifiant déjà présent pour cette cible, et à qui il appartient (d'après le marqueur).
/// Le mot de passe n'est pas lu (Windows ne le renvoie pas pour ce type).
pub fn existing(target: &str) -> Existing {
    let name = wide(target);
    let mut found: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `name` est une chaîne UTF-16 terminée par zéro qui vit jusqu'à la fin de
    // l'appel ; `found` est un pointeur de sortie valide. En cas de succès, `found` désigne
    // une structure allouée par Windows, lue avant d'être libérée par `CredFree`, une seule
    // fois ; `Comment` est nul ou une chaîne terminée par zéro dans ce même tampon.
    unsafe {
        if CredReadW(name.as_ptr(), CRED_TYPE_DOMAIN_PASSWORD, 0, &mut found) == 0 {
            return Existing::None;
        }
        let comment = read_wide((*found).Comment);
        CredFree(found.cast_const().cast());
        if comment == MARKER {
            Existing::Younji
        } else {
            Existing::User
        }
    }
}

/// Supprime les identifiants `TERMSRV/*` laissés par Younji (arrêt brutal pendant le délai de
/// nettoyage). Ceux de l'utilisateur, sans le marqueur, ne sont jamais touchés. Renvoie leur
/// nombre.
pub fn cleanup_stale() -> usize {
    let filter = wide("TERMSRV/*");
    let mut count: u32 = 0;
    let mut list: *mut *mut CREDENTIALW = std::ptr::null_mut();
    let mut stale = Vec::new();
    // SAFETY: `filter` est une chaîne UTF-16 terminée par zéro vivante pendant l'appel ;
    // `count` et `list` sont des pointeurs de sortie valides. En cas de succès, `list` désigne
    // `count` pointeurs vers des structures valides jusqu'au `CredFree` unique qui suit ;
    // `TargetName` et `Comment` sont nuls ou terminés par zéro dans ce tampon.
    unsafe {
        if CredEnumerateW(filter.as_ptr(), 0, &mut count, &mut list) == 0 {
            return 0;
        }
        let entries = std::slice::from_raw_parts(list, count as usize);
        for &entry in entries {
            if entry.is_null() || (*entry).Type != CRED_TYPE_DOMAIN_PASSWORD {
                continue;
            }
            if read_wide((*entry).Comment) == MARKER {
                stale.push(read_wide((*entry).TargetName));
            }
        }
        CredFree(list.cast_const().cast());
    }
    for target in &stale {
        delete(target);
    }
    stale.len()
}

/// Écrit l'identifiant `TERMSRV/<hôte>`.
pub fn write(target: &str, username: &str, password: &SecretString) -> Result<(), RdpError> {
    let mut name = wide(target);
    let mut user = wide(username);
    let mut comment = wide(MARKER);
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
        Comment: comment.as_mut_ptr(),
        ..CREDENTIALW::default()
    };
    // SAFETY: tous les pointeurs de `credential` désignent des tampons (`name`, `user`,
    // `comment`, `blob`) qui restent vivants et inchangés pendant l'appel ; Windows les copie et ne les
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
