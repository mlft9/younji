//! Commandes IPC. Chaque commande est une fonction fine : elle valide l'entrée, appelle une
//! crate métier et convertit l'erreur. Toute nouvelle commande doit être déclarée dans
//! `build.rs`, accordée dans une capability et ajoutée à la liste blanche de l'isolation.

pub mod app;
pub mod book;
pub mod rdp;
pub mod settings;
pub mod sftp;
pub mod snippets;
pub mod ssh;
pub mod term;
pub mod url;
pub mod vault;
