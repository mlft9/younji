//! Détection des shells installés (LOC-01, LOC-02).
//!
//! Règle de sécurité : un profil n'est lançable que s'il sort de cette détection. L'interface
//! le désigne par son identifiant ; le programme et ses arguments restent dans le cœur. Aucun
//! shell intermédiaire n'est utilisé : le programme est lancé directement, avec des arguments
//! séparés.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Longueur maximale d'un nom de distribution WSL accepté.
pub const MAX_WSL_NAME_LEN: usize = 64;
/// Nombre maximal de distributions WSL retenues.
pub const MAX_WSL_DISTROS: usize = 32;
/// Taille maximale lue dans `/etc/shells` ou dans la sortie de `wsl.exe`.
const MAX_LIST_LEN: usize = 64 * 1024;

/// Famille de profil (sert à l'icône et au libellé côté interface).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    /// Windows PowerShell 5.1.
    PowerShell,
    /// PowerShell 7 (`pwsh`).
    Pwsh,
    /// Invite de commandes.
    Cmd,
    /// Distribution WSL.
    Wsl,
    /// Shell Unix (bash, zsh, fish…).
    Shell,
}

impl ProfileKind {
    /// Nom stable, transmis à l'interface.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PowerShell => "powershell",
            Self::Pwsh => "pwsh",
            Self::Cmd => "cmd",
            Self::Wsl => "wsl",
            Self::Shell => "shell",
        }
    }
}

/// Shell local détecté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalProfile {
    /// Identifiant stable (`cmd`, `wsl:Ubuntu`, `shell:/usr/bin/zsh`…).
    pub id: String,
    /// Nom affiché (nom du programme ou de la distribution).
    pub name: String,
    /// Famille.
    pub kind: ProfileKind,
    /// Shell par défaut de l'utilisateur (`$SHELL`), ou premier profil sous Windows.
    pub is_default: bool,
    program: PathBuf,
    args: Vec<OsString>,
}

impl LocalProfile {
    fn new(
        id: String,
        name: String,
        kind: ProfileKind,
        program: PathBuf,
        args: Vec<OsString>,
    ) -> Self {
        Self {
            id,
            name,
            kind,
            is_default: false,
            program,
            args,
        }
    }

    /// Programme lancé (chemin absolu).
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Arguments, passés séparément au programme (jamais via un shell).
    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    /// Profil arbitraire, réservé aux tests de la crate.
    #[cfg(all(test, unix))]
    pub(crate) fn for_test(program: &str, args: &[&str]) -> Self {
        Self::new(
            "test".into(),
            "test".into(),
            ProfileKind::Shell,
            PathBuf::from(program),
            args.iter().map(OsString::from).collect(),
        )
    }
}

/// Détecte les shells installés sur ce poste. Ne lance aucun processus sous Linux ; sous
/// Windows, lance seulement `wsl.exe -l -q` (chemin absolu, délai borné) pour lister les
/// distributions.
pub fn detect_profiles() -> Vec<LocalProfile> {
    #[cfg(windows)]
    {
        windows::detect()
    }
    #[cfg(not(windows))]
    {
        let shells = read_bounded(Path::new("/etc/shells"));
        let shell_env = std::env::var_os("SHELL");
        detect_unix(shells.as_deref(), shell_env.as_deref(), &is_executable_file)
    }
}

/// Lit un petit fichier texte (au plus 64 Kio), ou `None`.
#[cfg_attr(windows, allow(dead_code))]
fn read_bounded(path: &Path) -> Option<String> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(MAX_LIST_LEN as u64)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// Vrai pour un fichier exécutable (lien résolu).
#[cfg_attr(windows, allow(dead_code))]
fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// Shells Unix retenus (dans cet ordre), parmi ceux de `/etc/shells`.
#[cfg_attr(windows, allow(dead_code))]
const UNIX_SHELLS: [&str; 8] = ["bash", "zsh", "fish", "ksh", "tcsh", "dash", "sh", "nu"];
/// Emplacements essayés si `/etc/shells` est absent.
#[cfg_attr(windows, allow(dead_code))]
const UNIX_FALLBACK: [&str; 6] = [
    "/bin/bash",
    "/usr/bin/bash",
    "/bin/zsh",
    "/usr/bin/zsh",
    "/usr/bin/fish",
    "/bin/sh",
];

