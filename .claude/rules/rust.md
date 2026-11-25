---
paths:
  - "src-tauri/**/*.rs"
  - "crates/**/*.rs"
  - "**/Cargo.toml"
---

# Règles Rust

## Structure
- La logique métier va dans `crates/core-*` : aucune dépendance à `tauri` dans ces crates.
- `src-tauri/src/commands/` : une commande = une fonction fine qui valide l'entrée, appelle une crate et convertit l'erreur. Pas de logique métier.
- L'état partagé passe par `tauri::State` avec des types dédiés (`SessionManager`, `VaultState`…). Pas de `static mut` ni de globals.
- Les flux continus (sortie du terminal, progression SFTP) passent par `tauri::ipc::Channel<T>` reçu en paramètre de commande, pas par des événements globaux.

## Erreurs
- Dans les crates : `thiserror`, avec des variantes précises. Dans `src-tauri` : conversion en `AppError { code: &'static str, … }` sérialisable, dont le `code` correspond à une clé i18n (`errors.<code>`).
- Pas de `unwrap()`, `expect()` ni `panic!` hors tests et hors invariants prouvés (avec un commentaire qui justifie).
- Un message d'erreur ne contient **jamais** de secret ni de contenu brut venant du serveur sans le nettoyer.

## Async et ressources
- `tokio` multi-thread. Ne bloque jamais l'exécuteur : I/O fichiers lourdes et crypto coûteuse (Argon2) dans `spawn_blocking`.
- Chaque session a une tâche propriétaire, annulable (`CancellationToken`). Libère les ressources à la fermeture.
- Flux bornés : canaux avec capacité, regroupement de la sortie du terminal (lots de quelques ms) pour ne pas saturer l'IPC.

## Secrets
- `secrecy::SecretString` / `SecretBox<[u8]>` pour les secrets, `zeroize::Zeroizing` pour les tampons temporaires.
- `#[derive(Debug)]` interdit sur une structure qui contient un secret, sauf avec un `Debug` manuel qui masque les champs.
- Aucune sérialisation `serde` d'un secret vers l'interface.

## Style et qualité
- `cargo fmt` ; `cargo clippy --all-targets -- -D warnings` sans `#[allow]` injustifié (justification en commentaire si besoin).
- Documentation `///` sur les éléments publics des crates.
- Tests unitaires dans le module (`#[cfg(test)]`), tests d'intégration dans `crates/*/tests/`. Les tests qui ont besoin d'un serveur SSH sont marqués `#[ignore]` et lancés par `npm run test:integration`.
- **Aucune clé privée dans le dépôt, même de test** : les tests génèrent leurs clés à l'exécution (dans un dossier temporaire). Les autres fixtures (configs OpenSSH, known_hosts d'exemple) vont dans `tests/fixtures/`, nommées `test_*`.
