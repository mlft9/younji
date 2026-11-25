//! Lecture de `~/.ssh/config` (IMP-01).
//!
//! Règles reprises d'OpenSSH (`ssh_config(5)`) :
//! - un mot-clé par ligne, insensible à la casse, séparé de ses arguments par des espaces ou
//!   un `=` ; arguments entre guillemets admis ; lignes vides et commentaires (`#`) ignorés ;
//! - `Include` insère les fichiers désignés **à l'endroit de la directive** (chemins relatifs à
//!   `~/.ssh`, `~` développé, motifs `*` et `?`, fichiers triés) ;
//! - pour un alias, **la première valeur obtenue gagne**, en parcourant tous les blocs `Host`
//!   qui lui correspondent (motifs, négations `!`), y compris les blocs à jokers comme
//!   `Host *` en fin de fichier ;
//! - `Match` n'est pas pris en charge : son bloc est ignoré et signalé.
//!
//! Bornes : profondeur d'`Include` [`MAX_DEPTH`], [`MAX_FILES`] fichiers, [`MAX_FILE_SIZE`] par
//! fichier, [`MAX_LINES`] lignes au total, [`MAX_HOSTS`] alias. Les clés privées
//! (`IdentityFile`) ne sont jamais lues.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::text::{display, is_unsafe, wildcard_match};

/// Profondeur maximale d'`Include`.
pub const MAX_DEPTH: usize = 16;
/// Nombre maximal de fichiers lus.
pub const MAX_FILES: usize = 256;
/// Taille maximale d'un fichier.
pub const MAX_FILE_SIZE: u64 = 1024 * 1024;
/// Nombre maximal de lignes lues au total.
pub const MAX_LINES: usize = 50_000;
/// Nombre maximal d'alias retenus.
pub const MAX_HOSTS: usize = 10_000;
/// Entrées d'un dossier examinées au plus pour un motif d'`Include`.
const MAX_DIR_ENTRIES: usize = 4096;
/// Longueur maximale d'une valeur retenue (nom d'hôte, utilisateur).
const MAX_VALUE_LEN: usize = 255;

/// Raison pour laquelle une entrée n'est pas importée. Le code stable sert de clé i18n
/// (`import.skip.<code>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Motif à joker : ce n'est pas un hôte, ses valeurs s'appliquent aux hôtes correspondants.
    Wildcard,
    /// Bloc `Match`, non pris en charge.
    MatchBlock,
    /// Fichier d'`Include` introuvable.
    IncludeNotFound,
    /// Fichier illisible (droits, encodage).
    Unreadable,
    /// Fichier trop volumineux.
    FileTooLarge,
    /// `Include` trop profond, en boucle, ou trop de fichiers / lignes.
    LimitReached,
    /// Jeton `%` non pris en charge dans `HostName` ou `User`.
    UnsupportedToken,
    /// Port invalide.
    InvalidPort,
    /// Nom d'hôte ou utilisateur invalide.
    InvalidValue,
    /// Même adresse, port et utilisateur qu'un alias précédent.
    Duplicate,
    /// Déjà présent dans le carnet (signalé par l'appelant).
    AlreadyInVault,
}

impl SkipReason {
    /// Code stable.
    pub fn code(self) -> &'static str {
        match self {
            Self::Wildcard => "wildcard",
            Self::MatchBlock => "match_block",
            Self::IncludeNotFound => "include_not_found",
            Self::Unreadable => "unreadable",
            Self::FileTooLarge => "file_too_large",
            Self::LimitReached => "limit_reached",
            Self::UnsupportedToken => "unsupported_token",
            Self::InvalidPort => "invalid_port",
            Self::InvalidValue => "invalid_value",
            Self::Duplicate => "duplicate",
            Self::AlreadyInVault => "already_in_vault",
        }
    }
}

/// Entrée non importée, avec sa raison. `entry` est un texte nettoyé et borné.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// Directive concernée (`Host *.lab`, `Include config.d/*`…).
    pub entry: String,
    /// Raison.
    pub reason: SkipReason,
}

