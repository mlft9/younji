//! Application Tauri de Younji : elle ne fait que relier les commandes IPC aux crates métier
//! (ADR-004). Aucune logique métier ici.

mod commands;
mod editor;
mod error;
mod logging;
mod navigation;
mod settings;
mod sftp_state;
mod state;
mod vault_state;

use std::sync::Arc;
use std::time::Duration;

use core_ssh::KnownHosts;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::vault::lock_everything;
use crate::settings::SettingsStore;
use crate::state::SessionManager;
use crate::vault_state::{Activity, LockReason, VaultState};

/// Nom de l'application côté Rust (ADR-014). Seule autre source : `productName` dans
/// `tauri.conf.json` et la clé i18n `app.name`.
pub const APP_NAME: &str = "Younji";

/// Étiquette de la fenêtre principale ; elle doit correspondre à `capabilities/main.json`.
const MAIN_WINDOW: &str = "main";
/// Ancien fichier des clés d'hôte en clair (étape 1), repris dans le coffre à sa création.
const LEGACY_KNOWN_HOSTS_FILE: &str = "known_hosts";
/// Fichier du coffre chiffré (SECURITE.md §3.3).
const VAULT_FILE: &str = "vault.younji";
/// Paramètres d'apparence (en clair, sans donnée sensible).
const SETTINGS_FILE: &str = "settings.json";
/// Fréquence de vérification du verrouillage automatique.
const AUTO_LOCK_TICK: Duration = Duration::from_secs(5);

/// Verrouille le coffre quand le délai d'inactivité est écoulé. Le minuteur tourne dans le
/// cœur : l'interface ne peut pas l'arrêter, seulement signaler une activité.
fn spawn_auto_lock(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(AUTO_LOCK_TICK).await;
            if app.state::<VaultState>().idle_lock_due(Activity::now()) {
                lock_everything(&app, LockReason::Idle);
            }
        }
    });
}

/// Démarre l'application.
pub fn run() {
    let result = tauri::Builder::default()
        // Plugin utilisé uniquement depuis Rust (sélection d'une clé privée) : aucune de ses
        // permissions n'est accordée à l'interface.
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Sans dossier de journaux connu, on se replie sur la console : jamais sur un
            // dossier temporaire partagé, dont le nom serait prévisible pour un autre utilisateur.
            let guard = logging::init(app.path().app_log_dir().ok().as_deref());
            app.manage(guard);
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                "démarrage de {APP_NAME}"
            );

            let data_dir = app.path().app_data_dir()?;
            let vault = VaultState::new(
                data_dir.join(VAULT_FILE),
                data_dir.join(LEGACY_KNOWN_HOSTS_FILE),
            );
            // Les clés d'hôte connues sont stockées dans le coffre (chiffrées et authentifiées).
            let known_hosts = Arc::new(KnownHosts::with_storage(vault.known_hosts_storage()));
            app.manage(SessionManager::new(known_hosts));
            app.manage(vault);
            app.manage(SettingsStore::new(data_dir.join(SETTINGS_FILE)));
            // Copies d'édition SFTP abandonnées (exécution interrompue il y a plus de 24 h) :
            // supprimées. Les plus récentes peuvent appartenir à une autre instance.
            if let Ok(dir) = commands::sftp::edit_dir(app.handle()) {
                let removed =
                    core_ssh::sftp::edit::cleanup_stale(&dir, core_ssh::sftp::edit::STALE_AGE);
                if removed > 0 {
                    tracing::info!(removed, "copies d'édition restantes supprimées");
                }
            }
            spawn_auto_lock(app.handle().clone());

            // Le serveur Vite n'est autorisé qu'en développement.
            let dev_url = if cfg!(dev) {
                app.config().build.dev_url.clone()
            } else {
                None
            };

            // La fenêtre est créée ici plutôt que dans la configuration pour que son titre
            // vienne de APP_NAME. Elle ne charge que le contenu embarqué de l'application,
            // ne peut pas naviguer ailleurs et ne peut pas ouvrir d'autre fenêtre.
            WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
                .title(APP_NAME)
                .on_navigation(move |url| {
                    let allowed = navigation::is_allowed_navigation(url, dev_url.as_ref());
                    if !allowed {
                        // Seul le schéma est journalisé : l'adresse complète pourrait contenir
                        // des données sensibles.
                        tracing::warn!(scheme = url.scheme(), "navigation refusée");
                    }
                    allowed
                })
                // Un rechargement de l'interface (F5, erreur) perd les Channel : les sessions
                // n'auraient plus personne pour les piloter. On les ferme et on verrouille le
                // coffre, ce qui efface aussi les secrets de la mémoire.
                .on_page_load(|window, payload| {
                    if payload.event() == PageLoadEvent::Started {
                        lock_everything(window.app_handle(), LockReason::Reload);
                    }
                })
                .on_new_window(|url, _| {
                    tracing::warn!(scheme = url.scheme(), "ouverture de fenêtre refusée");
                    NewWindowResponse::Deny
                })
                // Gestionnaire natif de glisser-déposer désactivé : il empêche le glisser-déposer
                // HTML5 entre les panneaux SFTP sous Windows, et il exposerait à l'interface les
                // chemins des fichiers déposés depuis le système (ADR-034).
                .disable_drag_drop_handler()
                .inner_size(1200.0, 780.0)
                .min_inner_size(800.0, 500.0)
                .center()
                .build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::ssh::ssh_connect,
            commands::ssh::ssh_write,
            commands::ssh::ssh_resize,
            commands::ssh::ssh_ack,
            commands::ssh::ssh_host_key_decision,
            commands::ssh::ssh_close,
            commands::ssh::ssh_pick_key_file,
            commands::ssh::ssh_connect_host,
            commands::ssh::ssh_auth_answer,
            commands::ssh::key_deploy,
            commands::url::open_external_url,
            commands::vault::vault_status,
            commands::vault::vault_create,
            commands::vault::vault_unlock,
            commands::vault::vault_lock,
            commands::vault::vault_change_password,
            commands::vault::vault_touch,
            commands::vault::vault_watch,
            commands::vault::vault_set_auto_lock,
            commands::book::vault_snapshot,
            commands::book::host_save,
            commands::book::host_delete,
            commands::book::group_save,
            commands::book::group_delete,
            commands::snippets::snippet_save,
            commands::snippets::snippet_delete,
            commands::tunnels::tunnel_save,
            commands::tunnels::tunnel_delete,
            commands::tunnels::tunnel_start,
            commands::book::identity_save,
            commands::book::identity_delete,
            commands::book::key_import,
            commands::book::key_rename,
            commands::book::key_delete,
            commands::book::key_generate,
            commands::book::key_export_public,
            commands::book::known_host_list,
            commands::book::known_host_delete,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::sftp::sftp_open,
            commands::sftp::sftp_close,
            commands::sftp::sftp_home,
            commands::sftp::sftp_list,
            commands::sftp::sftp_mkdir,
            commands::sftp::sftp_rename,
            commands::sftp::sftp_remove,
            commands::sftp::sftp_chmod,
            commands::sftp::sftp_download,
            commands::sftp::sftp_upload,
            commands::sftp::sftp_transfer_cancel,
            commands::sftp::sftp_conflict_decision,
            commands::sftp::sftp_edit,
            commands::sftp::sftp_edit_close,
            commands::sftp::sftp_edit_force,
            commands::sftp::local_pick_root,
            commands::sftp::local_list,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        tracing::error!(%error, "arrêt sur erreur");
        eprintln!("{APP_NAME} n'a pas pu démarrer : {error}");
        std::process::exit(1);
    }
}
