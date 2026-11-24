//! Cible d'une connexion (hôte, port, utilisateur) et taille du terminal, validées.
//!
//! Ces valeurs viennent de l'interface, considérée comme non fiable (SECURITE.md §5) : elles
//! sont validées ici avant toute résolution DNS ou écriture dans `known_hosts`. Un hôte valide
//! ne contient ni espace, ni virgule, ni crochet en dehors d'une adresse IPv6 : il peut donc
//! être écrit tel quel dans une ligne `known_hosts` sans risque d'injection.

use std::fmt;
use std::net::{IpAddr, Ipv6Addr};

use crate::SshError;

/// Longueur maximale d'un nom d'hôte (RFC 1035).
const MAX_HOST_LEN: usize = 253;
/// Longueur maximale d'une étiquette DNS.
const MAX_LABEL_LEN: usize = 63;
/// Longueur maximale d'un nom d'utilisateur (large, pour les comptes de domaine Windows).
const MAX_USERNAME_LEN: usize = 256;

/// Colonnes et lignes maximales acceptées pour le PTY.
pub const MAX_COLS: u32 = 1000;
/// Lignes maximales acceptées pour le PTY.
pub const MAX_ROWS: u32 = 500;

/// Cible validée d'une connexion SSH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    host: String,
    port: u16,
    username: String,
}

impl Target {
    /// Valide et construit une cible. L'hôte est normalisé en minuscules (comme OpenSSH) et une
    /// adresse IPv6 entre crochets est acceptée puis stockée sans crochets.
    pub fn new(host: &str, port: u32, username: &str) -> Result<Self, SshError> {
        let host = normalize_host(host)?;
        let port = u16::try_from(port)
            .ok()
            .filter(|p| *p != 0)
            .ok_or(SshError::InvalidPort)?;
        validate_username(username)?;
        Ok(Self {
            host,
            port,
            username: username.to_owned(),
        })
    }

    /// Hôte normalisé (minuscules, IPv6 sans crochets).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Port TCP.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Nom d'utilisateur.
    pub fn username(&self) -> &str {
        &self.username
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.contains(':') {
            write!(f, "{}@[{}]:{}", self.username, self.host, self.port)
        } else {
            write!(f, "{}@{}:{}", self.username, self.host, self.port)
        }
    }
}

pub(crate) fn normalize_host(raw: &str) -> Result<String, SshError> {
    if raw.chars().any(char::is_control) {
        return Err(SshError::InvalidHost);
    }
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > MAX_HOST_LEN {
        return Err(SshError::InvalidHost);
    }
    // IPv6 entre crochets : [2001:db8::1]
    if let Some(inner) = raw.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return inner
            .parse::<Ipv6Addr>()
            .map(|ip| ip.to_string())
            .map_err(|_| SshError::InvalidHost);
    }
    if let Ok(ip) = raw.parse::<IpAddr>() {
        return Ok(ip.to_string());
    }
    let host = raw.to_ascii_lowercase();
    let host_without_root = host.strip_suffix('.').unwrap_or(&host);
    let valid = !host_without_root.is_empty()
        && host_without_root.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= MAX_LABEL_LEN
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        });
    if valid {
        Ok(host_without_root.to_owned())
    } else {
        Err(SshError::InvalidHost)
    }
}

fn validate_username(username: &str) -> Result<(), SshError> {
    let valid = !username.is_empty()
        && username.len() <= MAX_USERNAME_LEN
        && username.trim() == username
        && !username.chars().any(char::is_control);
    if valid {
        Ok(())
    } else {
        Err(SshError::InvalidUsername)
    }
}

/// Taille du terminal distant, en caractères.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    cols: u32,
    rows: u32,
}

impl TermSize {
    /// Valide une taille : 1–[`MAX_COLS`] colonnes, 1–[`MAX_ROWS`] lignes.
    pub fn new(cols: u32, rows: u32) -> Result<Self, SshError> {
        if (1..=MAX_COLS).contains(&cols) && (1..=MAX_ROWS).contains(&rows) {
            Ok(Self { cols, rows })
        } else {
            Err(SshError::InvalidTerminalSize)
        }
    }

    /// Nombre de colonnes.
    pub fn cols(self) -> u32 {
        self.cols
    }

    /// Nombre de lignes.
    pub fn rows(self) -> u32 {
        self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalizes_valid_hosts() {
        let cases = [
            ("Example.COM", "example.com"),
            ("srv-01.lab.local.", "srv-01.lab.local"),
            ("my_host", "my_host"),
            ("192.168.1.10", "192.168.1.10"),
            ("  10.0.0.1 ", "10.0.0.1"),
            ("2001:DB8::1", "2001:db8::1"),
            ("[2001:db8::1]", "2001:db8::1"),
            ("::1", "::1"),
        ];
        for (input, expected) in cases {
            let target = Target::new(input, 22, "root").unwrap();
            assert_eq!(target.host(), expected, "{input}");
        }
    }

    #[test]
    fn rejects_invalid_hosts() {
        let long = "a".repeat(254);
        let long_label = format!("{}.com", "a".repeat(64));
        let cases = [
            "",
            "   ",
            "host name",
            "host,other",
            "-host",
            "host-.com",
            "a..b",
            "[host]",
            "[::1",
            "host\n",
            "hôte.fr",
            "user@host",
            "host:22",
            "*.example.com",
            "|1|abc",
            long.as_str(),
            long_label.as_str(),
        ];
        for input in cases {
            assert_eq!(
                Target::new(input, 22, "root"),
                Err(SshError::InvalidHost),
                "{input:?}"
            );
        }
    }

    #[test]
    fn validates_ports() {
        assert_eq!(Target::new("h", 0, "u"), Err(SshError::InvalidPort));
        assert_eq!(Target::new("h", 65536, "u"), Err(SshError::InvalidPort));
        assert_eq!(Target::new("h", 65535, "u").unwrap().port(), 65535);
        assert_eq!(Target::new("h", 1, "u").unwrap().port(), 1);
    }

    #[test]
    fn validates_usernames() {
        assert!(Target::new("h", 22, "DOMAIN\\Jean Dupont").is_ok());
        for bad in ["", " root", "root ", "ro\u{0}ot", "a\nb", &"u".repeat(257)] {
            assert_eq!(
                Target::new("h", 22, bad),
                Err(SshError::InvalidUsername),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn displays_ipv6_with_brackets() {
        let target = Target::new("::1", 2222, "u").unwrap();
        assert_eq!(target.to_string(), "u@[::1]:2222");
        let target = Target::new("host", 22, "u").unwrap();
        assert_eq!(target.to_string(), "u@host:22");
    }

    #[test]
    fn validates_terminal_size() {
        assert!(TermSize::new(80, 24).is_ok());
        assert!(TermSize::new(MAX_COLS, MAX_ROWS).is_ok());
        for (c, r) in [(0, 24), (80, 0), (MAX_COLS + 1, 24), (80, MAX_ROWS + 1)] {
            assert_eq!(TermSize::new(c, r), Err(SshError::InvalidTerminalSize));
        }
    }
}