/// Avertissement sur un hôte importable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostWarning {
    /// `ProxyJump` / `ProxyCommand` ignoré : Younji se connectera directement.
    ProxyIgnored,
    /// Redirections de ports ignorées (hors périmètre, ADR-038).
    ForwardingIgnored,
    /// `IdentityFile` : clé à importer à part (Clés > Importer).
    IdentityFileNotImported,
    /// Utilisateur absent : celui du poste sera utilisé.
    DefaultUser,
}

impl HostWarning {
    /// Code stable (`import.warning.<code>`).
    pub fn code(self) -> &'static str {
        match self {
            Self::ProxyIgnored => "proxy_ignored",
            Self::ForwardingIgnored => "forwarding_ignored",
            Self::IdentityFileNotImported => "identity_file",
            Self::DefaultUser => "default_user",
        }
    }
}

/// Hôte importable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedHost {
    /// Alias (`Host`), utilisé comme nom.
    pub alias: String,
    /// Adresse (`HostName`, ou l'alias).
    pub hostname: String,
    /// Port (22 par défaut).
    pub port: u16,
    /// Utilisateur (`User`), s'il est indiqué.
    pub user: Option<String>,
    /// Fichiers de clé indiqués (textes nettoyés, jamais lus).
    pub identity_files: Vec<String>,
    /// Avertissements.
    pub warnings: Vec<HostWarning>,
}

/// Résultat de la lecture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigReport {
    /// Hôtes importables, dans l'ordre du fichier.
    pub hosts: Vec<ImportedHost>,
    /// Entrées ignorées, avec leur raison.
    pub skipped: Vec<Skipped>,
}

/// Élément de configuration, `Include` déjà développés.
#[derive(Debug)]
enum Item {
    Host(Vec<String>),
    Match,
    Option { key: String, args: Vec<String> },
}

struct Reader<'a> {
    ssh_dir: &'a Path,
    home: &'a Path,
    items: Vec<Item>,
    skipped: Vec<Skipped>,
    files: usize,
    lines: usize,
}

/// Lit `path` (en général `~/.ssh/config`) et calcule la configuration de chaque alias.
/// `ssh_dir` sert de base aux `Include` relatifs, `home` au développement de `~`.
pub fn parse_ssh_config(path: &Path, ssh_dir: &Path, home: &Path) -> ConfigReport {
    let mut reader = Reader {
        ssh_dir,
        home,
        items: Vec::new(),
        skipped: Vec::new(),
        files: 0,
        lines: 0,
    };
    let mut stack = Vec::new();
    reader.read_file(path, 0, &mut stack, &display(&path.to_string_lossy()));
    let Reader {
        items, mut skipped, ..
    } = reader;
    let hosts = resolve(&items, &mut skipped);
    ConfigReport { hosts, skipped }
}

impl Reader<'_> {
    fn skip(&mut self, entry: String, reason: SkipReason) {
        self.skipped.push(Skipped { entry, reason });
    }

    fn read_file(&mut self, path: &Path, depth: usize, stack: &mut Vec<PathBuf>, label: &str) {
        if depth > MAX_DEPTH || self.files >= MAX_FILES {
            self.skip(label.to_owned(), SkipReason::LimitReached);
            return;
        }
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => {
                self.skip(label.to_owned(), SkipReason::IncludeNotFound);
                return;
            }
        };
        if stack.contains(&canonical) {
            self.skip(label.to_owned(), SkipReason::LimitReached);
            return;
        }
        let text = match read_bounded(&canonical) {
            Ok(text) => text,
            Err(reason) => {
                self.skip(label.to_owned(), reason);
                return;
            }
        };
        self.files += 1;
        stack.push(canonical);
        for line in text.lines() {
            if self.lines >= MAX_LINES {
                self.skip(label.to_owned(), SkipReason::LimitReached);
                break;
            }
            self.lines += 1;
            let Some((key, args)) = tokenize(line) else {
                continue;
            };
            match key.as_str() {
                "host" => self.items.push(Item::Host(args)),
                "match" => {
                    self.skip(
                        format!("Match {}", display(&args.join(" "))),
                        SkipReason::MatchBlock,
                    );
                    self.items.push(Item::Match);
                }
                "include" => {
                    for pattern in &args {
                        self.include(pattern, depth, stack);
                    }
                }
                _ => self.items.push(Item::Option { key, args }),
            }
        }
        stack.pop();
    }

    fn include(&mut self, pattern: &str, depth: usize, stack: &mut Vec<PathBuf>) {
        let label = format!("Include {}", display(pattern));
        let full = if let Some(rest) = pattern.strip_prefix("~/") {
            self.home.join(rest)
        } else if Path::new(pattern).is_absolute() {
            PathBuf::from(pattern)
        } else {
            self.ssh_dir.join(pattern)
        };
        let files = expand_glob(&full);
        if files.is_empty() {
            // OpenSSH ignore en silence un Include sans fichier ; on le signale.
            self.skip(label, SkipReason::IncludeNotFound);
            return;
        }
        for file in files {
            self.read_file(&file, depth + 1, stack, &label);
        }
    }
}

