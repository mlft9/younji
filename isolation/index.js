// Application d'isolation Tauri (ADR-015).
//
// Tauri fait transiter chaque appel IPC de l'interface par cette iframe sandboxée,
// sur une origine distincte, avant de le chiffrer pour le cœur Rust. Une dépendance npm
// compromise dans l'interface ne peut donc ni lire ce script ni le contourner.
//
// Règle : seules les commandes de la liste blanche passent. Toute autre commande est
// remplacée par une commande inexistante : le cœur la rejette avec une erreur, ce qui
// fait échouer l'appel proprement (lever une exception ici le laisserait bloqué).
//
// Ce fichier doit rester autonome : aucune dépendance, aucune étape de compilation.
(function (root) {
  'use strict';

  /** Commandes que l'interface a le droit d'appeler. À tenir à jour avec build.rs et les capabilities. */
  var ALLOWED_COMMANDS = Object.freeze([
    'app_info',
    'ssh_connect',
    'ssh_write',
    'ssh_resize',
    'ssh_ack',
    'ssh_host_key_decision',
    'ssh_close',
    'ssh_pick_key_file',
    'open_external_url',
    'ssh_connect_host',
    'vault_status',
    'vault_create',
    'vault_unlock',
    'vault_lock',
    'vault_change_password',
    'vault_touch',
    'vault_watch',
    'vault_set_auto_lock',
    'vault_snapshot',
    'host_save',
    'host_delete',
    'group_save',
    'group_delete',
    'snippet_save',
    'snippet_delete',
    'identity_save',
    'identity_delete',
    'key_import',
    'key_rename',
    'key_delete',
    'known_host_list',
    'known_host_delete',
    'settings_get',
    'settings_set',
    'ssh_auth_answer',
    'key_generate',
    'key_export_public',
    'key_deploy',
    'sftp_open',
    'sftp_close',
    'sftp_home',
    'sftp_list',
    'sftp_mkdir',
    'sftp_rename',
    'sftp_remove',
    'sftp_chmod',
    'sftp_download',
    'sftp_upload',
    'sftp_transfer_cancel',
    'sftp_conflict_decision',
    'sftp_edit',
    'sftp_edit_close',
    'sftp_edit_force',
    'local_pick_root',
    'local_list',
    'term_profiles',
    'term_open',
    'term_write',
    'term_resize',
    'term_ack',
    'term_close',
    'rdp_launch',
    'import_ssh_config_preview',
    'import_ssh_config_apply',
    'import_known_hosts_preview',
    'import_known_hosts_apply',
    'backup_export',
    'backup_import',
  ]);

  /**
   * Commandes internes de Tauri indispensables au fonctionnement de l'IPC.
   * - `plugin:__TAURI_CHANNEL__|fetch` : récupération des messages de `Channel` de plus de
   *   1 Kio (sortie du terminal). Tauri l'exempte d'ACL ; elle ne renvoie qu'une donnée déjà
   *   destinée à cette fenêtre, une seule fois, désignée par un identifiant aléatoire.
   */
  var INTERNAL_COMMANDS = Object.freeze(['plugin:__TAURI_CHANNEL__|fetch']);

  /** Nom volontairement absent du cœur : un appel refusé échoue avec une erreur. */
  var DENIED_COMMAND = 'younji_isolation_denied';

  function isAllowed(cmd) {
    return (
      typeof cmd === 'string' &&
      (ALLOWED_COMMANDS.indexOf(cmd) !== -1 || INTERNAL_COMMANDS.indexOf(cmd) !== -1)
    );
  }

  function hook(message) {
    if (message !== null && typeof message === 'object' && isAllowed(message.cmd)) {
      return message;
    }
    var denied = Object.create(null);
    denied.cmd = DENIED_COMMAND;
    denied.callback = message && message.callback;
    denied.error = message && message.error;
    denied.options = undefined;
    // La charge utile d'un appel refusé n'est jamais transmise au cœur.
    denied.payload = Object.create(null);
    return denied;
  }

  Object.defineProperty(root, '__TAURI_ISOLATION_HOOK__', { value: hook });
  Object.defineProperty(root, '__YOUNJI_ISOLATION__', {
    value: Object.freeze({
      allowedCommands: ALLOWED_COMMANDS,
      internalCommands: INTERNAL_COMMANDS,
      deniedCommand: DENIED_COMMAND,
    }),
  });
})(window);
