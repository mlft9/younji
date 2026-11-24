//! Commandes des snippets (SNP-01). Les snippets vivent dans le coffre chiffré ; ce ne sont
//! pas des secrets (ils sont affichés et envoyés au terminal par l'interface). La validation
//! (longueurs, caractères de contrôle, variables) est faite par `core-vault`.

use core_vault::{EntryId, Snippet, SnippetInput, snippet_variables};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::vault_state::VaultState;

/// Snippet, tel qu'affiché par l'interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetDto {
    id: EntryId,
    name: String,
    content: String,
    description: String,
    /// Variables `{{nom}}`, dans l'ordre d'apparition (calculées par le cœur, qui fait foi).
    variables: Vec<String>,
}

impl From<&Snippet> for SnippetDto {
    fn from(snippet: &Snippet) -> Self {
        Self {
            id: snippet.id,
            name: snippet.name.clone(),
            content: snippet.content.clone(),
            description: snippet.description.clone(),
            variables: snippet_variables(&snippet.content)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

/// Création ou modification d'un snippet.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnippetRequest {
    id: Option<EntryId>,
    name: String,
    content: String,
    description: String,
}

/// Crée ou modifie un snippet. Renvoie son identifiant.
#[tauri::command]
pub async fn snippet_save(
    request: SnippetRequest,
    state: State<'_, VaultState>,
) -> Result<EntryId, AppError> {
    let input = SnippetInput {
        id: request.id,
        name: request.name,
        content: request.content,
        description: request.description,
    };
    state
        .update_blocking(move |data| data.upsert_snippet(input))
        .await
}

/// Supprime un snippet.
#[tauri::command]
pub async fn snippet_delete(id: EntryId, state: State<'_, VaultState>) -> Result<(), AppError> {
    state
        .update_blocking(move |data| data.delete_snippet(id))
        .await
}

#[cfg(test)]
mod tests {
    use core_vault::VaultData;

    use super::*;

    #[test]
    fn request_is_strict() {
        let ok = serde_json::json!({
            "id": null, "name": "n", "content": "ls", "description": ""
        });
        assert!(serde_json::from_value::<SnippetRequest>(ok.clone()).is_ok());
        let mut extra = ok.clone();
        extra["run"] = serde_json::json!(true);
        assert!(serde_json::from_value::<SnippetRequest>(extra).is_err());
        let mut bad_id = ok;
        bad_id["id"] = serde_json::json!("../x");
        assert!(serde_json::from_value::<SnippetRequest>(bad_id).is_err());
    }

    #[test]
    fn dto_lists_variables() {
        let mut data = VaultData::default();
        data.upsert_snippet(SnippetInput {
            id: None,
            name: "Restart".into(),
            content: "systemctl restart {{service}} && journalctl -u {{service}} -n {{lines}}"
                .into(),
            description: String::new(),
        })
        .unwrap();
        let dto = SnippetDto::from(&data.snippets()[0]);
        assert_eq!(dto.variables, vec!["service", "lines"]);
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["variables"][0], "service");
    }
}
