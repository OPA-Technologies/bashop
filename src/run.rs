//! Exécution interactive : hôte PTY, boucle d'évènements et rendu du cadre.
//!
//! Le shell tourne dans un pseudo-terminal et conserve son édition native ; on
//! se contente d'observer le flux VT, de dessiner les trois lignes du cadre et
//! d'intercepter quelques touches au prompt.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use crossterm::event::{
  self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event,
  KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::Terminal as RatTerminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::config::Config;
use crate::detect::Privilege;
use crate::frame::{context, stat};
use crate::observe::EtatShell;
use crate::providers::{self, MesureSysteme};
use crate::restauration;
use crate::safety;
use crate::shells::Shell;
use crate::term::{SourisMode, Terminal};
use crate::term::grid::{Cellule, Couleur};
use crate::term::overlay::{Nature, Overlay};
use crate::theme::{self, Theme};

/// Fichier de démarrage généré, à nettoyer en sortie.
enum RcPrepare
  {
    /// Fichier `--rcfile` (bash).
    Rcfile(PathBuf),
    /// Répertoire `ZDOTDIR` (zsh).
    Zdotdir(PathBuf),
  }

/// Lance un shell à l'intérieur du cadre permanent.
pub fn executer(shell_demande: Option<String>, config: &Config) -> Result<()>
  {
    if crate::detect::deja_actif() || crate::detect::ancetre_bashop()
      {
        bail!("Bashop est déjà actif : cadre imbriqué refusé (utilisez BASHOP_DISABLE=1).");
      }
    let shell = shell_demande
      .as_deref()
      .and_then(Shell::par_nom)
      .unwrap_or_else(Shell::depuis_environnement);
    let theme = theme::resoudre(&config.theme, crate::detect::privilege());

    let (largeur, hauteur) = crossterm::terminal::size().unwrap_or((100, 30));
    let lignes_shell = hauteur.saturating_sub(2).max(1);
    let colonnes = largeur.max(1);

    let pty_system = native_pty_system();
    let paire = pty_system.openpty(PtySize {
      rows: lignes_shell,
      cols: colonnes,
      pixel_width: 0,
      pixel_height: 0,
    })?;

    let rc = preparer_rc(shell, &theme, config.suffix_width);
    let mut commande = CommandBuilder::new(shell.programme());
    commande.env("TERM", "xterm-256color");
    commande.env("BASHOP_ACTIVE", "1");
    match &rc
      {
        Some(RcPrepare::Rcfile(chemin)) => {
          commande.arg("--rcfile");
          commande.arg(chemin);
          commande.arg("-i");
        }
        Some(RcPrepare::Zdotdir(dossier)) => {
          commande.env("ZDOTDIR", dossier);
        }
        None => {}
      }

    let enfant = paire.slave.spawn_command(commande)?;
    if let Some(pid) = enfant.process_id()
      {
        restauration::surveiller_enfant(pid);
      }
    drop(paire.slave);
    let mut lecteur = paire.master.try_clone_reader()?;
    let ecrivain = paire.master.take_writer()?;

    let (emetteur, recepteur) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
      let mut tampon = [0u8; 8192];
      loop
        {
          match lecteur.read(&mut tampon)
            {
              Ok(0) | Err(_) => break,
              Ok(n) => {
                if emetteur.send(tampon[..n].to_vec()).is_err()
                  {
                    break;
                  }
              }
            }
        }
    });

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
      let _ = disable_raw_mode();
      let _ = write!(io::stdout(), "{}", reinitialiser_fond());
      let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
      );
      hook(info);
    }));

    restauration::installer();
    enable_raw_mode()?;
    let mut sortie = io::stdout();
    execute!(sortie, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)?;
    write!(sortie, "{}", definir_fond())?;
    sortie.flush()?;
    let backend = CrosstermBackend::new(sortie);
    let mut terminal = RatTerminal::new(backend)?;

    let mut app = App {
      parser: vte::Parser::new(),
      term: Terminal::nouveau(colonnes, lignes_shell),
      config: config.clone(),
      theme,
      largeur,
      hauteur,
      recepteur,
      ecrivain,
      master: paire.master,
      enfant,
      overlay: None,
      suggestion: None,
      ghost: None,
      decalage: 0,
      overlay_decalage: 0,
      options_help: Arc::new(Mutex::new(HashMap::new())),
      options_help_en_cours: Arc::new(Mutex::new(HashSet::new())),
      a_executer: None,
      a_executer_simule: None,
      historique: charger_historique(),
      systeme: MesureSysteme::nouveau(),
      charge: providers::charge(),
      derniere_maj: Instant::now(),
      dernier_flux: Instant::now(),
      demarrage_commande: None,
      instant_commande: None,
      notification: None,
      quitter: false,
    };

    let resultat = app.boucle(&mut terminal);

    restauration::desactiver();
    disable_raw_mode()?;
    let arriere = terminal.backend_mut();
    let _ = write!(arriere, "{}", reinitialiser_fond());
    execute!(arriere, LeaveAlternateScreen, DisableMouseCapture, DisableBracketedPaste)?;
    arriere.flush()?;
    terminal.show_cursor()?;
    nettoyer_rc(rc);
    resultat
  }

/// Séquence OSC 11 : fixe la couleur de fond par défaut du terminal (cadre).
fn definir_fond() -> String
  {
    let (r, g, b) = crate::frame::FOND_CADRE;
    format!("\x1b]11;rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}\x1b\\")
  }

/// Séquence OSC 110 : rétablit la couleur de fond par défaut du terminal.
fn reinitialiser_fond() -> &'static str
  {
    "\x1b]110\x1b\\"
  }

/// État complet de la session interactive.
struct App
  {
    parser: vte::Parser,
    term: Terminal,
    config: Config,
    theme: Theme,
    largeur: u16,
    hauteur: u16,
    recepteur: mpsc::Receiver<Vec<u8>>,
    ecrivain: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    enfant: Box<dyn Child + Send + Sync>,
    overlay: Option<Overlay>,
    suggestion: Option<String>,
    ghost: Option<String>,
    decalage: usize,
    overlay_decalage: usize,
    options_help: Arc<Mutex<HashMap<String, Vec<String>>>>,
    options_help_en_cours: Arc<Mutex<HashSet<String>>>,
    a_executer: Option<Vec<u8>>,
    a_executer_simule: Option<Vec<u8>>,
    historique: Vec<String>,
    systeme: MesureSysteme,
    charge: String,
    derniere_maj: Instant,
    dernier_flux: Instant,
    demarrage_commande: Option<Instant>,
    instant_commande: Option<String>,
    notification: Option<(String, Instant)>,
    quitter: bool,
  }

