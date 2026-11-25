//! Détection du client RDP, lancement supervisé et nettoyage.
//!
//! - **Windows** : fichier `.rdp` temporaire (nom aléatoire, sans mot de passe) et identifiant
//!   `TERMSRV/<hôte>` écrit par l'API du gestionnaire d'identifiants (jamais `cmdkey /pass:`).
//!   Les deux sont supprimés à la fin de mstsc, au plus tard [`CLEANUP_DELAY`] après le
//!   lancement, et à l'arrêt demandé par Younji (verrouillage).
//! - **Linux** : `xfreerdp3` ou `xfreerdp`, mot de passe écrit sur l'entrée standard puis tube
//!   fermé (`/from-stdin:force`).
//!
//! Le mot de passe n'apparaît jamais dans les arguments ni dans l'environnement du processus.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use secrecy::SecretString;
use tempfile::TempPath;

use crate::RdpError;
use crate::target::RdpTarget;

/// Délai maximal de conservation de l'identifiant et du fichier `.rdp` après le lancement.
pub const CLEANUP_DELAY: Duration = Duration::from_secs(60);
/// Fréquence de surveillance du client.
const POLL: Duration = Duration::from_millis(250);
/// Préfixe des fichiers `.rdp` temporaires.
const RDP_FILE_PREFIX: &str = "younji-";

/// Client RDP du système.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RdpClient {
    /// `mstsc.exe` (Windows).
    Mstsc(PathBuf),
    /// `xfreerdp3` / `xfreerdp` (Linux).
    FreeRdp(PathBuf),
}

/// Cherche le client du système : `%SystemRoot%\System32\mstsc.exe` sous Windows ; sous les
/// autres systèmes, `xfreerdp3` puis `xfreerdp` dans les seuls dossiers **absolus** du `PATH`.
pub fn find_client() -> Result<RdpClient, RdpError> {
    #[cfg(windows)]
    {
        let root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute() && p.is_dir())
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let mstsc = root.join("System32").join("mstsc.exe");
        if mstsc.is_file() {
            Ok(RdpClient::Mstsc(mstsc))
        } else {
            Err(RdpError::ClientNotFound)
        }
    }
    #[cfg(not(windows))]
    {
        find_freerdp(std::env::var_os("PATH").as_deref())
            .map(RdpClient::FreeRdp)
            .ok_or(RdpError::ClientNotFound)
    }
}

