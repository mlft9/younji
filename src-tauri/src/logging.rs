//! Journalisation applicative (APP-08, base).
//!
//! Fichiers rotatifs quotidiens dans le dossier de journaux de l'application, 7 fichiers au plus.
//! Règle absolue : aucun secret ni contenu de terminal ou de fichier transféré dans les journaux
//! (SECURITE.md §6). La variable `YOUNJI_LOG` règle le niveau (`error` à `trace`, `info` par
//! défaut) du **seul code de Younji** : les dépendances restent plafonnées à `warn`, pour qu'un
//! niveau de diagnostic élevé ne fasse jamais écrire à une bibliothèque des paquets réseau ou
//! des arguments IPC (donc potentiellement des secrets) dans les journaux.

use std::io;
use std::path::Path;

use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

/// Variable d'environnement qui règle le niveau des journaux.
pub const LOG_ENV_VAR: &str = "YOUNJI_LOG";
const DEFAULT_LEVEL: LevelFilter = LevelFilter::INFO;
/// Plafond imposé aux dépendances, quel que soit `YOUNJI_LOG`.
const THIRD_PARTY_LEVEL: LevelFilter = LevelFilter::WARN;
/// Cibles (noms de crates) du code de Younji. Chaque nouvelle crate `core_*` s'y ajoute.
const OWN_TARGETS: &[&str] = &[
    "younji",
    "younji_lib",
    "core_import",
    "core_rdp",
    "core_ssh",
    "core_term",
    "core_vault",
];
const FILE_PREFIX: &str = "younji";
const FILE_SUFFIX: &str = "log";
const MAX_LOG_FILES: usize = 7;

/// Erreurs de mise en place des journaux. Elles ne sont jamais fatales.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("impossible de créer le dossier des journaux")]
    CreateDir(#[source] io::Error),
    #[error("impossible d'ouvrir le fichier de journal")]
    Appender(#[source] tracing_appender::rolling::InitError),
    #[error("dossier des journaux inconnu")]
    NoLogDir,
    #[error("un collecteur de journaux est déjà installé")]
    AlreadyInitialized,
}

/// Garde à conserver pendant toute la vie de l'application : à sa destruction,
/// les derniers journaux en attente sont écrits sur le disque.
pub struct LogGuard {
    _worker: Option<WorkerGuard>,
}

/// Crée le dossier des journaux avec des permissions restrictives (0700 sous Linux).
fn create_log_dir(dir: &Path) -> Result<(), LogError> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir).map_err(LogError::CreateDir)?;

    // Si le dossier existait déjà avec des droits plus larges, on les resserre.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(LogError::CreateDir)?;
    }
    Ok(())
}

/// Ouvre l'écrivain non bloquant vers les fichiers rotatifs de `dir`.
fn file_writer(dir: &Path) -> Result<(NonBlocking, WorkerGuard), LogError> {
    create_log_dir(dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(FILE_PREFIX)
        .filename_suffix(FILE_SUFFIX)
        .max_log_files(MAX_LOG_FILES)
        .build(dir)
        .map_err(LogError::Appender)?;
    Ok(tracing_appender::non_blocking(appender))
}

/// Construit les directives de filtrage à partir de la valeur de `YOUNJI_LOG`.
/// Seul un niveau est accepté ; toute autre valeur (directive par cible, texte invalide) est
/// ignorée au profit du niveau par défaut.
fn filter_directives(requested: Option<&str>) -> String {
    let level = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<LevelFilter>().ok())
        .unwrap_or(DEFAULT_LEVEL);
    let mut directives = THIRD_PARTY_LEVEL.to_string().to_lowercase();
    for target in OWN_TARGETS {
        directives.push_str(&format!(",{target}={}", level.to_string().to_lowercase()));
    }
    directives
}

fn env_filter() -> EnvFilter {
    let requested = std::env::var(LOG_ENV_VAR).ok();
    EnvFilter::new(filter_directives(requested.as_deref()))
}

