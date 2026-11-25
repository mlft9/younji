//! Cible RDP validée. Chaque champ finit dans un fichier `.rdp` (une valeur par ligne) ou dans
//! un argument de FreeRDP : on refuse tout ce qui pourrait ajouter une ligne, une option ou un
//! caractère invisible.

use crate::RdpError;

/// Longueur maximale d'une adresse.
pub const MAX_ADDRESS_LEN: usize = 255;
/// Longueur maximale d'un utilisateur.
pub const MAX_USERNAME_LEN: usize = 256;
/// Longueur maximale d'un domaine.
pub const MAX_DOMAIN_LEN: usize = 255;
/// Largeurs admises.
pub const WIDTH_RANGE: std::ops::RangeInclusive<u16> = 640..=7680;
/// Hauteurs admises.
pub const HEIGHT_RANGE: std::ops::RangeInclusive<u16> = 480..=4320;

/// Résolution de la session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Plein écran.
    Fullscreen,
    /// Taille fixe.
    Fixed {
        /// Largeur en pixels.
        width: u16,
        /// Hauteur en pixels.
        height: u16,
    },
}

/// Cible RDP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdpTarget {
    address: String,
    port: u16,
    username: String,
    domain: String,
    resolution: Resolution,
}

/// Caractères interdits partout : contrôles (dont retours à la ligne) et caractères de mise en
/// forme invisibles (bidirectionnels, largeur nulle, BOM).
fn is_forbidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2069}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{FEFF}'
        )
}

/// Adresse : nom d'hôte ou IPv4 (lettres, chiffres, `.`, `-`, `_`), ou IPv6 (chiffres
/// hexadécimaux, `:`, `.`), sans tiret ni barre initiale.
fn is_valid_address(address: &str) -> bool {
    if address.is_empty() || address.len() > MAX_ADDRESS_LEN || address.starts_with(['-', '/']) {
        return false;
    }
    if address.contains(':') {
        address
            .chars()
            .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.'))
    } else {
        address
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    }
}

impl RdpTarget {
    /// Valide une cible.
    pub fn new(
        address: &str,
        port: u16,
        username: &str,
        domain: &str,
        resolution: Resolution,
    ) -> Result<Self, RdpError> {
        if !is_valid_address(address) {
            return Err(RdpError::InvalidAddress);
        }
        if port == 0 {
            return Err(RdpError::InvalidPort);
        }
        if username.trim().is_empty()
            || username.len() > MAX_USERNAME_LEN
            || username.chars().any(is_forbidden)
            || username.starts_with(['-', '/'])
        {
            return Err(RdpError::InvalidUsername);
        }
        if domain.len() > MAX_DOMAIN_LEN
            || domain.starts_with(['-', '/'])
            || !domain
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        {
            return Err(RdpError::InvalidDomain);
        }
        if let Resolution::Fixed { width, height } = resolution
            && !(WIDTH_RANGE.contains(&width) && HEIGHT_RANGE.contains(&height))
        {
            return Err(RdpError::InvalidResolution);
        }
        Ok(Self {
            address: address.to_owned(),
            port,
            username: username.to_owned(),
            domain: domain.to_owned(),
            resolution,
        })
    }

    /// Adresse (sans crochets).
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Utilisateur.
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Domaine (vide : aucun).
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Résolution.
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// `hôte:port`, avec crochets pour une adresse IPv6.
    pub fn host_port(&self) -> String {
        if self.address.contains(':') {
            format!("[{}]:{}", self.address, self.port)
        } else {
            format!("{}:{}", self.address, self.port)
        }
    }

    /// Vrai si l'utilisateur porte déjà son domaine (`DOMAINE\utilisateur` ou
    /// `utilisateur@domaine`).
    pub fn username_has_domain(&self) -> bool {
        self.username.contains(['\\', '@'])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(address: &str, username: &str, domain: &str) -> Result<RdpTarget, RdpError> {
        RdpTarget::new(address, 3389, username, domain, Resolution::Fullscreen)
    }

    #[test]
    fn accepts_usual_targets() {
        assert!(target("win11.lab", "Administrateur", "").is_ok());
        assert!(target("192.168.1.20", "LAB\\alice", "").is_ok());
        assert!(target("fe80::1", "alice@lab.local", "LAB").is_ok());
        assert!(target("srv", "John Smith", "corp.example.com").is_ok());
        let t = target("fe80::1", "a", "").unwrap();
        assert_eq!(t.host_port(), "[fe80::1]:3389");
    }

    #[test]
    fn refuses_injections() {
        assert_eq!(target("", "a", "").unwrap_err(), RdpError::InvalidAddress);
        assert_eq!(target("-x", "a", "").unwrap_err(), RdpError::InvalidAddress);
        assert_eq!(
            target("/p:x", "a", "").unwrap_err(),
            RdpError::InvalidAddress
        );
        assert_eq!(
            target("a b", "a", "").unwrap_err(),
            RdpError::InvalidAddress
        );
        assert_eq!(
            target("srv\nx", "a", "").unwrap_err(),
            RdpError::InvalidAddress
        );
        for user in [
            "",
            " ",
            "a\nalternate shell:s:cmd",
            "a\r",
            "/p:secret",
            "-x",
            "a\u{202E}b",
        ] {
            assert_eq!(
                target("srv", user, "").unwrap_err(),
                RdpError::InvalidUsername,
                "{user:?}"
            );
        }
        for domain in ["a b", "a\nb", "/p:x", "-x", "a\\b"] {
            assert_eq!(
                target("srv", "a", domain).unwrap_err(),
                RdpError::InvalidDomain
            );
        }
        assert_eq!(
            RdpTarget::new("srv", 0, "a", "", Resolution::Fullscreen).unwrap_err(),
            RdpError::InvalidPort
        );
        for (width, height) in [(320, 1080), (1920, 100), (9000, 1080)] {
            assert_eq!(
                RdpTarget::new("srv", 3389, "a", "", Resolution::Fixed { width, height })
                    .unwrap_err(),
                RdpError::InvalidResolution
            );
        }
    }
}