impl App
  {
    fn boucle(&mut self, terminal: &mut RatTerminal<CrosstermBackend<io::Stdout>>) -> Result<()>
      {
        let mut sale = true;
        loop
          {
            if self.derniere_maj.elapsed() >= Duration::from_millis(500)
              {
                self.systeme.maj();
                self.charge = providers::charge();
                self.derniere_maj = Instant::now();
                sale = true;
              }
            loop
              {
                match self.recepteur.try_recv()
                  {
                    Ok(octets) => {
                      self.term.alimenter(&mut self.parser, &octets);
                      self.dernier_flux = Instant::now();
                      self.rafraichir_privilege();
                      self.sur_fin_commande();
                      sale = true;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                      self.quitter = true;
                      break;
                    }
                  }
              }
            if self.enfant.try_wait()?.is_some() || self.quitter
              {
                break;
              }
            if event::poll(Duration::from_millis(30))?
              {
                match event::read()?
                  {
                    Event::Key(touche) if touche.kind != KeyEventKind::Release => {
                      self.touche(touche)?;
                      sale = true;
                    }
                    Event::Resize(largeur, hauteur) => {
                      self.redimensionner(largeur, hauteur)?;
                      sale = true;
                    }
                    Event::Paste(texte) => {
                      self.coller(&texte)?;
                      sale = true;
                    }
                    Event::Mouse(souris) => {
                      self.souris(&souris)?;
                      sale = true;
                    }
                    _ => {}
                  }
              }
            if sale
              {
                terminal.draw(|cadre| self.dessiner(cadre))?;
                sale = false;
              }
          }
        Ok(())
      }

    fn redimensionner(&mut self, largeur: u16, hauteur: u16) -> Result<()>
      {
        self.largeur = largeur.max(1);
        self.hauteur = hauteur.max(3);
        let lignes_shell = self.hauteur.saturating_sub(2).max(1);
        self.term.redimensionner(self.largeur, lignes_shell);
        self.master.resize(PtySize {
          rows: lignes_shell,
          cols: self.largeur,
          pixel_width: 0,
          pixel_height: 0,
        })?;
        Ok(())
      }

    fn touche(&mut self, touche: KeyEvent) -> Result<()>
      {
        if self.overlay.is_some()
          {
            return self.touche_overlay(touche);
          }
        let pas = self.hauteur.saturating_sub(3).max(1) as i32;
        match touche.code
          {
            KeyCode::PageUp => {
              self.defiler(pas);
              return Ok(());
            }
            KeyCode::PageDown => {
              self.defiler(-pas);
              return Ok(());
            }
            _ => {}
          }
        let au_prompt = self.etat_effectif();
        if au_prompt
          {
            match touche.code
              {
                KeyCode::F(1) if touche.modifiers.contains(KeyModifiers::CONTROL) => {
                  self.term.dernier_statut = None;
                  self.completer();
                  return Ok(());
                }
                KeyCode::F(1) => {
                  self.term.dernier_statut = None;
                  self.aide_contextuelle();
                  return Ok(());
                }
                KeyCode::F(10) => {
                  self.term.dernier_statut = None;
                  self.aide_bashop();
                  return Ok(());
                }
                KeyCode::F(2) => {
                  self.config.capture = !self.config.capture;
                  return Ok(());
                }
                KeyCode::Char('c') if touche.modifiers.contains(KeyModifiers::CONTROL) => {
                  self.suggestion = None;
                  self.ghost = None;
                }
                KeyCode::Enter => return self.valider(),
                _ => {}
              }
          }
        let octets = encoder_touche(&touche);
        if !octets.is_empty()
          {
            self.suggestion = None;
            self.ghost = None;
            self.ecrivain.write_all(&octets)?;
            self.ecrivain.flush()?;
          }
        Ok(())
      }

    fn touche_overlay(&mut self, touche: KeyEvent) -> Result<()>
      {
        if let Some(overlay) = &self.overlay
          && overlay.nature == Nature::Confirmation
          {
            match touche.code
              {
                KeyCode::Char('o') | KeyCode::Char('y') | KeyCode::Enter => {
                  if let Some(octets) = self.a_executer.take()
                    {
                      self.ecrivain.write_all(&octets)?;
                      self.ecrivain.flush()?;
                      self.term.alimenter(&mut self.parser, b"\x1b]133;C\x1b\\");
                      self.term.etat = EtatShell::Commande;
                      self.debuter_commande();
                    }
                }
                KeyCode::Char('d') if self.a_executer_simule.is_some() => {
                  if let Some(octets) = self.a_executer_simule.take()
                    {
                      self.ecrivain.write_all(&octets)?;
                      self.ecrivain.flush()?;
                      self.term.alimenter(&mut self.parser, b"\x1b]133;C\x1b\\");
                      self.term.etat = EtatShell::Commande;
                      self.debuter_commande();
                    }
                }
                _ => {
                  self.a_executer = None;
                }
              }
            self.a_executer = None;
            self.a_executer_simule = None;
            self.overlay = None;
            return Ok(());
          }
        if let Some(overlay) = &self.overlay
          && overlay.nature == Nature::Information
          && touche.code == KeyCode::F(10)
          {
            self.term.dernier_statut = None;
            self.aide_bashop();
            return Ok(());
          }
        if let Some(overlay) = &self.overlay
          && overlay.nature == Nature::Information
          && touche.code == KeyCode::F(2)
          {
            self.config.capture = !self.config.capture;
            self.term.dernier_statut = None;
            self.aide_bashop();
            return Ok(());
          }
        if self.overlay_decale_par(touche.code)
          {
            return Ok(());
          }
        if touche.code == KeyCode::Esc
          {
            self.overlay_decalage = 0;
            self.overlay = None;
            self.a_executer = None;
            self.a_executer_simule = None;
            return Ok(());
          }
        self.overlay_decalage = 0;
        self.overlay = None;
        self.a_executer = None;
        self.a_executer_simule = None;
        Ok(())
      }

    /// Fait défiler la surcouche d'information si la touche l'exige.
    /// Renvoie `true` si la touche a été consommée (défilement possible) plutôt
    /// que d'avoir fermé la fenêtre.
    fn overlay_decale_par(&mut self, code: KeyCode) -> bool
      {
        let Some(overlay) = &self.overlay
          else
            {
              return false;
            };
        let max = self.overlay_decalage_max(overlay);
        if max == 0
          {
            return false;
          }
        let page = (self.hauteur.saturating_sub(6).max(2) as usize).saturating_sub(2);
        let pas = match code
          {
            KeyCode::Up => 1,
            KeyCode::Down => 1,
            KeyCode::PageUp => page.max(1),
            KeyCode::PageDown => page.max(1),
            _ => 0,
          };
        if pas == 0
          {
            return false;
          }
        match code
          {
            KeyCode::Up | KeyCode::PageUp => {
              self.overlay_decalage = self.overlay_decalage.saturating_sub(pas);
              true
            }
            KeyCode::Down | KeyCode::PageDown => {
              self.overlay_decalage = (self.overlay_decalage + pas).min(max);
              true
            }
            KeyCode::Home => {
              self.overlay_decalage = 0;
              true
            }
            KeyCode::End => {
              self.overlay_decalage = max;
              true
            }
            _ => false,
          }
      }

    /// Nombre maximal de lignes défila bles dans la surcouche courante.
    fn overlay_decalage_max(&self, overlay: &Overlay) -> usize
      {
        let contenu = overlay.lignes.len() + 2;
        contenu.saturating_sub(self.hauteur.saturating_sub(6).max(2) as usize)
      }

    /// Alerte affichée au centre quand la ligne en cours de saisie (ou
    /// rappelée de l'historique) présente un risque, ou qu'une simulation est
    /// disponible. `None` si rien à signaler.
    fn alerte_courante(&self) -> Option<String>
      {
        if !self.etat_effectif()
          {
            return None;
          }
        let commande = self.term.commande_courante();
        if commande.trim().is_empty()
          {
            return None;
          }
        let verdict = safety::evaluer(&commande);
        let mut fragments: Vec<String> =
          verdict.motifs.iter().take(2).map(|motif| format!("⚠ {motif}")).collect();
        if let Some(simulation) = safety::simulation(&commande)
          {
            fragments.push(format!("💡 {simulation}"));
          }
        if fragments.is_empty()
          {
            return None;
          }
        Some(fragments.join("   "))
      }

    fn valider(&mut self) -> Result<()>
      {
        let commande = self.term.commande_courante();
        let verdict = safety::evaluer(&commande);
        let simulation = safety::simulation(&commande);
        let confirmer = verdict.niveau == safety::Niveau::Danger
          || (verdict.niveau == safety::Niveau::Avertissement && simulation.is_some());
        if confirmer
          {
            let mut lignes = vec![format!("Commande : {}", commande), String::new()];
            for motif in &verdict.motifs
              {
                lignes.push(format!("• {motif}"));
              }
            if let Some(simulation) = &simulation
              {
                lignes.push(String::new());
                lignes.push(format!("💡 Simulation possible : {simulation}"));
              }
            self.a_executer = Some(b"\r".to_vec());
            self.a_executer_simule =
              simulation.map(|s| format!("\x01\x0b{s}\r").into_bytes());
            self.overlay_decalage = 0;
            let mut surcouche = Overlay::confirmation("Confirmation requise", lignes);
            if self.a_executer_simule.is_some()
              {
                surcouche.aide = "[o] exécuter   [d] simulation   [n] annuler".to_string();
              }
            self.overlay = Some(surcouche);
            return Ok(());
          }
        self.executer_ligne()
      }

    fn executer_ligne(&mut self) -> Result<()>
      {
        self.ecrivain.write_all(b"\r")?;
        self.ecrivain.flush()?;
        self.term.alimenter(&mut self.parser, b"\x1b]133;C\x1b\\");
        self.term.etat = EtatShell::Commande;
        self.debuter_commande();
        self.suggestion = None;
        self.ghost = None;
        self.decalage = 0;
        self.overlay = None;
        Ok(())
      }

    /// Marque le début d'une commande : mémorise l'instant de lancement (pour
    /// la notification de fin) et l'heure (horodatage optionnel).
    fn debuter_commande(&mut self)
      {
        self.demarrage_commande = Some(Instant::now());
        if self.config.horodatage
          {
            self.instant_commande = Some(format!("🕐 {}", providers::heure()));
          }
        self.notification = None;
      }

    /// Détecte le retour au prompt (OSC 133;A) après une commande : si elle a
    /// dépassé le seuil, publie une notification transitoire (`term_context`).
    fn sur_fin_commande(&mut self)
      {
        let Some(demarrage) = self.demarrage_commande.take()
          else
            {
              return;
            };
        if self.term.etat != EtatShell::Prompt
          {
            self.demarrage_commande = Some(demarrage);
            return;
          }
        let duree = demarrage.elapsed();
        if self.config.notification && duree >= Duration::from_secs(self.config.seuil_notification)
          {
            let texte = format!("⏱ terminée en {}", format_duree(duree));
            self.notification = Some((texte, Instant::now()));
          }
      }

    /// Fait défiler la vue de `delta` lignes (<0 vers le haut, >0 vers le bas).
    fn defiler(&mut self, delta: i32)
      {
        let max = self.term.grid.arriere_plan_len() as i32;
        self.decalage = (self.decalage as i32 + delta).clamp(0, max) as usize;
      }

    /// État effectif du shell, avec repli « idle » pour les shells sans OSC 133.
    ///
    /// Si aucun octet n'est arrivé depuis un court délai, on considère que le
    /// shell attend une saisie (repli documenté SPEC §2).
    fn etat_effectif(&self) -> bool
      {
        match self.term.etat
          {
            EtatShell::Prompt => true,
            EtatShell::Inconnu | EtatShell::Commande => {
              self.dernier_flux.elapsed() >= Duration::from_millis(250)
            }
          }
      }

    /// Recalcule le thème si le privilège publié par le shell a changé.
    fn rafraichir_privilege(&mut self)
      {
        if let Some(privilege) = self.term.privilege
          {
            let theme = theme::resoudre(&self.config.theme, privilege);
            if theme != self.theme
              {
                self.theme = theme;
              }
          }
      }

    /// Identité affichée : celle publiée par le shell (reprise root), sinon la
    /// détection locale.
    fn identite_courante(&self) -> String
      {
        match &self.term.utilisateur
          {
            Some(utilisateur) if !utilisateur.is_empty() => {
              format!("{utilisateur}@{}", providers::hote())
            }
            _ => providers::identite(),
          }
      }

    /// Traite un collage : confirmation si multi-lignes, sinon transmission.
    fn coller(&mut self, texte: &str) -> Result<()>
      {
        if texte.lines().count() > 1
          {
            let mut lignes = vec![
              format!("Collage de {} lignes :", texte.lines().count()),
              String::new(),
            ];
            for ligne in texte.lines().take(6)
              {
                lignes.push(format!("  {ligne}"));
              }
            lignes.push(String::new());
            lignes.push("Transmettre le collage tel quel ?".to_string());
            self.a_executer = Some(self.encoder_collage(texte));
            self.overlay_decalage = 0;
            self.overlay = Some(Overlay::confirmation("Confirmation de collage", lignes));
            return Ok(());
          }
        let octets = self.encoder_collage(texte);
        self.ecrivain.write_all(&octets)?;
        self.ecrivain.flush()?;
        Ok(())
      }

    /// Encadre un collage si le programme enfant a activé le bracketed paste.
    fn encoder_collage(&self, texte: &str) -> Vec<u8>
      {
        if self.term.paste_bracketed
          {
            let mut octets = Vec::with_capacity(texte.len() + 12);
            octets.extend_from_slice(b"\x1b[200~");
            octets.extend_from_slice(texte.as_bytes());
            octets.extend_from_slice(b"\x1b[201~");
            octets
          }
        else
          {
            texte.as_bytes().to_vec()
          }
      }

    /// Traite un évènement souris : transmis si l'enfant l'a demandé, sinon
    /// utilisé pour défiler l'historique.
    fn souris(&mut self, ev: &MouseEvent) -> Result<()>
      {
        if self.term.suivi_souris()
          && let Some(octets) = encoder_souris(&self.term, ev)
          {
            self.ecrivain.write_all(&octets)?;
            self.ecrivain.flush()?;
            return Ok(());
          }
        match ev.kind
          {
            MouseEventKind::ScrollUp => self.defiler(3),
            MouseEventKind::ScrollDown => self.defiler(-3),
            _ => {}
          }
        Ok(())
      }

    /// Indique si le cadre doit signaler un état root (thème ou déclencheur).
    fn root_actif(&self) -> bool
      {
        if matches!(self.term.privilege, Some(Privilege::Root)) || crate::detect::est_root()
          {
            return true;
          }
        let commande = self.term.commande_courante();
        crate::lexer::contient_declencheur(&commande, &self.config.triggers)
      }

    fn completer(&mut self)
      {
        let commande = self.term.commande_courante();
        if commande.trim().is_empty()
          {
            self.suggestion = Some("tapez d'abord une commande".to_string());
            self.ghost = None;
            return;
          }
        let jeton = commande.split_whitespace().last().unwrap_or("");
        let prefixe_jeton = &commande[commande.len() - jeton.len()..];

        let mut candidats: Vec<String> = self.options_courantes();
        let mut manque_help = false;
        if let Some(programme) = commande.split_whitespace().next()
          && !programme.is_empty()
          {
            match self.options_help(programme)
              {
                Some(help) => {
                  for option in help
                    {
                      if !candidats.iter().any(|c| c == &option)
                        {
                          candidats.push(option);
                        }
                    }
                }
                None => {
                  self.charger_options_help(programme);
                  manque_help = true;
                }
              }
          }

        if let Some(complet) = candidats
          .iter()
          .find(|c| c.starts_with(prefixe_jeton) && c.as_str() != jeton)
          .cloned()
          {
            self.suggestion = Some(complet.clone());
            self.ghost = Some(complet[prefixe_jeton.len()..].to_string());
            return;
          }

        match self.meilleure_suggestion(&commande)
          {
            Some(suggestion) => {
              let reste = if suggestion.starts_with(&commande)
                {
                  suggestion[commande.len()..].to_string()
                }
                else
                  {
                    String::new()
                  };
              self.ghost = Some(reste);
              self.suggestion = Some(suggestion);
            }
            None => {
              let message = if manque_help
                {
                  "aucune option connue (aide en cours de chargement)".to_string()
                }
                else
                  {
                    "aucune correspondance dans l'historique".to_string()
                  };
              self.suggestion = Some(message);
              self.ghost = None;
            }
          }
      }

    /// Options déjà extraites de `programme --help`, sans lancer de processus.
    fn options_help(&self, programme: &str) -> Option<Vec<String>>
      {
        self.options_help.lock().ok().and_then(|cache| cache.get(programme).cloned())
      }

    /// Lance en arrière-plan l'extraction des options de `programme --help`
    /// (une seule fois par programme), sans jamais bloquer la boucle.
    fn charger_options_help(&self, programme: &str)
      {
        let programme = programme.to_string();
        let deja_en_cours = {
          let Ok(mut en_cours) = self.options_help_en_cours.lock()
            else
              {
                return;
              };
          !en_cours.insert(programme.clone())
        };
        if deja_en_cours
          {
            return;
          }
        let cache = self.options_help.clone();
        let en_cours = self.options_help_en_cours.clone();
        thread::spawn(move || {
          let options = options_du_help(&programme);
          if let Ok(mut cache) = cache.lock()
            {
              cache.insert(programme.clone(), options);
            }
          if let Ok(mut en_cours) = en_cours.lock()
            {
              en_cours.remove(&programme);
            }
        });
      }

    /// Suggestion fantôme continue pendant la saisie (autosuggestion) :
    /// options de `--help` quand le jeton courant est une option, sinon
    /// historique. Renvoie `(reste à afficher, suggestion complète)`.
    fn fantome_live(&self) -> Option<(String, String)>
      {
        if !self.etat_effectif()
          {
            return None;
          }
        let commande = self.term.commande_courante();
        if commande.trim().is_empty() || commande.ends_with(' ')
          {
            return None;
          }
        let jeton = commande.split_whitespace().last().unwrap_or("");
        let prefixe_jeton = &commande[commande.len() - jeton.len()..];
        let programme = commande.split_whitespace().next();

        let mut candidats: Vec<String> = self.options_courantes();
        if let Some(programme) = programme
          && let Some(help) = self.options_help(programme)
          {
            for option in help
              {
                if !candidats.iter().any(|c| c == &option)
                  {
                    candidats.push(option);
                  }
              }
          }

        if let Some(complet) = candidats
          .iter()
          .find(|c| c.starts_with(prefixe_jeton) && c.as_str() != jeton)
          .cloned()
          {
            return Some((complet[prefixe_jeton.len()..].to_string(), complet));
          }
        if let Some(suggestion) = self.meilleure_suggestion(&commande)
          && suggestion.starts_with(&commande)
          {
            return Some((suggestion[commande.len()..].to_string(), suggestion));
          }
        None
      }

    fn meilleure_suggestion(&self, commande: &str) -> Option<String>
      {
        if commande.trim().is_empty()
          {
            return None;
          }
        self.historique
          .iter()
          .rev()
          .find(|ligne| ligne.trim() != commande.trim() && ligne.trim().starts_with(commande.trim()))
          .map(|ligne| ligne.trim().to_string())
      }

    fn options_courantes(&self) -> Vec<String>
      {
        let commande = self.term.commande_courante();
        let mut jetons = commande.split_whitespace();
        let Some(programme) = jetons.next()
          else
            {
              return Vec::new();
            };
        let Some(debut) = jetons.next()
          else
            {
              return Vec::new();
            };
        if debut.is_empty()
          {
            return Vec::new();
          }
        let mut vues: Vec<String> = Vec::new();
        for ligne in self.historique.iter().rev()
          {
            let mut mots = ligne.split_whitespace();
            if mots.next() == Some(programme)
              && let Some(argument) = mots.next()
              && argument.starts_with(debut)
              && argument != debut
              && !vues.iter().any(|v| v == argument)
              {
                vues.push(argument.to_string());
              }
            if vues.len() >= 5
              {
                break;
              }
          }
        vues
      }

    fn aide_contextuelle(&mut self)
      {
        let commande = self.term.commande_courante();
        let mot = commande.split_whitespace().next().unwrap_or("").to_string();
        if mot.is_empty()
          {
            self.overlay_decalage = 0;
            self.overlay = Some(Overlay::information(
              "Aide",
              vec!["Saisissez une commande puis appuyez sur F1.".to_string()],
            ));
            return;
          }
        let lignes = match aide_commande(&mot)
          {
            Some(lignes) => lignes,
            None => vec![format!("Aucune aide disponible pour « {mot} ».")],
          };
        self.overlay_decalage = 0;
        self.overlay = Some(Overlay::information(format!("Aide : {mot}"), lignes));
      }

    fn aide_bashop(&mut self)
      {
        let lignes = vec![
          format!(
            "Bashop {} ({}) — cadre terminal permanent (3 lignes)",
            crate::VERSION,
            crate::STATUT
          ),
          String::new(),
          "F1        aide de la commande en cours (--help)".to_string(),
          "Ctrl+F1   suggestion issue de l'historique".to_string(),
          "F10       cette aide".to_string(),
          "F2        bascule le mode capture (masquage)".to_string(),
          String::new(),
          format!("thème   : {}", self.theme.nom),
          format!("capture : {}", if self.config.capture { "on" } else { "off" }),
          format!("root    : {}", if self.root_actif() { "oui" } else { "non" }),
        ];
        self.overlay_decalage = 0;
        let mut surcouche = Overlay::information("Aide de Bashop", lignes);
        surcouche.aide = "F2 capture · F10 rafraîchit · Échap ferme".to_string();
        self.overlay = Some(surcouche);
      }

    fn dessiner(&self, cadre: &mut ratatui::Frame)
      {
        let (ghost, suggestion) = match &self.ghost
          {
            Some(ghost) => (Some(ghost.clone()), self.suggestion.clone()),
            None => match self.fantome_live()
              {
                Some((reste, complet)) => (Some(reste), Some(complet)),
                None => (None, self.suggestion.clone()),
              },
          };
        let notification = match &self.notification
          {
            Some((texte, moment)) if moment.elapsed() < Duration::from_secs(6) => Some(texte.clone()),
            _ => None,
          };
        let vue = Vue {
          term: &self.term,
          config: &self.config,
          root: self.root_actif(),
          identite: self.identite_courante(),
          charge: self.charge.clone(),
          options: self.options_courantes(),
          suggestion,
          ghost: ghost.unwrap_or_default().chars().collect(),
          decalage: self.decalage,
          overlay_decalage: self.overlay_decalage,
          systeme: self.systeme.texte.clone(),
          overlay: self.overlay.as_ref(),
          notification,
          horodatage: self.instant_commande.clone(),
          alerte: self.alerte_courante(),
        };
        vue.dessiner(cadre);
      }
  }

