//! Session de terminal local sur un PTY (ConPTY sous Windows, openpty sous Linux).
//!
//! Trois fils par session :
//! - **lecture** : sortie du PTY par blocs de 32 Kio au plus, envoyée au [`TermSink`]. Au-delà de
//!   [`HIGH_WATERMARK`] octets non acquittés ([`LocalSession::ack`]), la lecture s'arrête : le
//!   tampon du PTY se remplit et le programme est bloqué en écriture (même contrôle de flux que
//!   SSH, ADR-024) ;
//! - **écriture** : file bornée de saisies ;
//! - **attente** : fin du processus, puis fermeture du PTY et événement de fin, une seule fois.
//!
//! Aucun contenu du terminal n'est journalisé.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::error::TermError;
use crate::profiles::LocalProfile;

/// Colonnes maximales (comme SSH).
pub const MAX_COLS: u32 = 1000;
/// Lignes maximales (comme SSH).
pub const MAX_ROWS: u32 = 500;
/// Taille maximale d'une saisie envoyée en un appel.
pub const MAX_WRITE: usize = 64 * 1024;
/// Octets non acquittés au-delà desquels la lecture du PTY est suspendue.
pub const HIGH_WATERMARK: usize = 2 * 1024 * 1024;
/// Taille d'un bloc de sortie.
const READ_CHUNK: usize = 32 * 1024;
/// Saisies en attente d'écriture au plus.
const WRITE_QUEUE: usize = 64;
/// Délai laissé à la lecture pour vider le PTY après la fin du processus.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Taille du terminal, validée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    cols: u16,
    rows: u16,
}

impl TermSize {
    /// Valide une taille : 1–[`MAX_COLS`] colonnes, 1–[`MAX_ROWS`] lignes.
    pub fn new(cols: u32, rows: u32) -> Result<Self, TermError> {
        if (1..=MAX_COLS).contains(&cols) && (1..=MAX_ROWS).contains(&rows) {
            // Bornes vérifiées ci-dessus : la conversion ne peut pas échouer.
            let cols = u16::try_from(cols).map_err(|_| TermError::InvalidSize)?;
            let rows = u16::try_from(rows).map_err(|_| TermError::InvalidSize)?;
            Ok(Self { cols, rows })
        } else {
            Err(TermError::InvalidSize)
        }
    }

    fn pty(self) -> PtySize {
        PtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Événement de fin de session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermEvent {
    /// Le processus s'est terminé (code de sortie s'il est connu), ou la session a été fermée.
    Exited {
        /// Code de sortie.
        code: Option<u32>,
    },
}

/// Destinataire de la sortie et de la fin d'une session.
pub trait TermSink: Send + Sync + 'static {
    /// Bloc de sortie du terminal (octets bruts).
    fn output(&self, data: Vec<u8>);
    /// Fin de la session (appelé une seule fois, en dernier).
    fn event(&self, event: TermEvent);
}

/// État partagé entre la session et ses fils.
struct Shared {
    /// Octets envoyés et non encore acquittés.
    unacked: Mutex<usize>,
    acked: Condvar,
    closed: AtomicBool,
    finished: AtomicBool,
}

impl Shared {
    fn wake(&self) {
        self.acked.notify_all();
    }
}

/// Session de terminal local. La fermer (ou la détruire) tue le processus.
pub struct LocalSession {
    shared: Arc<Shared>,
    writer: Mutex<Option<SyncSender<Vec<u8>>>>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl std::fmt::Debug for LocalSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalSession").finish_non_exhaustive()
    }
}

fn spawn_failed(error: impl std::fmt::Display) -> TermError {
    tracing::warn!(%error, "lancement du terminal local impossible");
    TermError::SpawnFailed
}