/// Recherche de FreeRDP dans un `PATH` (dossiers relatifs ignorés : ils dépendraient du
/// dossier courant).
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn find_freerdp(path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = path
        .map(|p| {
            std::env::split_paths(p)
                .filter(|d| d.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    ["xfreerdp3", "xfreerdp"].iter().find_map(|name| {
        dirs.iter()
            .map(|dir| dir.join(name))
            .find(|candidate| is_executable(candidate))
    })
}

#[cfg_attr(windows, allow(dead_code))]
fn is_executable(path: &Path) -> bool {
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

/// Traces temporaires d'un lancement, supprimées une seule fois (et à la destruction).
struct Cleanup {
    rdp_file: Option<TempPath>,
    #[cfg_attr(not(windows), allow(dead_code))]
    credential: Option<String>,
}

impl Cleanup {
    fn run(&mut self) {
        if let Some(file) = self.rdp_file.take()
            && file.close().is_err()
        {
            tracing::warn!("fichier .rdp temporaire non supprimé");
        }
        #[cfg(windows)]
        if let Some(target) = self.credential.take() {
            crate::wincred::delete(&target);
        }
        #[cfg(not(windows))]
        {
            self.credential = None;
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.run();
    }
}

struct Inner {
    child: Child,
    cleanup: Cleanup,
    running: bool,
}

/// Client RDP lancé par Younji. [`RdpProcess::terminate`] l'arrête et nettoie.
pub struct RdpProcess {
    inner: Arc<Mutex<Inner>>,
    existing_credential: bool,
}

impl std::fmt::Debug for RdpProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RdpProcess").finish_non_exhaustive()
    }
}

impl RdpProcess {
    /// Vrai tant que le client tourne.
    pub fn is_running(&self) -> bool {
        self.inner.lock().map(|i| i.running).unwrap_or(false)
    }

    /// Windows : un identifiant enregistré par l'utilisateur existait déjà pour ce serveur ; il
    /// a été laissé intact et mstsc l'utilise.
    pub fn used_existing_credential(&self) -> bool {
        self.existing_credential
    }

    /// Arrête le client (verrouillage) et supprime ses traces.
    pub fn terminate(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.running {
                let _ = inner.child.kill();
                let _ = inner.child.wait();
                inner.running = false;
            }
            inner.cleanup.run();
        }
    }
}

/// Lance le client pour la cible. `password` : mot de passe de l'identité (facultatif sous
/// Windows, où mstsc le demande lui-même ; obligatoire avec FreeRDP). `temp_dir` : dossier
/// privé de l'application pour le fichier `.rdp`.
pub fn launch(
    client: &RdpClient,
    target: &RdpTarget,
    password: Option<&SecretString>,
    temp_dir: &Path,
) -> Result<RdpProcess, RdpError> {
    let (child, cleanup, existing_credential) = match client {
        RdpClient::Mstsc(program) => spawn_mstsc(program, target, password, temp_dir)?,
        RdpClient::FreeRdp(program) => (
            spawn_freerdp(program, target, password)?,
            Cleanup {
                rdp_file: None,
                credential: None,
            },
            false,
        ),
    };
    let inner = Arc::new(Mutex::new(Inner {
        child,
        cleanup,
        running: true,
    }));
    supervise(inner.clone());
    tracing::debug!("client RDP lancé");
    Ok(RdpProcess {
        inner,
        existing_credential,
    })
}

/// Surveille le client : nettoyage à sa fin ou au plus tard après [`CLEANUP_DELAY`], puis
/// attente de sa fin pour ne pas laisser de processus zombie.
fn supervise(inner: Arc<Mutex<Inner>>) {
    let spawned = std::thread::Builder::new()
        .name("younji-rdp".into())
        .spawn(move || {
            let start = Instant::now();
            loop {
                std::thread::sleep(POLL);
                let Ok(mut guard) = inner.lock() else {
                    return;
                };
                if !guard.running {
                    guard.cleanup.run();
                    return;
                }
                match guard.child.try_wait() {
                    Ok(Some(_)) | Err(_) => {
                        guard.running = false;
                        guard.cleanup.run();
                        tracing::debug!("client RDP terminé");
                        return;
                    }
                    Ok(None) => {
                        if start.elapsed() >= CLEANUP_DELAY {
                            guard.cleanup.run();
                        }
                    }
                }
            }
        });
    if spawned.is_err() {
        tracing::warn!("surveillance du client RDP impossible");
    }
}

fn spawn_freerdp(
    program: &Path,
    target: &RdpTarget,
    password: Option<&SecretString>,
) -> Result<Child, RdpError> {
    use std::io::Write;

    let password = password.ok_or(RdpError::PasswordRequired)?;
    let input = crate::freerdp::freerdp_stdin(target, password)?;
    let mut child = Command::new(program)
        .args(crate::freerdp::freerdp_args(target, true))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            tracing::warn!(%error, "lancement de FreeRDP impossible");
            RdpError::LaunchFailed
        })?;
    // Écriture puis fermeture immédiate du tube : FreeRDP ne lit rien d'autre sur l'entrée.
    let written = child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(&input).and_then(|()| stdin.flush()));
    if !matches!(written, Some(Ok(()))) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(RdpError::LaunchFailed);
    }
    Ok(child)
}

#[cfg_attr(not(windows), allow(unused_variables, unused_mut))]
fn spawn_mstsc(
    program: &Path,
    target: &RdpTarget,
    password: Option<&SecretString>,
    temp_dir: &Path,
) -> Result<(Child, Cleanup, bool), RdpError> {
    let mut cleanup = Cleanup {
        rdp_file: Some(write_rdp_file(target, temp_dir)?),
        credential: None,
    };
    let mut existing = false;
    #[cfg(windows)]
    if let Some(password) = password {
        let name = crate::wincred::target_name(target.address());
        if crate::wincred::exists(&name) {
            // Identifiant de l'utilisateur : ni écrasé, ni supprimé.
            existing = true;
        } else {
            let username = if target.domain().is_empty() {
                target.username().to_owned()
            } else {
                format!("{}\\{}", target.domain(), target.username())
            };
            crate::wincred::write(&name, &username, password)?;
            cleanup.credential = Some(name);
        }
    }
    let file = cleanup
        .rdp_file
        .as_ref()
        .map(|f| f.to_path_buf())
        .ok_or(RdpError::TempFileFailed)?;
    let child = Command::new(program)
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            tracing::warn!(%error, "lancement de mstsc impossible");
            RdpError::LaunchFailed
        })?;
    // En cas d'échec ci-dessus, `cleanup` est détruit : fichier et identifiant supprimés.
    Ok((child, cleanup, existing))
}