/// Vue de rendu, indépendante des ressources du PTY (donc testable seule).
struct Vue<'a>
  {
    term: &'a Terminal,
    config: &'a Config,
    root: bool,
    identite: String,
    charge: String,
    options: Vec<String>,
    suggestion: Option<String>,
    ghost: Vec<char>,
    decalage: usize,
    overlay_decalage: usize,
    systeme: String,
    overlay: Option<&'a Overlay>,
    notification: Option<String>,
    horodatage: Option<String>,
    alerte: Option<String>,
  }

impl Vue<'_>
  {
    fn dessiner(&self, cadre: &mut ratatui::Frame)
      {
        let aire = cadre.area();
        let largeur = aire.width;
        let hauteur = aire.height;
        if hauteur < 3 || largeur == 0
          {
            return;
          }

        let stat_texte = stat::contenu(largeur, &self.identite, &self.charge);
        let stat_style = Style::default()
          .fg(couleur_tuple(stat::TEXTE))
          .bg(couleur_tuple(stat::FOND))
          .add_modifier(Modifier::BOLD);
        cadre.render_widget(
          Paragraph::new(Line::from(Span::styled(stat_texte, stat_style))),
          Rect { x: 0, y: 0, width: largeur, height: 1 },
        );

        let contexte = context::Contexte {
          capture: self.config.capture,
          root: self.root,
          statut: self.term.dernier_statut,
          options: self.options.clone(),
          suggestion: self.suggestion.clone(),
          overlay: self.overlay.is_some(),
          systeme: self.systeme.clone(),
          notification: self.notification.clone(),
          horodatage: self.horodatage.clone(),
          alerte: self.alerte.clone(),
        };
        cadre.render_widget(
          Paragraph::new(ligne_contexte(&contexte, largeur)),
          Rect { x: 0, y: hauteur - 1, width: largeur, height: 1 },
        );

        let central = Rect { x: 0, y: 1, width: largeur, height: hauteur - 2 };
        let lignes = self.lignes_centrales(largeur);
        cadre.render_widget(Paragraph::new(lignes), central);

        if self.overlay.is_none() && self.term.curseur_visible && self.decalage == 0
          {
            let cy = 1 + self.term.grid.curseur_lig;
            let cx = self.term.grid.curseur_col.min(largeur.saturating_sub(1));
            if cy < hauteur - 1
              {
                cadre.set_cursor_position(Position::new(cx, cy));
              }
          }

        if let Some(overlay) = self.overlay
          {
            dessiner_overlay(cadre, overlay, central, self.overlay_decalage);
          }
      }

    fn lignes_centrales(&self, largeur: u16) -> Vec<Line<'static>>
      {
        let grid = &self.term.grid;
        let hauteur = grid.hauteur as usize;
        let largeur = largeur as usize;
        let arriere = grid.arriere_plan_len() as isize;
        let base = arriere - self.decalage as isize;
        let lig_ghost = grid.curseur_lig as isize + self.decalage as isize;
        let col_ghost = grid.curseur_col as usize;
        let mut lignes = Vec::with_capacity(hauteur);
        for lig in 0..hauteur
          {
            let source = base + lig as isize;
            let depuis_arriere = source < arriere;
            let mut spans: Vec<Span> = Vec::new();
            let mut courant: Option<(Style, String)> = None;
            for col in 0..largeur
              {
                let sur_ghost =
                  lig as isize == lig_ghost && col >= col_ghost && (col - col_ghost) < self.ghost.len();
                let (caractere, style) = if sur_ghost
                  {
                    (self.ghost[col - col_ghost], style_ghost())
                  }
                else if source < 0
                  {
                    (' ', Style::default())
                  }
                else if depuis_arriere
                  {
                    let cellule = grid
                      .arriere_plan(source as usize)
                      .and_then(|ligne| ligne.get(col).copied())
                      .unwrap_or_default();
                    if cellule.continuation
                      {
                        continue;
                      }
                    (cellule.c, style_cellule(&cellule))
                  }
                else
                  {
                    let cellule = grid.cellule((source - arriere) as u16, col as u16);
                    if cellule.continuation
                      {
                        continue;
                      }
                    (cellule.c, style_cellule(&cellule))
                  };
                match &mut courant
                  {
                    Some((style_courant, texte)) if *style_courant == style => texte.push(caractere),
                    Some((style_courant, texte)) => {
                      spans.push(Span::styled(std::mem::take(texte), *style_courant));
                      courant = Some((style, caractere.to_string()));
                    }
                    None => courant = Some((style, caractere.to_string())),
                  }
              }
            if let Some((style, texte)) = courant
              {
                spans.push(Span::styled(texte, style));
              }
            if self.config.capture
              {
                let brut: String = spans.iter().map(|span| span.content.as_ref()).collect();
                let masque = providers::masquer(&brut);
                if masque != brut
                  {
                    spans = vec![Span::raw(masque)];
                  }
              }
            lignes.push(Line::from(spans));
          }
        lignes
      }
  }