impl LocalSession {
    /// Lance le profil dans un nouveau PTY. Le programme est celui du profil détecté, avec ses
    /// arguments, sans shell intermédiaire ; le dossier de départ est le dossier personnel.
    pub fn spawn(
        profile: &LocalProfile,
        size: TermSize,
        sink: Arc<dyn TermSink>,
    ) -> Result<Self, TermError> {
        let pair = native_pty_system()
            .openpty(size.pty())
            .map_err(spawn_failed)?;
        let mut command = CommandBuilder::new(profile.program());
        command.args(profile.args());
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        if let Some(home) = home_dir() {
            command.cwd(home);
        }
        let mut child = pair.slave.spawn_command(command).map_err(spawn_failed)?;
        // Sans cette fermeture, la lecture ne verrait jamais la fin du PTY sous Linux.
        drop(pair.slave);
        let killer = child.clone_killer();
        let reader = pair.master.try_clone_reader().map_err(|e| {
            let _ = child.kill();
            spawn_failed(e)
        })?;
        let writer = pair.master.take_writer().map_err(|e| {
            let _ = child.kill();
            spawn_failed(e)
        })?;

        let shared = Arc::new(Shared {
            unacked: Mutex::new(0),
            acked: Condvar::new(),
            closed: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        });
        let master = Arc::new(Mutex::new(Some(pair.master)));

        let (drained_tx, drained_rx) = mpsc::channel::<()>();
        let reader_shared = shared.clone();
        let reader_sink = sink.clone();
        std::thread::Builder::new()
            .name("younji-term-read".into())
            .spawn(move || read_loop(reader, &reader_shared, reader_sink.as_ref(), &drained_tx))
            .map_err(spawn_failed)?;

        let (write_tx, write_rx) = mpsc::sync_channel::<Vec<u8>>(WRITE_QUEUE);
        std::thread::Builder::new()
            .name("younji-term-write".into())
            .spawn(move || write_loop(writer, &write_rx))
            .map_err(spawn_failed)?;

        let wait_shared = shared.clone();
        let wait_master = master.clone();
        std::thread::Builder::new()
            .name("younji-term-wait".into())
            .spawn(move || {
                let code = child.wait().ok().map(|status| status.exit_code());
                // Fermer le PTY termine la lecture (indispensable sous Windows : ConPTY ne
                // signale la fin qu'une fois fermé), après un court délai pour vider la sortie.
                let _ = drained_rx.recv_timeout(DRAIN_TIMEOUT);
                if let Ok(mut master) = wait_master.lock() {
                    master.take();
                }
                finish(&wait_shared, sink.as_ref(), code);
            })
            .map_err(spawn_failed)?;

        tracing::debug!(kind = profile.kind.as_str(), "terminal local lancé");
        Ok(Self {
            shared,
            writer: Mutex::new(Some(write_tx)),
            master,
            killer: Mutex::new(killer),
        })
    }

    /// Envoie une saisie au programme (64 Kio au plus). Peut attendre si la file est pleine
    /// (programme qui ne lit plus) : à appeler hors de l'exécuteur async.
    pub fn write(&self, data: Vec<u8>) -> Result<(), TermError> {
        if data.len() > MAX_WRITE {
            return Err(TermError::InputTooLarge);
        }
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(TermError::Closed);
        }
        let sender = self
            .writer
            .lock()
            .map_err(|_| TermError::Closed)?
            .clone()
            .ok_or(TermError::Closed)?;
        sender.send(data).map_err(|_| TermError::Closed)
    }

    /// Répercute la taille du terminal.
    pub fn resize(&self, size: TermSize) -> Result<(), TermError> {
        let master = self.master.lock().map_err(|_| TermError::Closed)?;
        let master = master.as_ref().ok_or(TermError::Closed)?;
        master.resize(size.pty()).map_err(|error| {
            tracing::debug!(%error, "redimensionnement du terminal local refusé");
            TermError::Closed
        })
    }

    /// Acquitte des octets affichés. Un acquittement excessif ramène le compteur à zéro.
    pub fn ack(&self, bytes: usize) {
        if let Ok(mut unacked) = self.shared.unacked.lock() {
            *unacked = unacked.saturating_sub(bytes);
        }
        self.shared.wake();
    }

    /// Ferme la session : le processus est tué, l'écriture arrêtée, la lecture réveillée.
    pub fn close(&self) {
        if self.shared.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut writer) = self.writer.lock() {
            writer.take();
        }
        if let Ok(mut killer) = self.killer.lock() {
            let _ = killer.kill();
        }
        self.shared.wake();
        tracing::debug!("terminal local fermé");
    }

    /// Vrai une fois la fin de session signalée.
    pub fn is_finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }
}

impl Drop for LocalSession {
    fn drop(&mut self) {
        self.close();
    }
}

/// Signale la fin une seule fois.
fn finish(shared: &Shared, sink: &dyn TermSink, code: Option<u32>) {
    shared.closed.store(true, Ordering::Release);
    if !shared.finished.swap(true, Ordering::AcqRel) {
        sink.event(TermEvent::Exited { code });
    }
    shared.wake();
}

fn read_loop(
    mut reader: Box<dyn Read + Send>,
    shared: &Shared,
    sink: &dyn TermSink,
    drained: &mpsc::Sender<()>,
) {
    let mut buffer = vec![0u8; READ_CHUNK];
    loop {
        // Contrôle de flux : attendre l'acquittement de l'interface.
        {
            let Ok(mut unacked) = shared.unacked.lock() else {
                break;
            };
            while *unacked >= HIGH_WATERMARK && !shared.closed.load(Ordering::Acquire) {
                match shared
                    .acked
                    .wait_timeout(unacked, Duration::from_millis(500))
                {
                    Ok((guard, _)) => unacked = guard,
                    Err(_) => return,
                }
            }
        }
        if shared.finished.load(Ordering::Acquire) {
            break;
        }
        let n = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if shared.finished.load(Ordering::Acquire) {
            break;
        }
        if let Ok(mut unacked) = shared.unacked.lock() {
            *unacked = unacked.saturating_add(n);
        }
        if let Some(chunk) = buffer.get(..n) {
            sink.output(chunk.to_vec());
        }
    }
    let _ = drained.send(());
}