fn read_bounded(path: &Path) -> Result<String, SkipReason> {
    // Type vérifié **avant** l'ouverture : ouvrir un FIFO bloquerait jusqu'à l'arrivée d'un
    // écrivain, et un périphérique (`/dev/zero`) ne finirait jamais.
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return Err(SkipReason::Unreadable);
    }
    let file = std::fs::File::open(path).map_err(|_| SkipReason::Unreadable)?;
    let meta = file.metadata().map_err(|_| SkipReason::Unreadable)?;
    if !meta.is_file() {
        return Err(SkipReason::Unreadable);
    }
    if meta.len() > MAX_FILE_SIZE {
        return Err(SkipReason::FileTooLarge);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SkipReason::Unreadable)?;
    if bytes.len() as u64 > MAX_FILE_SIZE {
        return Err(SkipReason::FileTooLarge);
    }
    String::from_utf8(bytes).map_err(|_| SkipReason::Unreadable)
}

/// Développe un chemin dont les composants peuvent contenir `*` et `?`. Résultat trié, fichiers
/// cachés exclus sauf motif commençant par `.`.
fn expand_glob(path: &Path) -> Vec<PathBuf> {
    let mut current: Vec<PathBuf> = vec![PathBuf::new()];
    for component in path.components() {
        let part = component.as_os_str().to_string_lossy();
        let is_pattern = matches!(component, Component::Normal(_)) && part.contains(['*', '?']);
        if !is_pattern {
            for candidate in &mut current {
                candidate.push(component.as_os_str());
            }
            continue;
        }
        let mut next = Vec::new();
        for dir in &current {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            let mut names: Vec<String> = entries
                .flatten()
                .take(MAX_DIR_ENTRIES)
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .filter(|name| !name.starts_with('.') || part.starts_with('.'))
                .filter(|name| wildcard_match(&part, name))
                .collect();
            names.sort();
            next.extend(names.into_iter().map(|name| dir.join(name)));
        }
        current = next;
        if current.len() > MAX_FILES {
            current.truncate(MAX_FILES);
        }
    }
    current.retain(|p| p.is_file());
    current
}

/// Découpe une ligne en mot-clé (minuscules) et arguments. `None` pour une ligne vide ou un
/// commentaire.
fn tokenize(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let end = line
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(line.len());
    let (keyword, rest) = line.split_at(end);
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest);
    Some((keyword.to_ascii_lowercase(), split_args(rest)))
}

/// Arguments séparés par des espaces, guillemets doubles respectés.
fn split_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has_token = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has_token = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has_token {
                    args.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        args.push(current);
    }
    args
}

fn is_concrete(pattern: &str) -> bool {
    !pattern.is_empty() && !pattern.contains(['*', '?', '!'])
}

/// Vrai si l'alias correspond au bloc `Host` : au moins un motif positif, aucun motif négatif.
fn host_matches(patterns: &[String], alias: &str) -> bool {
    let mut positive = false;
    for pattern in patterns {
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard_match(negated, alias) {
                return false;
            }
        } else if wildcard_match(pattern, alias) {
            positive = true;
        }
    }
    positive
}