fn couleur_tuple(rgb: (u8, u8, u8)) -> Color
  {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
  }

/// Compose la ligne de contexte sur toute la largeur (gauche / centre / droite).
fn ligne_contexte(contexte: &context::Contexte, largeur: u16) -> Line<'static>
  {
    let largeur = largeur as usize;
    if largeur == 0
      {
        return Line::default();
      }
    let base = Style::default()
      .fg(couleur_tuple(context::TEXTE))
      .bg(couleur_tuple(context::FOND))
      .add_modifier(Modifier::BOLD);
    let style_centre = Style::default()
      .fg(couleur_tuple(context::couleur_centre(contexte)))
      .bg(couleur_tuple(context::FOND))
      .add_modifier(Modifier::BOLD);
    let mut cellules: Vec<(char, Style)> = vec![(' ', base); largeur];

    let droite: Vec<char> = context::droite(contexte).chars().collect();
    let droite = &droite[..droite.len().min(largeur)];
    let depart_droite = largeur - droite.len();
    for (i, ch) in droite.iter().enumerate()
      {
        cellules[depart_droite + i] = (*ch, base);
      }

    let centre: Vec<char> = context::centre(contexte).chars().collect();
    let centre = &centre[..centre.len().min(depart_droite.max(1))];
    let depart_centre = largeur.saturating_sub(centre.len()) / 2;
    for (i, ch) in centre.iter().enumerate()
      {
        let position = depart_centre + i;
        if position < depart_droite
          {
            cellules[position] = (*ch, style_centre);
          }
      }

    let gauche: Vec<char> = context::gauche(contexte).chars().collect();
    let gauche = &gauche[..gauche.len().min(depart_centre)];
    for (i, ch) in gauche.iter().enumerate()
      {
        cellules[i] = (*ch, base);
      }

    let mut spans: Vec<Span> = Vec::new();
    let mut courant: Option<(Style, String)> = None;
    for (ch, style) in cellules
      {
        match &mut courant
          {
            Some((style_courant, texte)) if *style_courant == style => texte.push(ch),
            Some((style_courant, texte)) => {
              spans.push(Span::styled(std::mem::take(texte), *style_courant));
              courant = Some((style, ch.to_string()));
            }
            None => courant = Some((style, ch.to_string())),
          }
      }
    if let Some((style, texte)) = courant
      {
        spans.push(Span::styled(texte, style));
      }
    Line::from(spans)
  }

