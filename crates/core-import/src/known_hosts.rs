//! Fusion d'un `known_hosts` d'OpenSSH avec les clés d'hôte du coffre (IMP-02).
//!
//! Règle de sécurité (TOFU strict, ADR-023) : un import ne doit jamais **ajouter une autre
//! clé** à un hôte que le coffre connaît déjà. Une ligne qui nomme un hôte déjà présent (en
//! clair) est refusée et signalée comme conflit ; seule une ligne identique est ignorée sans
//! bruit. Les lignes `@revoked` sont conservées (elles ne font que restreindre), les lignes
//! `@cert-authority`, les motifs à joker et les noms hachés (que Younji ne consulte pas à la
//! connexion) sont refusés.

use std::collections::HashSet;

use crate::text::display;

/// Lignes lues au plus.
pub const MAX_IMPORT_LINES: usize = 100_000;
/// Longueur maximale du champ des noms d'hôte d'une ligne.
const MAX_HOSTS_FIELD: usize = 4096;
/// Lignes détaillées au plus dans le rapport.
const MAX_REPORTED: usize = 500;

/// Raison du refus d'une ligne (`import.knownHosts.skip.<code>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownHostsSkip {
    /// Ligne illisible ou clé invalide.
    Invalid,
    /// `@cert-authority` : non pris en charge.
    CertAuthority,
    /// Motif à joker ou négation : non pris en charge.
    Pattern,
    /// Noms hachés (`|1|…`) : Younji ne les consulte pas à la connexion (ADR-023), la ligne
    /// n'aurait aucun effet (et un `@revoked` haché ne révoquerait rien).
    Hashed,
    /// L'hôte est déjà connu avec d'autres clés : refusé.
    Conflict,
    /// Taille maximale des clés d'hôte du coffre atteinte.
    TooLarge,
}

impl KnownHostsSkip {
    /// Code stable.
    pub fn code(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::CertAuthority => "cert_authority",
            Self::Pattern => "pattern",
            Self::Hashed => "hashed",
            Self::Conflict => "conflict",
            Self::TooLarge => "too_large",
        }
    }
}

/// Résultat de la fusion (rien n'est encore enregistré).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownHostsMerge {
    /// Nouveau contenu complet des clés d'hôte du coffre.
    pub content: String,
    /// Lignes ajoutées : noms d'hôte affichables (`(haché)` pour une ligne hachée) et algorithme.
    pub added: Vec<(String, String)>,
    /// Nombre de lignes ajoutées.
    pub added_count: usize,
    /// Lignes déjà présentes à l'identique.
    pub duplicates: usize,
    /// Lignes refusées : numéro de ligne (à partir de 1), noms d'hôte affichables, raison.
    pub skipped: Vec<(usize, String, KnownHostsSkip)>,
    /// Nombre de lignes refusées.
    pub skipped_count: usize,
}

struct Line<'a> {
    marker: Option<&'a str>,
    hosts: &'a str,
    algorithm: &'a str,
    key: &'a str,
}

fn parse(line: &str) -> Option<Line<'_>> {
    let mut fields = line.split_whitespace();
    let first = fields.next()?;
    let (marker, hosts) = if first.starts_with('@') {
        (Some(first), fields.next()?)
    } else {
        (None, first)
    };
    Some(Line {
        marker,
        hosts,
        algorithm: fields.next()?,
        key: fields.next()?,
    })
}

impl Line<'_> {
    fn normalized(&self) -> String {
        match self.marker {
            Some(marker) => format!("{marker} {} {} {}", self.hosts, self.algorithm, self.key),
            None => format!("{} {} {}", self.hosts, self.algorithm, self.key),
        }
    }

    /// Noms en clair (minuscules) ; les entrées hachées (`|1|…`) sont exclues.
    fn plain_names(&self) -> impl Iterator<Item = String> + '_ {
        self.hosts
            .split(',')
            .filter(|h| !h.starts_with('|'))
            .map(str::to_ascii_lowercase)
    }

    fn display_hosts(&self) -> String {
        if self.hosts.starts_with('|') {
            "(haché)".to_owned()
        } else {
            display(self.hosts)
        }
    }
}