/// Détection Unix : shells de `/etc/shells` (ou d'une liste fixe à défaut), un par nom.
/// `$SHELL` n'est retenu comme défaut que s'il est absolu **et** listé dans `/etc/shells` : une
/// variable d'environnement modifiée ne peut pas faire lancer un autre programme.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn detect_unix(
    shells_file: Option<&str>,
    shell_env: Option<&OsStr>,
    exists: &dyn Fn(&Path) -> bool,
) -> Vec<LocalProfile> {
    let listed: Vec<PathBuf> = match shells_file {
        Some(text) => text
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('/') && !l.chars().any(char::is_control))
            .map(PathBuf::from)
            .collect(),
        None => UNIX_FALLBACK.iter().map(PathBuf::from).collect(),
    };
    let default = shell_env
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && listed.contains(p) && exists(p));

    let mut profiles: Vec<LocalProfile> = Vec::new();
    let mut add = |path: &Path, is_default: bool| {
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            return;
        };
        if !UNIX_SHELLS.contains(&name) || !exists(path) {
            return;
        }
        if let Some(existing) = profiles.iter_mut().find(|p| p.name == name) {
            existing.is_default |= is_default;
            return;
        }
        let mut profile = LocalProfile::new(
            format!("shell:{}", path.display()),
            name.to_owned(),
            ProfileKind::Shell,
            path.to_path_buf(),
            Vec::new(),
        );
        profile.is_default = is_default;
        profiles.push(profile);
    };
    if let Some(default) = &default {
        add(default, true);
    }
    for path in &listed {
        add(path, false);
    }
    // Ordre stable : défaut d'abord, puis l'ordre de UNIX_SHELLS.
    profiles.sort_by_key(|p| (!p.is_default, UNIX_SHELLS.iter().position(|s| *s == p.name)));
    profiles
}

/// Nom de distribution WSL admis : lettres, chiffres, `.`, `_`, `-`, 1 à 64 caractères, sans
/// tiret initial (il serait lu comme une option de `wsl.exe`).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn is_valid_wsl_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_WSL_NAME_LEN
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Distributions internes de Docker Desktop : pas des environnements de travail.
#[cfg_attr(not(windows), allow(dead_code))]
const HIDDEN_WSL: [&str; 2] = ["docker-desktop", "docker-desktop-data"];

/// Analyse la sortie de `wsl.exe -l -q` (UTF-16LE, parfois avec BOM, une distribution par
/// ligne). Les noms invalides sont ignorés.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn parse_wsl_list(raw: &[u8]) -> Vec<String> {
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16_lossy(&units);
    let mut names: Vec<String> = Vec::new();
    for line in text.split(['\r', '\n']) {
        let name = line.trim_matches(|c: char| c == '\u{feff}' || c.is_whitespace() || c == '\0');
        if is_valid_wsl_name(name)
            && !HIDDEN_WSL.contains(&name)
            && !names.iter().any(|n| n == name)
            && names.len() < MAX_WSL_DISTROS
        {
            names.push(name.to_owned());
        }
    }
    names
}

/// Profil WSL pour une distribution validée.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn wsl_profile(wsl_exe: &Path, distro: &str) -> LocalProfile {
    LocalProfile::new(
        format!("wsl:{distro}"),
        distro.to_owned(),
        ProfileKind::Wsl,
        wsl_exe.to_path_buf(),
        vec!["-d".into(), distro.into(), "--cd".into(), "~".into()],
    )
}

#[cfg(windows)]
mod windows {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{LocalProfile, MAX_LIST_LEN, ProfileKind, parse_wsl_list, wsl_profile};

    /// Pas de fenêtre de console pour `wsl.exe -l`.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// Délai maximal de `wsl.exe -l -q`.
    const WSL_LIST_TIMEOUT: Duration = Duration::from_secs(5);