fn couleur_ratatui(couleur: Couleur) -> Color
  {
    match couleur
      {
        Couleur::Defaut => Color::Reset,
        Couleur::Index(n) => Color::Indexed(n),
        Couleur::Rgb(r, g, b) => Color::Rgb(r, g, b),
      }
  }

fn style_cellule(cellule: &Cellule) -> Style
  {
    let mut avant = couleur_ratatui(cellule.avant);
    let mut arriere = couleur_ratatui(cellule.arriere);
    if cellule.inverse
      {
        std::mem::swap(&mut avant, &mut arriere);
      }
    let mut style = Style::default().fg(avant).bg(arriere);
    if cellule.gras
      {
        style = style.add_modifier(Modifier::BOLD);
      }
    if cellule.italique
      {
        style = style.add_modifier(Modifier::ITALIC);
      }
    if cellule.clignote
      {
        style = style.add_modifier(Modifier::SLOW_BLINK);
      }
    style
  }

/// Style de la suggestion fantôme affichée après le curseur.
fn style_ghost() -> Style
  {
    Style::default()
      .fg(Color::Rgb(0x6c, 0x70, 0x86))
      .add_modifier(Modifier::ITALIC)
  }

fn dessiner_overlay(cadre: &mut ratatui::Frame, overlay: &Overlay, zone: Rect, decalage: usize)
  {
    let largeur_max = overlay.lignes.iter().map(|l| l.chars().count()).max().unwrap_or(24) as u16;
    let largeur = (largeur_max + 4).max(24).min(zone.width.saturating_sub(2).max(24));
    let hauteur = (overlay.lignes.len() as u16 + 4).min(zone.height.saturating_sub(1).max(6));
    let Some(rect) = op_tui::layout::popup_rect(zone, largeur, hauteur)
      else
        {
          return;
        };
    let interieur = op_tui::overlay::cadre(cadre, rect, &overlay.titre, op_tui::Popup::SOMBRE);
    let mut contenu: Vec<String> = overlay.lignes.clone();
    contenu.push(String::new());
    contenu.push(overlay.aide.clone());
    let max_decalage = contenu.len().saturating_sub(interieur.height as usize);
    let decalage = decalage.min(max_decalage);
    let avec_conseil = max_decalage > 0;
    let visibles = (interieur.height as usize).saturating_sub(usize::from(avec_conseil));
    let mut lignes: Vec<Line> = contenu
      .iter()
      .skip(decalage)
      .take(visibles)
      .map(|l| Line::from(l.clone()))
      .collect();
    if avec_conseil
      {
        lignes.push(Line::from(format!("[{}‰] ↑/↓ ou PgPréc/PgSuiv pour défiler", decalage)));
      }
    if lignes.is_empty()
      {
        lignes.push(Line::from(" "));
      }
    cadre.render_widget(Paragraph::new(lignes), interieur);
  }