fn write_loop(mut writer: Box<dyn Write + Send>, queue: &Receiver<Vec<u8>>) {
    while let Ok(data) = queue.recv() {
        if writer
            .write_all(&data)
            .and_then(|()| writer.flush())
            .is_err()
        {
            break;
        }
    }
}

/// Dossier personnel (`HOME` sous Unix, `USERPROFILE` sous Windows), s'il existe.
fn home_dir() -> Option<std::path::PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_dir())
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[derive(Default)]
    struct Collect {
        output: Mutex<Vec<u8>>,
        events: Mutex<Vec<TermEvent>>,
    }

    impl TermSink for Collect {
        fn output(&self, data: Vec<u8>) {
            self.output.lock().unwrap().extend(data);
        }
        fn event(&self, event: TermEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    impl Collect {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned()
        }
        fn wait_for(&self, what: impl Fn(&Self) -> bool) -> bool {
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(10) {
                if what(self) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        }
    }

    fn size() -> TermSize {
        TermSize::new(80, 24).unwrap()
    }

    #[test]
    fn sizes_are_bounded() {
        assert!(TermSize::new(1, 1).is_ok());
        assert!(TermSize::new(MAX_COLS, MAX_ROWS).is_ok());
        for (c, r) in [(0, 24), (80, 0), (MAX_COLS + 1, 24), (80, MAX_ROWS + 1)] {
            assert_eq!(TermSize::new(c, r), Err(TermError::InvalidSize));
        }
    }

    #[test]
    fn a_shell_runs_commands_and_reports_its_exit() {
        let sink = Arc::new(Collect::default());
        let session = LocalSession::spawn(
            &LocalProfile::for_test("/bin/sh", &[]),
            size(),
            sink.clone(),
        )
        .unwrap();
        session
            .write(b"echo resultat-$((40+2))\nexit 3\n".to_vec())
            .unwrap();
        assert!(sink.wait_for(|s| !s.events.lock().unwrap().is_empty()));
        assert!(sink.text().contains("resultat-42"), "{}", sink.text());
        assert_eq!(
            *sink.events.lock().unwrap(),
            [TermEvent::Exited { code: Some(3) }]
        );
        assert!(session.is_finished());
        assert_eq!(session.write(b"x".to_vec()), Err(TermError::Closed));
    }

    #[test]
    fn the_terminal_size_reaches_the_program() {
        let sink = Arc::new(Collect::default());
        let session = LocalSession::spawn(
            &LocalProfile::for_test("/bin/sh", &[]),
            size(),
            sink.clone(),
        )
        .unwrap();
        session.resize(TermSize::new(132, 43).unwrap()).unwrap();
        session.write(b"stty size\n".to_vec()).unwrap();
        assert!(
            sink.wait_for(|s| s.text().contains("43 132")),
            "{}",
            sink.text()
        );
    }

    #[test]
    fn closing_kills_the_program_and_ends_once() {
        let sink = Arc::new(Collect::default());
        let session = LocalSession::spawn(
            &LocalProfile::for_test("/bin/sh", &["-c", "sleep 60"]),
            size(),
            sink.clone(),
        )
        .unwrap();
        session.close();
        session.close();
        assert!(sink.wait_for(|s| s.events.lock().unwrap().len() == 1));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(sink.events.lock().unwrap().len(), 1);
    }

    #[test]
    fn oversized_input_is_refused() {
        let sink = Arc::new(Collect::default());
        let session =
            LocalSession::spawn(&LocalProfile::for_test("/bin/sh", &[]), size(), sink).unwrap();
        assert_eq!(
            session.write(vec![b'a'; MAX_WRITE + 1]),
            Err(TermError::InputTooLarge)
        );
    }

    #[test]
    fn reading_stops_without_acknowledgement() {
        let sink = Arc::new(Collect::default());
        let session = LocalSession::spawn(
            &LocalProfile::for_test("/bin/sh", &["-c", "yes"]),
            size(),
            sink.clone(),
        )
        .unwrap();
        assert!(sink.wait_for(|s| s.output.lock().unwrap().len() >= HIGH_WATERMARK));
        std::thread::sleep(Duration::from_millis(300));
        let stalled = sink.output.lock().unwrap().len();
        std::thread::sleep(Duration::from_millis(300));
        // Sans acquittement, la lecture reste arrêtée (au plus un bloc de plus).
        assert!(sink.output.lock().unwrap().len() <= stalled + READ_CHUNK);
        assert!(stalled < HIGH_WATERMARK + READ_CHUNK);
        session.ack(stalled);
        assert!(sink.wait_for(|s| s.output.lock().unwrap().len() > stalled + READ_CHUNK));
        session.close();
    }

    #[test]
    fn spawning_a_missing_program_fails_cleanly() {
        let sink = Arc::new(Collect::default());
        let result = LocalSession::spawn(
            &LocalProfile::for_test("/nonexistent/shell", &[]),
            size(),
            sink,
        );
        assert_eq!(result.err(), Some(TermError::SpawnFailed));
    }
}
