fn main() {
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
