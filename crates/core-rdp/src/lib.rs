//! RDP v1 de Younji (RDP-02) : lancement du client du système (`mstsc` sous Windows,
//! `xfreerdp3`/`xfreerdp` sous Linux) **sans jamais passer le mot de passe en ligne de
//! commande** (SECURITE.md §4.3). Sans dépendance à Tauri (ADR-004).
//!
//! - [`target`] : cible validée (adresse, port, utilisateur, domaine, résolution) ;
//! - [`rdpfile`] : fichier `.rdp` pour mstsc (sans mot de passe) ;
//! - [`freerdp`] : arguments et entrée standard de FreeRDP ;
//! - [`launch`] : détection du client, lancement supervisé, nettoyage.

pub mod error;
pub mod freerdp;
pub mod launch;
pub mod rdpfile;
pub mod target;
#[cfg(windows)]
mod wincred;

pub use error::RdpError;
pub use launch::{CLEANUP_DELAY, RdpClient, RdpProcess, cleanup_stale, find_client, launch};
pub use target::{RdpTarget, Resolution};
