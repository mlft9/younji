//! Informations générales sur l'application.

use serde::Serialize;

use crate::APP_NAME;

/// Réponse de `app_info` (miroir de `AppInfo` dans `src/lib/types.ts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
    /// Commit compilé (7 caractères hexadécimaux, ou « inconnu »).
    commit: &'static str,
}

fn current_app_info() -> AppInfo {
    AppInfo {
        name: APP_NAME,
        version: env!("CARGO_PKG_VERSION"),
        commit: env!("YOUNJI_BUILD_COMMIT"),
    }
}

/// Renvoie le nom, la version et le commit compilé de l'application. Aucune entrée, aucune donnée sensible.
#[tauri::command]
pub fn app_info() -> AppInfo {
    current_app_info()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_name_and_version() {
        let json = serde_json::to_value(current_app_info()).unwrap();
        assert_eq!(json["name"], "Younji");
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
        let commit = json["commit"].as_str().unwrap();
        assert!(
            commit == "inconnu"
                || (commit.len() == 7 && commit.chars().all(|c| c.is_ascii_hexdigit()))
        );
        assert_eq!(json.as_object().unwrap().len(), 3);
    }
}