/// Fusionne `imported` dans `current`. `valid_key(algorithme, base64)` vérifie la clé
/// (algorithme admis, clé décodable). `max_len` : taille maximale du résultat.
pub fn merge_known_hosts(
    current: &str,
    imported: &str,
    valid_key: &dyn Fn(&str, &str) -> bool,
    max_len: usize,
) -> KnownHostsMerge {
    let mut known_lines: HashSet<String> = HashSet::new();
    let mut known_names: HashSet<String> = HashSet::new();
    for line in current.lines().filter_map(|l| parse(l.trim())) {
        known_lines.insert(line.normalized());
        if line.marker.is_none() {
            known_names.extend(line.plain_names());
        }
    }

    let mut merge = KnownHostsMerge {
        content: current.to_owned(),
        ..KnownHostsMerge::default()
    };
    if !merge.content.is_empty() && !merge.content.ends_with('\n') {
        merge.content.push('\n');
    }
    let skip = |merge: &mut KnownHostsMerge, number: usize, hosts: String, reason| {
        merge.skipped_count += 1;
        if merge.skipped.len() < MAX_REPORTED {
            merge.skipped.push((number, hosts, reason));
        }
    };

    for (index, raw) in imported.lines().take(MAX_IMPORT_LINES).enumerate() {
        let number = index + 1;
        let raw = raw.trim();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let Some(line) = parse(raw) else {
            skip(&mut merge, number, String::new(), KnownHostsSkip::Invalid);
            continue;
        };
        let shown = line.display_hosts();
        let revoked = match line.marker {
            None => false,
            Some("@revoked") => true,
            Some("@cert-authority") => {
                skip(&mut merge, number, shown, KnownHostsSkip::CertAuthority);
                continue;
            }
            Some(_) => {
                skip(&mut merge, number, shown, KnownHostsSkip::Invalid);
                continue;
            }
        };
        if line.hosts.len() > MAX_HOSTS_FIELD
            || line.hosts.chars().any(crate::text::is_unsafe)
            || !valid_key(line.algorithm, line.key)
        {
            skip(&mut merge, number, shown, KnownHostsSkip::Invalid);
            continue;
        }
        if line.hosts.split(',').any(|h| h.starts_with('|')) {
            skip(&mut merge, number, shown, KnownHostsSkip::Hashed);
            continue;
        }
        if line.hosts.contains(['*', '?', '!']) {
            skip(&mut merge, number, shown, KnownHostsSkip::Pattern);
            continue;
        }
        let normalized = line.normalized();
        if known_lines.contains(&normalized) {
            merge.duplicates += 1;
            continue;
        }
        if !revoked && line.plain_names().any(|name| known_names.contains(&name)) {
            skip(&mut merge, number, shown, KnownHostsSkip::Conflict);
            continue;
        }
        if merge.content.len() + normalized.len() + 1 > max_len {
            skip(&mut merge, number, shown, KnownHostsSkip::TooLarge);
            continue;
        }
        merge.content.push_str(&normalized);
        merge.content.push('\n');
        known_lines.insert(normalized);
        merge.added_count += 1;
        if merge.added.len() < MAX_REPORTED {
            merge.added.push((shown, display(line.algorithm)));
        }
    }
    // Les noms ajoutés par cet import ne comptent pas comme « déjà connus » pour les lignes
    // suivantes : un hôte a souvent plusieurs clés (ed25519, ecdsa, rsa) dans le même fichier.
    merge
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(algorithm: &str, key: &str) -> bool {
        algorithm.starts_with("ssh-") && algorithm != "ssh-dss" && key.starts_with("AAAA")
    }

    const MAX: usize = 4 * 1024 * 1024;

    #[test]
    fn adds_new_hosts_and_ignores_identical_lines() {
        let current = "srv.lab ssh-ed25519 AAAAcle1\n";
        let imported = "# commentaire\n\nsrv.lab ssh-ed25519 AAAAcle1 commentaire\n\
                        neuf.lab,10.0.0.9 ssh-ed25519 AAAAcle2\n\
                        neuf.lab ssh-rsa AAAAcle3\n";
        let merge = merge_known_hosts(current, imported, &valid, MAX);
        assert_eq!(merge.duplicates, 1);
        assert_eq!(merge.added_count, 2);
        assert_eq!(
            merge.content,
            "srv.lab ssh-ed25519 AAAAcle1\nneuf.lab,10.0.0.9 ssh-ed25519 AAAAcle2\nneuf.lab ssh-rsa AAAAcle3\n"
        );
        assert!(merge.skipped.is_empty());
    }

    #[test]
    fn never_adds_another_key_for_a_known_host() {
        let current = "[srv.lab]:2222 ssh-ed25519 AAAAcle1\n";
        let imported =
            "[SRV.lab]:2222 ssh-ed25519 AAAAattaquant\nautre,[srv.lab]:2222 ssh-rsa AAAAx\n";
        let merge = merge_known_hosts(current, imported, &valid, MAX);
        assert_eq!(merge.added_count, 0);
        assert_eq!(merge.content, current);
        assert_eq!(
            merge.skipped.iter().map(|s| s.2).collect::<Vec<_>>(),
            [KnownHostsSkip::Conflict, KnownHostsSkip::Conflict]
        );
    }

    #[test]
    fn markers_patterns_and_invalid_lines() {
        let current = "srv ssh-ed25519 AAAAcle1\n";
        let imported = "@revoked srv ssh-ed25519 AAAAvieille\n\
                        @cert-authority *.lab ssh-ed25519 AAAAca\n\
                        *.lab ssh-ed25519 AAAAx\n\
                        bad ssh-dss AAAAx\n\
                        @other h ssh-ed25519 AAAAx\n\
                        tronquee ssh-ed25519\n\
                        |1|c2Vs|aGFjaA== ssh-ed25519 AAAAhache\n\
                        a\u{1b}b ssh-ed25519 AAAAx\n";
        let merge = merge_known_hosts(current, imported, &valid, MAX);
        assert_eq!(merge.added_count, 1);
        assert!(
            merge
                .content
                .contains("@revoked srv ssh-ed25519 AAAAvieille\n")
        );
        assert!(!merge.content.contains("AAAAhache"));
        let reasons: Vec<_> = merge.skipped.iter().map(|s| (s.0, s.2)).collect();
        assert_eq!(
            reasons,
            [
                (2, KnownHostsSkip::CertAuthority),
                (3, KnownHostsSkip::Pattern),
                (4, KnownHostsSkip::Invalid),
                (5, KnownHostsSkip::Invalid),
                (6, KnownHostsSkip::Invalid),
                (7, KnownHostsSkip::Hashed),
                (8, KnownHostsSkip::Invalid),
            ]
        );
    }

    #[test]
    fn size_is_bounded() {
        let merge = merge_known_hosts("", "a ssh-ed25519 AAAA1\nb ssh-ed25519 AAAA2\n", &valid, 25);
        assert_eq!(merge.added_count, 1);
        assert_eq!(merge.skipped[0].2, KnownHostsSkip::TooLarge);
    }
}