/// Écrit le fichier `.rdp` (nom aléatoire, 0600 sous Unix) dans le dossier privé.
fn write_rdp_file(target: &RdpTarget, dir: &Path) -> Result<TempPath, RdpError> {
    use std::io::Write;

    create_private_dir(dir).map_err(|_| RdpError::TempFileFailed)?;
    let mut file = tempfile::Builder::new()
        .prefix(RDP_FILE_PREFIX)
        .suffix(".rdp")
        .tempfile_in(dir)
        .map_err(|_| RdpError::TempFileFailed)?;
    file.write_all(crate::rdpfile::rdp_file(target).as_bytes())
        .and_then(|()| file.flush())
        .map_err(|_| RdpError::TempFileFailed)?;
    Ok(file.into_temp_path())
}

fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Supprime les fichiers `.rdp` restés d'une exécution interrompue. Renvoie leur nombre.
pub fn cleanup_stale(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with(RDP_FILE_PREFIX) && name.ends_with(".rdp")
        })
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| std::fs::remove_file(e.path()).is_ok())
        .count()
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::Resolution;

    fn target() -> RdpTarget {
        RdpTarget::new("win.lab", 3389, "alice", "LAB", Resolution::Fullscreen).unwrap()
    }

    /// Faux client : enregistre ses arguments et son entrée standard, puis attend un peu.
    fn fake_client(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let out = dir.display();
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > {out}/args\nenv > {out}/env\ncat > {out}/stdin\nsleep 30\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn wait_for(path: &Path) -> String {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            if let Ok(text) = std::fs::read_to_string(path)
                && !text.is_empty()
            {
                return text;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("fichier attendu absent");
    }

    #[test]
    fn freerdp_gets_the_password_on_stdin_only() {
        let dir = tempfile::tempdir().unwrap();
        let client = RdpClient::FreeRdp(fake_client(dir.path(), "xfreerdp3"));
        let password = SecretString::from("Mot-De-Passe-Unique-42");
        let process = launch(&client, &target(), Some(&password), dir.path()).unwrap();
        let stdin = wait_for(&dir.path().join("stdin"));
        assert_eq!(stdin, "Mot-De-Passe-Unique-42\n");
        let args = wait_for(&dir.path().join("args"));
        assert!(!args.contains("Mot-De-Passe-Unique-42"), "{args}");
        assert!(args.contains("/from-stdin:force"));
        let env = wait_for(&dir.path().join("env"));
        assert!(!env.contains("Mot-De-Passe-Unique-42"));
        assert!(process.is_running());
        process.terminate();
        assert!(!process.is_running());
    }

    #[test]
    fn freerdp_requires_a_password() {
        let dir = tempfile::tempdir().unwrap();
        let client = RdpClient::FreeRdp(fake_client(dir.path(), "xfreerdp"));
        assert_eq!(
            launch(&client, &target(), None, dir.path()).unwrap_err(),
            RdpError::PasswordRequired
        );
    }

    #[test]
    fn the_rdp_file_is_private_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let rdp_dir = dir.path().join("rdp");
        let client = RdpClient::Mstsc(fake_client(dir.path(), "mstsc"));
        let process = launch(&client, &target(), None, &rdp_dir).unwrap();
        let args = wait_for(&dir.path().join("args"));
        let file = PathBuf::from(args.trim());
        assert!(file.starts_with(&rdp_dir));
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0);
        assert_eq!(
            std::fs::metadata(&rdp_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(
            std::fs::read_to_string(&file)
                .unwrap()
                .contains("full address:s:win.lab:3389")
        );
        process.terminate();
        assert!(!file.exists());
    }

    #[test]
    fn the_rdp_file_is_removed_when_the_client_exits() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("quick");
        std::fs::write(&program, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let process = launch(&RdpClient::Mstsc(program), &target(), None, dir.path()).unwrap();
        let start = Instant::now();
        while process.is_running() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!process.is_running());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(cleanup_stale(dir.path()), 0);
    }

    #[test]
    fn freerdp_is_searched_in_absolute_path_entries_only() {
        let dir = tempfile::tempdir().unwrap();
        let found = fake_client(dir.path(), "xfreerdp");
        let path =
            std::env::join_paths([PathBuf::from("relative"), dir.path().to_path_buf()]).unwrap();
        assert_eq!(find_freerdp(Some(&path)), Some(found));
        assert_eq!(
            find_freerdp(Some(std::ffi::OsStr::new("relative:bin"))),
            None
        );
        assert_eq!(find_freerdp(None), None);
    }

    #[test]
    fn stale_rdp_files_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("younji-abc.rdp"), "x").unwrap();
        std::fs::write(dir.path().join("autre.rdp"), "x").unwrap();
        assert_eq!(cleanup_stale(dir.path()), 1);
        assert!(dir.path().join("autre.rdp").exists());
    }
}
