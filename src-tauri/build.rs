/// Identifiant court du commit compilé, affiché dans les Paramètres pour savoir quelle
/// compilation est testée (la version reste la même d'un build à l'autre). En CI,
/// `GITHUB_SHA` ; en local, `git rev-parse`. Seuls des chiffres hexadécimaux sont retenus.
fn build_commit() -> String {
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    let from_ci = std::env::var("GITHUB_SHA").ok();
    let from_git = || {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
    };
    from_ci
        .or_else(from_git)
        .map(|sha| sha.trim().chars().take(7).collect::<String>())
        .filter(|sha| sha.len() == 7 && sha.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| "inconnu".to_owned())
}

fn main() {
    println!("cargo:rustc-env=YOUNJI_BUILD_COMMIT={}", build_commit());
    // Les commandes applicatives sont déclarées ici : Tauri génère pour chacune une permission
    // `allow-<commande>`, et une commande n'est appelable que si une capability l'accorde
    // explicitement (src-tauri/capabilities/). Toute nouvelle commande doit aussi être ajoutée
    // à la liste blanche de l'isolation (isolation/index.js) ; un test vérifie la cohérence.
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "app_info",
            "ssh_connect",
            "ssh_write",
            "ssh_resize",
            "ssh_ack",
            "ssh_host_key_decision",
            "ssh_close",
            "ssh_pick_key_file",
            "open_external_url",
            "ssh_connect_host",
            "vault_status",
            "vault_create",
            "vault_unlock",
            "vault_lock",
            "vault_change_password",
            "vault_touch",
            "vault_watch",
            "vault_set_auto_lock",
            "vault_snapshot",
            "host_save",
            "host_delete",
            "group_save",
            "group_delete",
            "snippet_save",
            "snippet_delete",
            "identity_save",
            "identity_delete",
            "key_import",
            "key_rename",
            "key_delete",
            "known_host_list",
            "known_host_delete",
            "settings_get",
            "settings_set",
            "ssh_auth_answer",
            "key_generate",
            "key_export_public",
            "key_deploy",
            "sftp_open",
            "sftp_close",
            "sftp_home",
            "sftp_list",
            "sftp_mkdir",
            "sftp_rename",
            "sftp_remove",
            "sftp_chmod",
            "sftp_download",
            "sftp_upload",
            "sftp_transfer_cancel",
            "sftp_conflict_decision",
            "sftp_edit",
            "sftp_edit_close",
            "sftp_edit_force",
            "local_pick_root",
            "local_list",
        ]));

    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("échec de tauri-build : {error:#}");
        std::process::exit(1);
    }
}
