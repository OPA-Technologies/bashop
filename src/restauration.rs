//! Restauration du terminal : signaux, hook de panique et garde RAII.
//!
//! Le terminal est mis en mode brut et en écran alterné ; si le processus
//! meurt par signal (ou panique), il faut rendre le terminal propre. Les
//! gestionnaires de signaux sont volontairement minimaux et *async-signal-safe*
//! (pas d'allocation, uniquement `write`, `tcsetattr`, `kill`, `_exit`).

#[cfg(unix)]
mod unix_impl
  {
    use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

    /// Séquence de restauration écrite telle quelle sur la sortie standard.
    const RESTAURATION: &[u8] =
      b"\x1b[<1u\x1b]110\x1b\\\x1b[?1049l\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l\x1b[?2004l\x1b[?25h\x1b[0m\r\n";

    static ACTIF: AtomicBool = AtomicBool::new(false);
    static PID_ENFANT: AtomicI32 = AtomicI32::new(0);
    static TERMIOS_VALIDE: AtomicBool = AtomicBool::new(false);

    /// Sauvegarde brute de termios (accédée par pointeur depuis le handler).
    static mut TERMIOS_ORIG: libc::termios = unsafe { std::mem::zeroed() };

    /// Handler appelé sur `SIGTERM`/`SIGINT`/`SIGHUP`/`SIGQUIT`.
    unsafe extern "C" fn sur_signal(signal: libc::c_int)
      {
        unsafe {
          let _ = libc::write(1, RESTAURATION.as_ptr().cast(), RESTAURATION.len());
          if TERMIOS_VALIDE.load(Ordering::SeqCst)
            {
              libc::tcsetattr(0, libc::TCSAFLUSH, std::ptr::addr_of!(TERMIOS_ORIG));
            }
          let pid = PID_ENFANT.load(Ordering::SeqCst);
          if pid > 0
            {
              libc::kill(pid, libc::SIGKILL);
            }
          libc::_exit(128 + signal);
        }
      }

    /// Installe les gestionnaires et sauvegarde termios (avant le mode brut).
    pub fn installer()
      {
        unsafe {
          let mut terme: libc::termios = std::mem::zeroed();
          if libc::tcgetattr(0, &mut terme) == 0
            {
              std::ptr::write(std::ptr::addr_of_mut!(TERMIOS_ORIG), terme);
              TERMIOS_VALIDE.store(true, Ordering::SeqCst);
            }
          for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGQUIT]
            {
              libc::signal(signal, sur_signal as *const () as libc::sighandler_t);
            }
        }
        ACTIF.store(true, Ordering::SeqCst);
      }

    /// Mémorise le pid de l'enfant à tuer en cas de signal.
    pub fn surveiller_enfant(pid: u32)
      {
        PID_ENFANT.store(pid as i32, Ordering::SeqCst);
      }

    /// Désactive la surveillance (sortie propre).
    pub fn desactiver()
      {
        ACTIF.store(false, Ordering::SeqCst);
        PID_ENFANT.store(0, Ordering::SeqCst);
      }

    /// Indique si les gestionnaires sont actifs.
    pub fn active() -> bool
      {
        ACTIF.load(Ordering::SeqCst)
      }
  }

#[cfg(not(unix))]
mod autre_impl
  {
    /// Aucun signal à gérer hors Unix (le terminal est restitué par l'API).
    pub fn installer() {}

    /// Sans objet hors Unix.
    pub fn surveiller_enfant(_pid: u32) {}

    /// Sans objet hors Unix.
    pub fn desactiver() {}

    /// Sans objet hors Unix.
    pub fn active() -> bool
      {
        false
      }
  }

#[cfg(unix)]
pub use unix_impl::{active, desactiver, installer, surveiller_enfant};

#[cfg(not(unix))]
pub use autre_impl::{active, desactiver, installer, surveiller_enfant};

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn la_surveillance_peut_etre_activee_et_desactivee()
      {
        installer();
        surveiller_enfant(std::process::id());
        assert!(active());
        desactiver();
        assert!(!active());
      }
  }