/// Valeur de modificateur xterm (`1` = aucun).
fn valeur_modificateur(ctrl: bool, alt: bool, shift: bool) -> u8
  {
    1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl)
  }

/// Séquence CSI d'une touche de curseur, modificateurs inclus.
fn sequence_curseur(finale: u8, ctrl: bool, alt: bool, shift: bool) -> Vec<u8>
  {
    let modificateur = valeur_modificateur(ctrl, alt, shift);
    if modificateur == 1
      {
        vec![0x1b, b'[', finale]
      }
    else
      {
        format!("\x1b[1;{modificateur}{}", finale as char).into_bytes()
      }
  }

/// Séquence CSI `n~` (Insert, Delete, PageUp, PageDown…), modificateurs inclus.
fn sequence_csi_tilde(code: u8, ctrl: bool, alt: bool, shift: bool) -> Vec<u8>
  {
    let modificateur = valeur_modificateur(ctrl, alt, shift);
    if modificateur == 1
      {
        format!("\x1b[{code}~").into_bytes()
      }
    else
      {
        format!("\x1b[{code};{modificateur}~").into_bytes()
      }
  }

/// Encode une touche en octets à transmettre au shell (Alt/Meta, Ctrl, Shift).
fn encoder_touche(touche: &KeyEvent) -> Vec<u8>
  {
    let ctrl = touche.modifiers.contains(KeyModifiers::CONTROL);
    let alt = touche.modifiers.contains(KeyModifiers::ALT);
    let shift = touche.modifiers.contains(KeyModifiers::SHIFT);

    if ctrl
      && !alt
      && let KeyCode::Char(caractere) = touche.code
      {
        let majuscule = caractere.to_ascii_uppercase();
        if majuscule.is_ascii()
          {
            return vec![(majuscule as u8) & 0x1f];
          }
      }

    let octets = match touche.code
      {
        KeyCode::Char(caractere) => {
          let mut tampon = [0u8; 4];
          caractere.encode_utf8(&mut tampon).as_bytes().to_vec()
        }
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Left => sequence_curseur(b'D', ctrl, alt, shift),
        KeyCode::Right => sequence_curseur(b'C', ctrl, alt, shift),
        KeyCode::Up => sequence_curseur(b'A', ctrl, alt, shift),
        KeyCode::Down => sequence_curseur(b'B', ctrl, alt, shift),
        KeyCode::Home => sequence_curseur(b'H', ctrl, alt, shift),
        KeyCode::End => sequence_curseur(b'F', ctrl, alt, shift),
        KeyCode::Delete => sequence_csi_tilde(3, ctrl, alt, shift),
        KeyCode::Insert => sequence_csi_tilde(2, ctrl, alt, shift),
        KeyCode::PageUp => sequence_csi_tilde(5, ctrl, alt, shift),
        KeyCode::PageDown => sequence_csi_tilde(6, ctrl, alt, shift),
        KeyCode::F(n) => sequence_fonction(n),
        _ => Vec::new(),
      };
    if alt && !octets.is_empty() && octets[0] != 0x1b
      {
        let mut precede = Vec::with_capacity(octets.len() + 1);
        precede.push(0x1b);
        precede.extend_from_slice(&octets);
        return precede;
      }
    octets
  }

/// Code de base d'un bouton souris (SGR/legacy).
fn bouton_base(bouton: MouseButton) -> u8
  {
    match bouton
      {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
      }
  }

/// Encode un évènement souris selon le protocole demandé par le programme.
fn encoder_souris(term: &Terminal, ev: &MouseEvent) -> Option<Vec<u8>>
  {
    match term.souris
      {
        SourisMode::Aucune => return None,
        SourisMode::X10 if !matches!(ev.kind, MouseEventKind::Down(_)) => return None,
        SourisMode::Bouton if matches!(ev.kind, MouseEventKind::Drag(_) | MouseEventKind::Moved) => {
          return None;
        }
        _ => {}
      }
    let (code, relache) = match ev.kind
      {
        MouseEventKind::Down(bouton) => (bouton_base(bouton), false),
        MouseEventKind::Up(bouton) => (bouton_base(bouton), true),
        MouseEventKind::Drag(bouton) => (bouton_base(bouton) + 32, false),
        MouseEventKind::Moved => (3 + 32, false),
        MouseEventKind::ScrollUp => (64, false),
        MouseEventKind::ScrollDown => (65, false),
        MouseEventKind::ScrollLeft => (66, false),
        MouseEventKind::ScrollRight => (67, false),
      };
    let x = ev.column + 1;
    let y = ev.row + 1;
    if term.souris_sgr
      {
        let finale = if relache { 'm' } else { 'M' };
        Some(format!("\x1b[<{code};{x};{y}{finale}").into_bytes())
      }
    else if term.souris_utf8
      {
        Some(format!("\x1b[{code};{x};{y}M").into_bytes())
      }
    else
      {
        let code = if relache { 3 } else { code };
        let cx = x.min(223) as u8 + 32;
        let cy = y.min(223) as u8 + 32;
        Some(vec![0x1b, b'[', b'M', 32 + code, cx, cy])
      }
  }

fn sequence_fonction(n: u8) -> Vec<u8>
  {
    match n
      {
        1 => b"\x1bOP".to_vec(),
        2 => b"\x1bOQ".to_vec(),
        3 => b"\x1bOR".to_vec(),
        4 => b"\x1bOS".to_vec(),
        5 => b"\x1b[15~".to_vec(),
        6 => b"\x1b[17~".to_vec(),
        7 => b"\x1b[18~".to_vec(),
        8 => b"\x1b[19~".to_vec(),
        9 => b"\x1b[20~".to_vec(),
        10 => b"\x1b[21~".to_vec(),
        11 => b"\x1b[23~".to_vec(),
        12 => b"\x1b[24~".to_vec(),
        _ => Vec::new(),
      }
  }

