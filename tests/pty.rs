//! Tests d'intégration PTY : un vrai programme s'exécute derrière un
//! pseudo-terminal, sa sortie est réémulée par [`Terminal`] et la grille
//! résultante est vérifiée. Les cas déterministes reposent sur `printf` ; un
//! cas `less` (application plein écran réelle) est ignoré si `less` est absent.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use bashop::term::Terminal;

/// Lance `programme` dans un PTY et renvoie tout ce qu'il écrit (EOF compris).
fn lire_pty(programme: &str, args: &[&str]) -> Option<Vec<u8>>
  {
    let systeme = native_pty_system();
    let paire = systeme
      .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
      .ok()?;
    let mut commande = CommandBuilder::new(programme);
    for argument in args
      {
        commande.arg(argument);
      }
    let mut enfant = paire.slave.spawn_command(commande).ok()?;
    drop(paire.slave);
    let mut lecteur = paire.master.try_clone_reader().ok()?;
    let mut sortie = Vec::new();
    let mut tampon = [0u8; 4096];
    loop
      {
        match lecteur.read(&mut tampon)
          {
            Ok(0) | Err(_) => break,
            Ok(n) => sortie.extend_from_slice(&tampon[..n]),
          }
      }
    let _ = enfant.wait();
    Some(sortie)
  }

/// Exécute `programme`, réémule sa sortie et rend le terminal résultant.
fn reemuler(programme: &str, args: &[&str]) -> Option<Terminal>
  {
    let octets = lire_pty(programme, args)?;
    let mut term = Terminal::nouveau(80, 24);
    let mut parser = vte::Parser::new();
    term.alimenter(&mut parser, &octets);
    Some(term)
  }

fn printf_present() -> bool
  {
    std::process::Command::new("printf").arg("").output().is_ok()
  }

fn programme_present(nom: &str) -> bool
  {
    std::process::Command::new("sh")
      .arg("-c")
      .arg(format!("command -v {nom}"))
      .output()
      .map(|r| r.status.success())
      .unwrap_or(false)
  }

fn contient(foin: &[u8], motif: &[u8]) -> bool
  {
    motif.len() <= foin.len() && foin.windows(motif.len()).any(|fenetre| fenetre == motif)
  }

#[test]
fn un_programme_plein_ecran_est_detecte()
  {
    if !printf_present()
      {
        return;
      }
    let Some(term) = reemuler("printf", &["\x1b[?1049hBONJOUR"])
      else
        {
          return;
        };
    assert!(term.en_plein_ecran(), "l'écran alterné doit être détecté");
    assert!(term.grid.texte_ligne(0).contains("BONJOUR"), "{}", term.grid.texte_ligne(0));
  }

#[test]
fn la_sortie_avec_sauts_de_ligne_est_reemulee()
  {
    if !printf_present()
      {
        return;
      }
    let Some(term) = reemuler("printf", &["un\ndeux\ntrois\n"])
      else
        {
          return;
        };
    assert!(term.grid.texte_ligne(0).starts_with("un"), "{}", term.grid.texte_ligne(0));
    assert!(term.grid.texte_ligne(1).starts_with("deux"), "{}", term.grid.texte_ligne(1));
    assert!(term.grid.texte_ligne(2).starts_with("trois"), "{}", term.grid.texte_ligne(2));
  }

#[test]
fn la_zone_de_defilement_est_respectee_a_travers_le_pty()
  {
    if !printf_present()
      {
        return;
      }
    let Some(term) = reemuler("printf", &["1\n2\n3\n\x1b[2;3r\x1b[3;1H\n"])
      else
        {
          return;
        };
    assert!(term.grid.texte_ligne(0).starts_with('1'), "hors zone intacte");
    assert!(term.grid.texte_ligne(1).starts_with('3'), "la ligne basse remonte");
    assert_eq!(term.grid.texte_ligne(2).trim(), "");
  }

#[test]
fn less_bascule_en_plein_ecran_et_quitte_proprement()
  {
    if !programme_present("less")
      {
        return;
      }
    let chemin = std::env::temp_dir().join("bashop_pty_less.txt");
    if std::fs::write(&chemin, "alpha\nbravo\ncharlie\ndelta\n").is_err()
      {
        return;
      }
    let systeme = native_pty_system();
    let Ok(paire) = systeme.openpty(PtySize { rows: 10, cols: 40, pixel_width: 0, pixel_height: 0 })
      else
        {
          return;
        };
    let mut commande = CommandBuilder::new("less");
    commande.arg(&chemin);
    let mut enfant = match paire.slave.spawn_command(commande)
      {
        Ok(enfant) => enfant,
        Err(_) => return,
      };
    drop(paire.slave);
    let Ok(mut lecteur) = paire.master.try_clone_reader()
      else
        {
          return;
        };
    let Ok(mut ecrivain) = paire.master.take_writer()
      else
        {
          return;
        };
    let tampon = Arc::new(Mutex::new(Vec::<u8>::new()));
    let (emetteur, recepteur) = mpsc::channel::<()>();
    let vue = Arc::clone(&tampon);
    thread::spawn(move || {
      let mut bloc = [0u8; 4096];
      while let Ok(n) = lecteur.read(&mut bloc)
        {
          if n == 0
            {
              break;
            }
          if let Ok(mut garde) = vue.lock()
            {
              garde.extend_from_slice(&bloc[..n]);
            }
          let _ = emetteur.send(());
        }
    });
    let mut vu_alt = false;
    let limite = std::time::Instant::now();
    while limite.elapsed() < Duration::from_secs(3)
      {
        if recepteur.recv_timeout(Duration::from_millis(200)).is_err()
          {
            break;
          }
        if let Ok(garde) = tampon.lock()
          && (contient(&garde, b"\x1b[?1049") || contient(&garde, b"\x1b[?1047") || contient(&garde, b"\x1b[?47h"))
          {
            vu_alt = true;
            break;
          }
      }
    let _ = ecrivain.write_all(b"q");
    let _ = ecrivain.flush();
    for _ in 0..20
      {
        if enfant.try_wait().ok().flatten().is_some()
          {
            break;
          }
        thread::sleep(Duration::from_millis(50));
      }
    let _ = enfant.kill();
    let _ = std::fs::remove_file(&chemin);
    assert!(vu_alt, "less doit demander l'écran alterné");
  }
