//! État des terminaux locaux (étape 7) : profils détectés et sessions ouvertes.
//!
//! Les profils lançables sont ceux de la **dernière détection faite par le cœur**
//! (`term_profiles`) : l'interface ne désigne qu'un identifiant, jamais un programme.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use core_term::{LocalProfile, LocalSession};

use crate::error::AppError;

/// Nombre maximal de terminaux locaux simultanés.
pub const MAX_LOCAL_SESSIONS: usize = 32;

/// Identifiant opaque d'un terminal local.
pub type TermId = u64;

type Sessions = Arc<Mutex<HashMap<TermId, Arc<LocalSession>>>>;

/// Profils détectés et terminaux locaux ouverts.
#[derive(Default)]
pub struct LocalTerminals {
    profiles: Mutex<Vec<LocalProfile>>,
    sessions: Sessions,
    next_id: AtomicU64,
}

impl LocalTerminals {
    fn lock_sessions(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<TermId, Arc<LocalSession>>>, AppError> {
        self.sessions.lock().map_err(|_| AppError::INTERNAL)
    }

    /// Remplace les profils connus par ceux d'une nouvelle détection.
    pub fn set_profiles(&self, profiles: Vec<LocalProfile>) -> Result<(), AppError> {
        *self.profiles.lock().map_err(|_| AppError::INTERNAL)? = profiles;
        Ok(())
    }

    /// Profil de la dernière détection portant cet identifiant.
    pub fn profile(&self, id: &str) -> Result<LocalProfile, AppError> {
        self.profiles
            .lock()
            .map_err(|_| AppError::INTERNAL)?
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| core_term::TermError::UnknownProfile.into())
    }

    /// Enregistre un terminal. `start` reçoit l'identifiant attribué et de quoi retirer le
    /// terminal à sa fin ; le verrou est tenu pendant le lancement (même raison que pour SSH).
    pub fn insert(
        &self,
        start: impl FnOnce(TermId, TermRemover) -> Result<LocalSession, AppError>,
    ) -> Result<TermId, AppError> {
        let mut sessions = self.lock_sessions()?;
        if sessions.len() >= MAX_LOCAL_SESSIONS {
            return Err(AppError::TOO_MANY_SESSIONS);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let remover = TermRemover {
            sessions: self.sessions.clone(),
            id,
        };
        sessions.insert(id, Arc::new(start(id, remover)?));
        Ok(id)
    }

    /// Terminal ouvert correspondant à l'identifiant.
    pub fn session(&self, id: TermId) -> Result<Arc<LocalSession>, AppError> {
        self.lock_sessions()?
            .get(&id)
            .cloned()
            .ok_or(AppError::SESSION_NOT_FOUND)
    }

    /// Ferme (processus tué) et oublie un terminal.
    pub fn close(&self, id: TermId) -> Result<(), AppError> {
        let session = self
            .lock_sessions()?
            .remove(&id)
            .ok_or(AppError::SESSION_NOT_FOUND)?;
        session.close();
        Ok(())
    }

    /// Ferme tous les terminaux (verrouillage, rechargement de l'interface).
    pub fn close_all(&self) {
        let sessions: Vec<_> = match self.sessions.lock() {
            Ok(mut sessions) => sessions.drain().map(|(_, s)| s).collect(),
            Err(_) => return,
        };
        for session in sessions {
            session.close();
        }
    }
}

/// Retire un terminal du registre quand son processus se termine de lui-même.
pub struct TermRemover {
    sessions: Sessions,
    id: TermId,
}

impl TermRemover {
    /// Retire le terminal (sans effet s'il a déjà été fermé).
    pub fn remove(&self) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_profiles_and_sessions_are_refused() {
        let terminals = LocalTerminals::default();
        assert_eq!(
            terminals.profile("cmd").unwrap_err(),
            AppError::new("term.unknown_profile")
        );
        assert_eq!(
            terminals.session(1).unwrap_err(),
            AppError::SESSION_NOT_FOUND
        );
        assert_eq!(terminals.close(1), Err(AppError::SESSION_NOT_FOUND));
    }
}