/// Nombre maximal de lignes d'historique conservées en mémoire.
const HISTORIQUE_MAX: usize = 20_000;

/// Charge l'historique (bash et zsh) pour les suggestions, borné.
fn charger_historique() -> Vec<String>
  {
    let mut lignes = Vec::new();
    let Some(home) = std::env::var_os("HOME")
      else
        {
          return lignes;
        };
    let home = PathBuf::from(home);
    for nom in [".bash_history", ".zsh_history"]
      {
        if let Ok(contenu) = std::fs::read_to_string(home.join(nom))
          {
            for ligne in contenu.lines()
              {
                let ligne = ligne.trim();
                if !ligne.is_empty()
                  {
                    lignes.push(ligne.to_string());
                  }
              }
          }
      }
    if lignes.len() > HISTORIQUE_MAX
      {
        lignes.drain(0..lignes.len() - HISTORIQUE_MAX);
      }
    lignes
  }

/// Lance `programme argument` et renvoie sa sortie, en tuant le processus
/// s'il dépasse `delai` (évite tout gel/blocage sur un `--help` capricieux).
fn sortie_avec_delai(programme: &str, argument: &str, delai: Duration) -> Option<std::process::Output>
  {
    sortie_avec_delai_env(programme, argument, delai, &[])
  }

/// Variante de [`sortie_avec_delai`] avec variables d'environnement à poser.
fn sortie_avec_delai_env(
    programme: &str,
    argument: &str,
    delai: Duration,
    environnement: &[(&str, &str)],
  ) -> Option<std::process::Output>
  {
    use std::process::{Command, Stdio};
    let mut commande = Command::new(programme);
    commande.arg(argument).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (cle, valeur) in environnement
      {
        commande.env(cle, valeur);
      }
    let mut enfant = commande.spawn().ok()?;
    let limite = Instant::now() + delai;
    loop
      {
        match enfant.try_wait()
          {
            Ok(Some(_)) => return enfant.wait_with_output().ok(),
            Ok(None) if Instant::now() >= limite => {
              let _ = enfant.kill();
              let _ = enfant.wait();
              return None;
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
          }
      }
  }

/// Nettoie la sortie de `man` : retire les gras/soulignés par retour arrière
/// (`A\x08A`) et les retours chariot.
fn nettoyer_man(texte: &str) -> String
  {
    let mut sortie = String::with_capacity(texte.len());
    for caractere in texte.chars()
      {
        if caractere == '\u{0008}'
          {
            sortie.pop();
          }
        else
          {
            sortie.push(caractere);
          }
      }
    sortie.replace('\r', "")
  }

/// Repli sur la page `man` de la commande (bornée), si disponible.
fn aide_man(commande: &str) -> Option<Vec<String>>
  {
    let sortie = sortie_avec_delai_env("man", commande, Duration::from_secs(2), &[("MANWIDTH", "80")])?;
    let texte = if sortie.stdout.is_empty()
      {
        String::from_utf8_lossy(&sortie.stderr).to_string()
      }
    else
      {
        String::from_utf8_lossy(&sortie.stdout).to_string()
      };
    let net = nettoyer_man(&texte);
    if net.trim().is_empty()
      {
        return None;
      }
    Some(
      net.lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.trim().is_empty())
        .take(200)
        .collect(),
    )
  }

/// Exécute `commande --help` et renvoie sa sortie (bornée), si disponible.
fn aide_commande(commande: &str) -> Option<Vec<String>>
  {
    if commande.contains('/') || commande.starts_with('-')
      {
        return None;
      }
    let sortie = sortie_avec_delai(commande, "--help", Duration::from_secs(2))?;
    let texte = if sortie.stdout.is_empty()
      {
        String::from_utf8_lossy(&sortie.stderr).to_string()
      }
    else
      {
        String::from_utf8_lossy(&sortie.stdout).to_string()
      };
    if texte.trim().is_empty()
      {
        return aide_man(commande);
      }
    Some(texte.lines().take(200).map(|l| l.trim_end().to_string()).collect())
  }

/// Extrait les options `-…`/`--…` de la sortie de `commande --help`.
fn options_du_help(commande: &str) -> Vec<String>
  {
    let Some(programme) = commande.split_whitespace().next()
      else
        {
          return Vec::new();
        };
    if programme.is_empty() || programme.contains('/') || programme.starts_with('-')
      {
        return Vec::new();
      }
    let Some(sortie) = sortie_avec_delai(programme, "--help", Duration::from_secs(3))
      else
        {
          return Vec::new();
        };
    let texte = if sortie.stdout.is_empty()
      {
        String::from_utf8_lossy(&sortie.stderr).to_string()
      }
    else
      {
        String::from_utf8_lossy(&sortie.stdout).to_string()
      };
    let mut options: Vec<String> = Vec::new();
    for jeton in texte.split_whitespace()
      {
        let propre = jeton.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '=');
        let second = propre.chars().nth(1);
        if propre.starts_with('-')
          && propre.len() > 1
          && second.map(|c| c.is_ascii_alphabetic() || c == '-').unwrap_or(false)
          {
            let nom = propre.split('=').next().unwrap_or(propre).to_string();
            if nom.len() > 1 && nom != "--" && !options.iter().any(|o| o == &nom)
              {
                options.push(nom);
              }
          }
        if options.len() >= 40
          {
            break;
          }
      }
    options
  }

/// Formatte une durée en « N s » ou « M min S s ».
fn format_duree(duree: Duration) -> String
  {
    let secondes = duree.as_secs();
    if secondes < 60
      {
        return format!("{secondes} s");
      }
    format!("{} min {} s", secondes / 60, secondes % 60)
  }

/// Répertoire de base pour les fichiers temporaires : `XDG_RUNTIME_DIR` si
/// disponible (nettoyé par le système), sinon le répertoire temporaire.
fn base_runtime() -> PathBuf
  {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR")
      {
        let chemin = PathBuf::from(dir);
        if chemin.is_dir()
          {
            return chemin;
          }
      }
    std::env::temp_dir()
  }

/// Restreint un répertoire au propriétaire (0700) sous Unix.
#[cfg(unix)]
fn proteger(chemin: &std::path::Path)
  {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(chemin, std::fs::Permissions::from_mode(0o700));
  }

/// Sans objet hors Unix.
#[cfg(not(unix))]
fn proteger(_chemin: &std::path::Path) {}

/// Prépare le fichier de démarrage du shell (bash : `--rcfile`, zsh : `ZDOTDIR`)
/// dans un répertoire privé propre au pid.
fn preparer_rc(shell: Shell, theme: &Theme, largeur_suffixe: u16) -> Option<RcPrepare>
  {
    let dossier = base_runtime().join(format!("bashop-{}", std::process::id()));
    std::fs::create_dir_all(&dossier).ok()?;
    proteger(&dossier);
    match shell
      {
        Shell::Bash => {
          let chemin = dossier.join("bashrc");
          std::fs::write(&chemin, crate::bootstrap::rc_bash(theme, largeur_suffixe)).ok()?;
          Some(RcPrepare::Rcfile(chemin))
        }
        Shell::Zsh => {
          std::fs::write(dossier.join(".zshenv"), crate::bootstrap::zshenv_zsh()).ok()?;
          std::fs::write(dossier.join(".zprofile"), crate::bootstrap::zprofile_zsh()).ok()?;
          std::fs::write(dossier.join(".zlogin"), crate::bootstrap::zlogin_zsh()).ok()?;
          std::fs::write(dossier.join(".zshrc"), crate::bootstrap::zshrc_zsh(theme, largeur_suffixe)).ok()?;
          Some(RcPrepare::Zdotdir(dossier))
        }
        _ => None,
      }
  }

