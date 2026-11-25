//! Import OpenSSH de Younji (IMP-01, IMP-02), sans dépendance à Tauri (ADR-004).
//!
//! - [`ssh_config`] : lecture de `~/.ssh/config` (avec `Include` et jokers), calcul de la
//!   configuration effective de chaque alias comme OpenSSH, rapport des entrées ignorées.
//! - [`known_hosts`] : fusion d'un `known_hosts` avec les clés d'hôte du coffre, sans jamais
//!   ajouter une autre clé à un hôte déjà connu.
//!
//! Le contenu importé est une donnée non fiable : textes nettoyés et bornés avant d'être
//! renvoyés, valeurs revalidées par l'appelant comme une saisie.

pub mod known_hosts;
pub mod ssh_config;
mod text;

pub use known_hosts::{KnownHostsMerge, KnownHostsSkip, merge_known_hosts};
pub use ssh_config::{
    ConfigReport, HostWarning, ImportedHost, SkipReason, Skipped, parse_ssh_config,
};