    /// Dossier Windows (`%SystemRoot%` s'il est absolu et existe, sinon `C:\Windows`).
    fn system_root() -> PathBuf {
        std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute() && p.is_dir())
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
    }

    fn program_files() -> Option<PathBuf> {
        std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute() && p.is_dir())
    }

    pub(super) fn detect() -> Vec<LocalProfile> {
        let system32 = system_root().join("System32");
        let mut profiles = Vec::new();
        let powershell = system32.join(r"WindowsPowerShell\v1.0\powershell.exe");
        if powershell.is_file() {
            profiles.push(LocalProfile::new(
                "powershell".into(),
                "Windows PowerShell".into(),
                ProfileKind::PowerShell,
                powershell,
                vec!["-NoLogo".into()],
            ));
        }
        if let Some(pf) = program_files() {
            for dir in ["7", "7-preview"] {
                let pwsh = pf.join("PowerShell").join(dir).join("pwsh.exe");
                if pwsh.is_file() {
                    profiles.push(LocalProfile::new(
                        "pwsh".into(),
                        "PowerShell 7".into(),
                        ProfileKind::Pwsh,
                        pwsh,
                        vec!["-NoLogo".into()],
                    ));
                    break;
                }
            }
        }
        let cmd = system32.join("cmd.exe");
        if cmd.is_file() {
            profiles.push(LocalProfile::new(
                "cmd".into(),
                "CMD".into(),
                ProfileKind::Cmd,
                cmd,
                Vec::new(),
            ));
        }
        let wsl = system32.join("wsl.exe");
        if wsl.is_file() {
            for distro in list_wsl(&wsl) {
                profiles.push(wsl_profile(&wsl, &distro));
            }
        }
        if let Some(first) = profiles.first_mut() {
            first.is_default = true;
        }
        profiles
    }

    /// Lance `wsl.exe -l -q` (sans fenêtre, délai borné, sortie bornée).
    fn list_wsl(wsl: &Path) -> Vec<String> {
        let child = Command::new(wsl)
            .args(["-l", "-q"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        let Ok(mut child) = child else {
            return Vec::new();
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            return Vec::new();
        };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut raw = Vec::new();
            let _ = stdout.take(MAX_LIST_LEN as u64).read_to_end(&mut raw);
            let _ = tx.send(raw);
        });
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if start.elapsed() < WSL_LIST_TIMEOUT => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    tracing::warn!("liste des distributions WSL indisponible");
                    return Vec::new();
                }
            }
        }
        rx.recv_timeout(Duration::from_secs(1))
            .map(|raw| parse_wsl_list(&raw))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_exist(_: &Path) -> bool {
        true
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn unix_shells_come_from_etc_shells_once_per_name() {
        let shells =
            "# commentaire\n/bin/sh\n/bin/bash\n/usr/bin/bash\n/usr/bin/zsh\n/usr/bin/tmux\n";
        let profiles = detect_unix(Some(shells), None, &all_exist);
        let names: Vec<_> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["bash", "zsh", "sh"]);
        assert_eq!(profiles[0].program(), Path::new("/bin/bash"));
        assert!(
            profiles
                .iter()
                .all(|p| !p.is_default && p.args().is_empty())
        );
    }

    #[test]
    fn listed_shell_env_is_the_default() {
        let shells = "/bin/bash\n/usr/bin/zsh\n";
        let profiles = detect_unix(Some(shells), Some(OsStr::new("/usr/bin/zsh")), &all_exist);
        assert_eq!(profiles[0].name, "zsh");
        assert!(profiles[0].is_default);
        assert_eq!(profiles[0].id, "shell:/usr/bin/zsh");
    }

    #[test]
    fn unlisted_or_relative_shell_env_is_ignored() {
        let shells = "/bin/bash\n";
        for env in ["/tmp/evil/bash", "bash", "/usr/bin/python3"] {
            let profiles = detect_unix(Some(shells), Some(OsStr::new(env)), &all_exist);
            assert_eq!(profiles.len(), 1, "{env}");
            assert_eq!(profiles[0].program(), Path::new("/bin/bash"));
            assert!(!profiles[0].is_default);
        }
    }

    #[test]
    fn missing_programs_are_skipped_and_fallback_is_used() {
        let only_sh = |p: &Path| p == Path::new("/bin/sh");
        let profiles = detect_unix(None, None, &only_sh);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "sh");
    }

    #[test]
    fn wsl_list_is_decoded_and_filtered() {
        let mut raw = vec![0xff, 0xfe];
        raw.extend(utf16(
            "Ubuntu-22.04\r\ndocker-desktop\r\nDebian\r\n-x\r\nbad name\r\nUbuntu-22.04\r\n\r\n",
        ));
        assert_eq!(parse_wsl_list(&raw), ["Ubuntu-22.04", "Debian"]);
    }

    #[test]
    fn wsl_names_are_validated() {
        assert!(is_valid_wsl_name("kali-linux"));
        assert!(is_valid_wsl_name("Ubuntu_24.04"));
        for bad in ["", "-d", "a b", "a;b", "a\"b", "é", &"a".repeat(65)] {
            assert!(!is_valid_wsl_name(bad), "{bad}");
        }
    }

    #[test]
    fn wsl_profile_passes_the_name_as_a_separate_argument() {
        let profile = wsl_profile(Path::new(r"C:\Windows\System32\wsl.exe"), "Debian");
        assert_eq!(profile.id, "wsl:Debian");
        assert_eq!(profile.kind, ProfileKind::Wsl);
        assert_eq!(profile.args(), ["-d", "Debian", "--cd", "~"]);
    }
}