/// Installe le collecteur global. En cas d'échec sur le fichier, on se replie sur la sortie
/// d'erreur standard plutôt que d'empêcher l'application de démarrer.
pub fn init(dir: Option<&Path>) -> LogGuard {
    match dir.ok_or(LogError::NoLogDir).and_then(file_writer) {
        Ok((writer, worker)) => {
            // En développement, les journaux sont aussi affichés dans la console.
            #[cfg(debug_assertions)]
            let writer = writer.and(io::stderr);
            let installed = tracing_subscriber::fmt()
                .with_env_filter(env_filter())
                .with_ansi(false)
                .with_writer(writer)
                .try_init()
                .map_err(|_| LogError::AlreadyInitialized);
            if let Err(error) = installed {
                eprintln!("journalisation : {error}");
            }
            LogGuard {
                _worker: Some(worker),
            }
        }
        Err(error) => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(env_filter())
                .with_writer(io::stderr)
                .try_init();
            tracing::warn!(%error, "journaux sur fichier indisponibles, repli sur la console");
            LogGuard { _worker: None }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_files(dir: &Path) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == FILE_SUFFIX))
            .collect()
    }

    #[test]
    fn log_level_applies_only_to_younji_targets() {
        assert_eq!(
            filter_directives(None),
            "warn,younji=info,younji_lib=info,core_import=info,core_rdp=info,core_ssh=info,core_term=info,core_vault=info"
        );
        assert_eq!(
            filter_directives(Some("TRACE")),
            "warn,younji=trace,younji_lib=trace,core_import=trace,core_rdp=trace,core_ssh=trace,core_term=trace,core_vault=trace"
        );
        assert_eq!(
            filter_directives(Some(" debug ")),
            "warn,younji=debug,younji_lib=debug,core_import=debug,core_rdp=debug,core_ssh=debug,core_term=debug,core_vault=debug"
        );
    }

    #[test]
    fn rejects_per_target_directives() {
        // Une directive qui viserait une dépendance (ex. russh=trace) est ignorée.
        for value in ["russh=trace", "trace,tauri=trace", "[{}]", ""] {
            assert_eq!(
                filter_directives(Some(value)),
                "warn,younji=info,younji_lib=info,core_import=info,core_rdp=info,core_ssh=info,core_term=info,core_vault=info",
                "{value}"
            );
        }
    }

    #[test]
    fn third_party_trace_events_are_filtered_out() {
        use tracing::Level;
        use tracing_subscriber::layer::SubscriberExt;

        let subscriber =
            tracing_subscriber::registry().with(EnvFilter::new(filter_directives(Some("trace"))));
        tracing::subscriber::with_default(subscriber, || {
            assert!(tracing::enabled!(target: "younji_lib::ssh", Level::TRACE));
            assert!(!tracing::enabled!(target: "russh::cipher", Level::TRACE));
            assert!(!tracing::enabled!(target: "tauri::ipc", Level::INFO));
            assert!(tracing::enabled!(target: "tauri::ipc", Level::WARN));
        });
    }

    #[test]
    fn writes_to_a_rotating_file_in_a_new_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("logs");

        let (writer, worker) = file_writer(&dir).unwrap();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(writer)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("message de test");
        });
        drop(worker); // vide le tampon sur le disque

        let files = log_files(&dir);
        assert_eq!(files.len(), 1);
        let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("younji."), "nom inattendu : {name}");
        let content = std::fs::read_to_string(&files[0]).unwrap();
        assert!(content.contains("message de test"));
    }

    #[cfg(unix)]
    #[test]
    fn log_directory_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("logs");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

        create_log_dir(&dir).unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[test]
    fn reports_an_unusable_directory_without_panicking() {
        let tmp = tempfile::tempdir().unwrap();
        // Un fichier ordinaire à la place du dossier attendu.
        let blocker = tmp.path().join("pas-un-dossier");
        std::fs::write(&blocker, b"").unwrap();

        assert!(matches!(
            file_writer(&blocker),
            Err(LogError::CreateDir(_) | LogError::Appender(_))
        ));
    }
}