/// Remplace `%h` (alias) et `%%` ; tout autre jeton est refusé.
fn expand_tokens(value: &str, alias: &str) -> Result<String, SkipReason> {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('h') => out.push_str(alias),
            _ => return Err(SkipReason::UnsupportedToken),
        }
    }
    Ok(out)
}

fn check_value(value: &str) -> Result<(), SkipReason> {
    if value.is_empty()
        || value.len() > MAX_VALUE_LEN
        || value.chars().any(|c| is_unsafe(c) || c.is_whitespace())
    {
        Err(SkipReason::InvalidValue)
    } else {
        Ok(())
    }
}

/// Configuration effective de chaque alias concret.
fn resolve(items: &[Item], skipped: &mut Vec<Skipped>) -> Vec<ImportedHost> {
    let mut aliases: Vec<&str> = Vec::new();
    let mut seen_patterns: HashSet<String> = HashSet::new();
    for item in items {
        let Item::Host(patterns) = item else {
            continue;
        };
        for pattern in patterns {
            if is_concrete(pattern) {
                if !aliases.contains(&pattern.as_str()) && aliases.len() < MAX_HOSTS {
                    aliases.push(pattern);
                }
            } else if !pattern.starts_with('!')
                && seen_patterns.insert(pattern.to_ascii_lowercase())
            {
                skipped.push(Skipped {
                    entry: format!("Host {}", display(pattern)),
                    reason: SkipReason::Wildcard,
                });
            }
        }
    }

    let mut hosts: Vec<ImportedHost> = Vec::new();
    for alias in aliases {
        match resolve_alias(items, alias) {
            Ok(host) => {
                let duplicate = hosts.iter().any(|h| {
                    h.hostname.eq_ignore_ascii_case(&host.hostname)
                        && h.port == host.port
                        && h.user == host.user
                });
                if duplicate {
                    skipped.push(Skipped {
                        entry: format!("Host {}", display(alias)),
                        reason: SkipReason::Duplicate,
                    });
                } else {
                    hosts.push(host);
                }
            }
            Err(reason) => skipped.push(Skipped {
                entry: format!("Host {}", display(alias)),
                reason,
            }),
        }
    }
    hosts
}