fn nettoyer_rc(rc: Option<RcPrepare>)
  {
    match rc
      {
        Some(RcPrepare::Rcfile(chemin)) => {
          let _ = std::fs::remove_file(&chemin);
          if let Some(parent) = chemin.parent()
            {
              let _ = std::fs::remove_dir(parent);
            }
        }
        Some(RcPrepare::Zdotdir(dossier)) => {
          for nom in [".zshenv", ".zprofile", ".zshrc", ".zlogin"]
            {
              let _ = std::fs::remove_file(dossier.join(nom));
            }
          let _ = std::fs::remove_dir(&dossier);
        }
        None => {}
      }
  }

#[cfg(test)]
mod tests
  {
    use super::*;
    use crate::config::Config;
    use crate::term::Terminal;
    use ratatui::Terminal as RatTerminal;
    use ratatui::backend::TestBackend;

    fn texte_buffer(buffer: &ratatui::buffer::Buffer) -> String
      {
        let mut texte = String::new();
        for cellule in buffer.content()
          {
            texte.push_str(cellule.symbol());
          }
        texte
      }

    #[test]
    fn le_cadre_affiche_la_sortie_du_shell()
      {
        let mut term = Terminal::nouveau(100, 6);
        let mut parser = vte::Parser::new();
        term.alimenter(&mut parser, b"bonjour\r\n");
        let config = Config::default();
        let vue = Vue {
          term: &term,
          config: &config,
          root: false,
          identite: "test@hote".to_string(),
          charge: "0.42".to_string(),
          options: vec!["--all".to_string()],
          suggestion: None,
          ghost: Vec::new(),
          decalage: 0,
          overlay_decalage: 0,
          systeme: "CPU 5%  MEM 20%  IO 0 o/s".to_string(),
          overlay: None,
          notification: None,
          horodatage: None,
          alerte: None,
        };
        let mut terminal = RatTerminal::new(TestBackend::new(100, 8)).unwrap();
        terminal.draw(|cadre| vue.dessiner(cadre)).unwrap();
        let texte = texte_buffer(terminal.backend().buffer());
        assert!(texte.contains("bonjour"), "sortie absente : {texte}");
        assert!(texte.contains('@'), "identité absente : {texte}");
        assert!(texte.contains("Aide contextuelle"), "contexte absent : {texte}");
        assert!(texte.contains("CPU 5%"), "compteurs système absents : {texte}");
      }

    #[test]
    fn un_terminal_trop_petit_ne_panique_pas()
      {
        let term = Terminal::nouveau(10, 1);
        let config = Config::default();
        let vue = Vue {
          term: &term,
          config: &config,
          root: false,
          identite: "test@hote".to_string(),
          charge: String::new(),
          options: Vec::new(),
          suggestion: None,
          ghost: Vec::new(),
          decalage: 0,
          overlay_decalage: 0,
          systeme: String::new(),
          overlay: None,
          notification: None,
          horodatage: None,
          alerte: None,
        };
        let mut terminal = RatTerminal::new(TestBackend::new(10, 2)).unwrap();
        terminal.draw(|cadre| vue.dessiner(cadre)).unwrap();
      }

    #[test]
    fn ctrl_c_donne_un_seul_octet()
      {
        let touche = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(encoder_touche(&touche), vec![0x03]);
      }

    #[test]
    fn alt_sur_une_lettre_ajoute_le_prefixe_escape()
      {
        let touche = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT);
        assert_eq!(encoder_touche(&touche), vec![0x1b, b'f']);
      }

    #[test]
    fn ctrl_majuscule_et_ctrl_les_deux_gestes()
      {
        let controle = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(encoder_touche(&controle), vec![0x17]);
        let controle_maj = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        assert_eq!(encoder_touche(&controle_maj), vec![0x17]);
      }

    #[test]
    fn les_sequences_sur_les_fleches_portent_les_modificateurs()
      {
        let gauche = KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL);
        assert_eq!(encoder_touche(&gauche), b"\x1b[1;5D".to_vec());
        let haut = KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT);
        assert_eq!(encoder_touche(&haut), b"\x1b[1;2A".to_vec());
        let simple = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(encoder_touche(&simple), b"\x1b[C".to_vec());
      }

    #[test]
    fn la_version_non_privee_des_touches_fonctionnaires_est_conservee()
      {
        let f1 = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(encoder_touche(&f1), b"\x1bOP".to_vec());
        let delete = KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT);
        assert_eq!(encoder_touche(&delete), b"\x1b[3;2~".to_vec());
      }

    #[test]
    fn la_souris_sgr_suit_le_mode_du_terminal()
      {
        let mut term = Terminal::nouveau(100, 30);
        term.souris = SourisMode::Mouvement;
        term.souris_sgr = true;
        let ev = MouseEvent {
          kind: MouseEventKind::Down(MouseButton::Left),
          column: 4,
          row: 2,
          modifiers: KeyModifiers::NONE,
        };
        let enc = encoder_souris(&term, &ev).unwrap();
        assert_eq!(enc, b"\x1b[<0;5;3M".to_vec());
      }

    #[test]
    fn la_souris_relachee_se_termine_par_m()
      {
        let mut term = Terminal::nouveau(100, 30);
        term.souris = SourisMode::Bouton;
        term.souris_sgr = true;
        let ev = MouseEvent {
          kind: MouseEventKind::Up(MouseButton::Right),
          column: 1,
          row: 1,
          modifiers: KeyModifiers::NONE,
        };
        let enc = encoder_souris(&term, &ev).unwrap();
        assert_eq!(enc, b"\x1b[<2;2;2m".to_vec());
      }

    #[test]
    fn la_souris_est_tuee_quand_personne_ne_l_a_demandee()
      {
        let term = Terminal::nouveau(100, 30);
        let ev = MouseEvent {
          kind: MouseEventKind::Down(MouseButton::Left),
          column: 1,
          row: 1,
          modifiers: KeyModifiers::NONE,
        };
        assert!(encoder_souris(&term, &ev).is_none());
      }

    #[test]
    fn sans_suivi_les_mouvements_ne_sont_pas_retransmis()
      {
        let mut term = Terminal::nouveau(100, 30);
        term.souris = SourisMode::Bouton;
        term.souris_sgr = true;
        let ev = MouseEvent {
          kind: MouseEventKind::Moved,
          column: 1,
          row: 1,
          modifiers: KeyModifiers::NONE,
        };
        assert!(encoder_souris(&term, &ev).is_none());
      }

    #[test]
    fn la_souris_legacy_encode_sur_trois_octets()
      {
        let mut term = Terminal::nouveau(100, 30);
        term.souris = SourisMode::Bouton;
        let ev = MouseEvent {
          kind: MouseEventKind::Down(MouseButton::Middle),
          column: 9,
          row: 12,
          modifiers: KeyModifiers::NONE,
        };
        let enc = encoder_souris(&term, &ev).unwrap();
        assert_eq!(enc, vec![0x1b, b'[', b'M', 32 + 1, 32 + 10, 32 + 13]);
      }

    #[test]
    fn la_sortie_de_man_est_nettoyee_des_surimpressions()
      {
        let net = nettoyer_man("H\x08H e\x08e y\x08y\r\nligne ligne");
        assert_eq!(net, "H e y\nligne ligne");
      }

    #[test]
    fn une_commande_sans_aide_tombe_sur_la_page_man()
      {
        let mots = aide_commande("env").unwrap_or_default();
        assert!(!mots.is_empty(), "env --help ou man manquants");
      }
  }