fn resolve_alias(items: &[Item], alias: &str) -> Result<ImportedHost, SkipReason> {
    let mut active = true;
    let (mut hostname, mut user, mut port): (Option<&str>, Option<&str>, Option<&str>) =
        (None, None, None);
    let mut identity_files = Vec::new();
    let mut warnings: Vec<HostWarning> = Vec::new();
    let mut warn = |w: HostWarning| {
        if !warnings.contains(&w) {
            warnings.push(w);
        }
    };
    for item in items {
        match item {
            Item::Host(patterns) => active = host_matches(patterns, alias),
            Item::Match => active = false,
            Item::Option { key, args } if active => {
                let first = args.first().map(String::as_str);
                match key.as_str() {
                    "hostname" => hostname = hostname.or(first),
                    "user" => user = user.or(first),
                    "port" => port = port.or(first),
                    "identityfile" => {
                        if let Some(file) = first
                            && identity_files.len() < 16
                        {
                            identity_files.push(display(file));
                            warn(HostWarning::IdentityFileNotImported);
                        }
                    }
                    "proxyjump" | "proxycommand" if first.is_some_and(|v| v != "none") => {
                        warn(HostWarning::ProxyIgnored);
                    }
                    "localforward" | "remoteforward" | "dynamicforward" => {
                        warn(HostWarning::ForwardingIgnored);
                    }
                    _ => {}
                }
            }
            Item::Option { .. } => {}
        }
    }
    let hostname = expand_tokens(hostname.unwrap_or(alias), alias)?;
    check_value(&hostname)?;
    let port = match port {
        None => 22,
        Some(text) => text
            .parse::<u16>()
            .ok()
            .filter(|p| *p != 0)
            .ok_or(SkipReason::InvalidPort)?,
    };
    let user = match user {
        None => {
            warn(HostWarning::DefaultUser);
            None
        }
        Some(text) => {
            let text = expand_tokens(text, alias)?;
            if text.contains('%') {
                return Err(SkipReason::UnsupportedToken);
            }
            check_value(&text)?;
            Some(text)
        }
    };
    Ok(ImportedHost {
        alias: display(alias),
        hostname,
        port,
        user,
        identity_files,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
        path
    }

    fn parse(content: &str) -> (tempfile::TempDir, ConfigReport) {
        let home = tempfile::tempdir().unwrap();
        let ssh = home.path().join(".ssh");
        let config = write(&ssh, "config", content);
        let report = parse_ssh_config(&config, &ssh, home.path());
        (home, report)
    }

    fn host<'a>(report: &'a ConfigReport, alias: &str) -> &'a ImportedHost {
        report.hosts.iter().find(|h| h.alias == alias).unwrap()
    }

    #[test]
    fn tokenizes_like_openssh() {
        assert_eq!(tokenize("  # commentaire"), None);
        assert_eq!(
            tokenize("HostName=srv.lab"),
            Some(("hostname".into(), vec!["srv.lab".into()]))
        );
        assert_eq!(
            tokenize("Port = 2222"),
            Some(("port".into(), vec!["2222".into()]))
        );
        assert_eq!(
            tokenize(r#"IdentityFile "~/.ssh/ma clé""#),
            Some(("identityfile".into(), vec!["~/.ssh/ma clé".into()]))
        );
        assert_eq!(
            tokenize("Host a b\tc"),
            Some(("host".into(), vec!["a".into(), "b".into(), "c".into()]))
        );
    }

    #[test]
    fn first_value_wins_and_wildcards_give_defaults() {
        let (_home, report) = parse(
            "Host web\n  HostName 10.0.0.10\n  User deploy\n\
             Host *.lab db\n  Port 2222\n  User lab\n\
             Host *\n  User root\n  Port 22\n  HostName ignored.example\n",
        );
        let web = host(&report, "web");
        assert_eq!((web.hostname.as_str(), web.port), ("10.0.0.10", 22));
        assert_eq!(web.user.as_deref(), Some("deploy"));
        let db = host(&report, "db");
        assert_eq!((db.hostname.as_str(), db.port), ("ignored.example", 2222));
        assert_eq!(db.user.as_deref(), Some("lab"));
        let reasons: Vec<_> = report
            .skipped
            .iter()
            .map(|s| (s.entry.as_str(), s.reason))
            .collect();
        assert!(reasons.contains(&("Host *.lab", SkipReason::Wildcard)));
        assert!(reasons.contains(&("Host *", SkipReason::Wildcard)));
    }

    #[test]
    fn negations_exclude_an_alias() {
        let (_home, report) = parse(
            "Host bastion\n\
             Host * !bastion\n  ProxyJump bastion\n  User u\n",
        );
        let bastion = host(&report, "bastion");
        assert!(!bastion.warnings.contains(&HostWarning::ProxyIgnored));
        assert!(bastion.warnings.contains(&HostWarning::DefaultUser));
        assert_eq!(bastion.user, None);
    }

    #[test]
    fn includes_are_expanded_in_place_with_wildcards() {
        let home = tempfile::tempdir().unwrap();
        let ssh = home.path().join(".ssh");
        write(
            &ssh,
            "config.d/10-lab",
            "Host pve\n  HostName pve.lab\n  User root\n",
        );
        write(&ssh, "config.d/20-prod", "Host web\n  Port 2200\n");
        write(&ssh, "config.d/.cache", "Host cache\n");
        let absolute = write(home.path(), "autre/config", "Host abs\n  User a\n");
        let config = write(
            &ssh,
            "config",
            &format!(
                "Include config.d/*\nInclude {}\nInclude ~/absent/*\nHost web\n  User www\n",
                absolute.display()
            ),
        );
        let report = parse_ssh_config(&config, &ssh, home.path());
        let names: Vec<_> = report.hosts.iter().map(|h| h.alias.as_str()).collect();
        assert_eq!(names, ["pve", "web", "abs"]);
        let web = host(&report, "web");
        assert_eq!((web.port, web.user.as_deref()), (2200, Some("www")));
        assert!(report.skipped.contains(&Skipped {
            entry: "Include ~/absent/*".into(),
            reason: SkipReason::IncludeNotFound,
        }));
    }

    #[test]
    fn include_loops_and_depth_are_bounded() {
        let home = tempfile::tempdir().unwrap();
        let ssh = home.path().join(".ssh");
        write(&ssh, "boucle", "Include boucle\nHost h\n  User u\n");
        let config = write(&ssh, "config", "Include boucle\n");
        let report = parse_ssh_config(&config, &ssh, home.path());
        assert_eq!(report.hosts.len(), 1);
        assert!(
            report
                .skipped
                .iter()
                .any(|s| s.reason == SkipReason::LimitReached)
        );
    }

    #[test]
    fn match_blocks_are_ignored_and_reported() {
        let (_home, report) =
            parse("Match host *.corp exec \"true\"\n  User piege\nHost a\n  User u\n");
        assert_eq!(host(&report, "a").user.as_deref(), Some("u"));
        assert!(
            report
                .skipped
                .iter()
                .any(|s| s.reason == SkipReason::MatchBlock && s.entry.starts_with("Match host"))
        );
    }

    #[test]
    fn tokens_ports_and_values_are_checked() {
        let (_home, report) = parse(
            "Host h1\n  HostName %h.lab\n  User u\n\
             Host h2\n  HostName %d.lab\n\
             Host h3\n  Port 99999\n\
             Host h4\n  User %u\n\
             Host h5\n  HostName \"a b\"\n",
        );
        assert_eq!(host(&report, "h1").hostname, "h1.lab");
        let reason = |alias: &str| {
            report
                .skipped
                .iter()
                .find(|s| s.entry == format!("Host {alias}"))
                .map(|s| s.reason)
        };
        assert_eq!(reason("h2"), Some(SkipReason::UnsupportedToken));
        assert_eq!(reason("h3"), Some(SkipReason::InvalidPort));
        assert_eq!(reason("h4"), Some(SkipReason::UnsupportedToken));
        assert_eq!(reason("h5"), Some(SkipReason::InvalidValue));
    }

    #[test]
    fn warnings_and_duplicates() {
        let (_home, report) = parse(
            "Host a\n  HostName srv\n  User u\n  IdentityFile ~/.ssh/id_ed25519\n  LocalForward 8080 localhost:80\n  ProxyCommand ssh -W %h:%p bastion\n\
             Host b\n  HostName SRV\n  User u\n",
        );
        let a = host(&report, "a");
        assert_eq!(a.identity_files, ["~/.ssh/id_ed25519"]);
        for w in [
            HostWarning::IdentityFileNotImported,
            HostWarning::ForwardingIgnored,
            HostWarning::ProxyIgnored,
        ] {
            assert!(a.warnings.contains(&w));
        }
        assert!(report.skipped.contains(&Skipped {
            entry: "Host b".into(),
            reason: SkipReason::Duplicate,
        }));
    }

    #[cfg(unix)]
    #[test]
    fn special_files_are_not_opened() {
        let (_home, report) = parse("Include /dev/zero\nHost a\n  User u\n");
        assert_eq!(report.hosts.len(), 1);
        // Le périphérique n'est jamais ouvert : l'Include est signalé, le reste est lu.
        assert!(
            report
                .skipped
                .iter()
                .any(|s| s.entry == "Include /dev/zero")
        );
        assert_eq!(
            read_bounded(Path::new("/dev/zero")),
            Err(SkipReason::Unreadable)
        );
    }

    #[test]
    fn missing_config_is_reported() {
        let home = tempfile::tempdir().unwrap();
        let report = parse_ssh_config(&home.path().join("absent"), home.path(), home.path());
        assert!(report.hosts.is_empty());
        assert_eq!(report.skipped[0].reason, SkipReason::IncludeNotFound);
    }

    #[test]
    fn oversized_files_are_refused() {
        let (_home, report) = parse(&"#".repeat(MAX_FILE_SIZE as usize + 1));
        assert_eq!(report.skipped[0].reason, SkipReason::FileTooLarge);
    }
}
