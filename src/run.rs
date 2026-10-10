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

use anyhow::Result;
use crossterm::event::{
  self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
  MouseEventKind,
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
use crate::frame::{context, stat};
use crate::observe::EtatShell;
use crate::providers::{self, MesureSysteme};
use crate::safety;
use crate::shells::Shell;
use crate::term::Terminal;
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
      let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
      hook(info);
    }));

    enable_raw_mode()?;
    let mut sortie = io::stdout();
    execute!(sortie, EnterAlternateScreen, EnableMouseCapture)?;
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
      historique: charger_historique(),
      systeme: MesureSysteme::nouveau(),
      derniere_maj: Instant::now(),
      quitter: false,
    };

    let resultat = app.boucle(&mut terminal);

    disable_raw_mode()?;
    let arriere = terminal.backend_mut();
    let _ = write!(arriere, "{}", reinitialiser_fond());
    execute!(arriere, LeaveAlternateScreen, DisableMouseCapture)?;
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
    historique: Vec<String>,
    systeme: MesureSysteme,
    derniere_maj: Instant,
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
                self.derniere_maj = Instant::now();
                sale = true;
              }
            loop
              {
                match self.recepteur.try_recv()
                  {
                    Ok(octets) => {
                      self.term.alimenter(&mut self.parser, &octets);
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
                      self.ecrivain.write_all(texte.as_bytes())?;
                      self.ecrivain.flush()?;
                      sale = true;
                    }
                    Event::Mouse(souris) => {
                      match souris.kind
                        {
                          MouseEventKind::ScrollUp => self.defiler(3),
                          MouseEventKind::ScrollDown => self.defiler(-3),
                          _ => {}
                        }
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
        let au_prompt = matches!(self.term.etat, EtatShell::Prompt | EtatShell::Inconnu);
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
                    }
                }
                _ => {
                  self.a_executer = None;
                }
              }
            self.overlay = None;
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
            return Ok(());
          }
        self.overlay_decalage = 0;
        self.overlay = None;
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

    fn valider(&mut self) -> Result<()>
      {
        let commande = self.term.commande_courante();
        let verdict = safety::evaluer(&commande);
        if verdict.niveau == safety::Niveau::Danger
          {
            let mut lignes = vec![format!("Commande : {}", commande), String::new()];
            for motif in &verdict.motifs
              {
                lignes.push(format!("• {motif}"));
              }
            self.a_executer = Some(b"\r".to_vec());
            self.overlay_decalage = 0;
            self.overlay = Some(Overlay::confirmation("Confirmation requise", lignes));
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
        self.suggestion = None;
        self.ghost = None;
        self.decalage = 0;
        self.overlay = None;
        Ok(())
      }

    /// Fait défiler la vue de `delta` lignes (<0 vers le haut, >0 vers le bas).
    fn defiler(&mut self, delta: i32)
      {
        let max = self.term.grid.arriere_plan_len() as i32;
        self.decalage = (self.decalage as i32 + delta).clamp(0, max) as usize;
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
        if !matches!(self.term.etat, EtatShell::Prompt | EtatShell::Inconnu)
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
          "Bashop — cadre terminal permanent (3 lignes)".to_string(),
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
        self.overlay = Some(Overlay::information("Aide de Bashop", lignes));
      }

    fn root_actif(&self) -> bool
      {
        if crate::detect::est_root()
          {
            return true;
          }
        let commande = self.term.commande_courante();
        crate::lexer::contient_declencheur(&commande, &self.config.triggers)
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
        let vue = Vue {
          term: &self.term,
          config: &self.config,
          root: self.root_actif(),
          options: self.options_courantes(),
          suggestion,
          ghost: ghost.unwrap_or_default().chars().collect(),
          decalage: self.decalage,
          overlay_decalage: self.overlay_decalage,
          systeme: self.systeme.texte.clone(),
          overlay: self.overlay.as_ref(),
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
    options: Vec<String>,
    suggestion: Option<String>,
    ghost: Vec<char>,
    decalage: usize,
    overlay_decalage: usize,
    systeme: String,
    overlay: Option<&'a Overlay>,
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

        let stat_texte = stat::contenu(largeur);
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

/// Encode une touche en octets à transmettre au shell.
fn encoder_touche(touche: &KeyEvent) -> Vec<u8>
  {
    if touche.modifiers.contains(KeyModifiers::CONTROL)
      && let KeyCode::Char(caractere) = touche.code
      {
        let majuscule = caractere.to_ascii_uppercase();
        if majuscule.is_ascii()
          {
            return vec![(majuscule as u8) & 0x1f];
          }
      }
    match touche.code
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
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(n) => sequence_fonction(n),
        _ => Vec::new(),
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

/// Charge l'historique (bash et zsh) pour les suggestions.
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
    lignes
  }

/// Exécute `commande --help` et renvoie sa sortie (bornée), si disponible.
fn aide_commande(commande: &str) -> Option<Vec<String>>
  {
    if commande.contains('/') || commande.starts_with('-')
      {
        return None;
      }
    let sortie = std::process::Command::new(commande).arg("--help").output().ok()?;
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
        return None;
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
    let Ok(sortie) = std::process::Command::new(programme).arg("--help").output()
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

/// Prépare le fichier de démarrage du shell (bash : `--rcfile`, zsh : `ZDOTDIR`).
fn preparer_rc(shell: Shell, theme: &Theme, largeur_suffixe: u16) -> Option<RcPrepare>
  {
    let racine = std::env::temp_dir();
    let pid = std::process::id();
    match shell
      {
        Shell::Bash => {
          let chemin = racine.join(format!("bashop-{pid}.bashrc"));
          std::fs::write(&chemin, crate::bootstrap::rc_bash(theme, largeur_suffixe)).ok()?;
          Some(RcPrepare::Rcfile(chemin))
        }
        Shell::Zsh => {
          let dossier = racine.join(format!("bashop-{pid}-zsh"));
          std::fs::create_dir_all(&dossier).ok()?;
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
          let _ = std::fs::remove_file(chemin);
        }
        Some(RcPrepare::Zdotdir(dossier)) => {
          let _ = std::fs::remove_file(dossier.join(".zshrc"));
          let _ = std::fs::remove_dir(dossier);
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
          options: vec!["--all".to_string()],
          suggestion: None,
          ghost: Vec::new(),
          decalage: 0,
          overlay_decalage: 0,
          systeme: "CPU 5%  MEM 20%  IO 0 o/s".to_string(),
          overlay: None,
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
          options: Vec::new(),
          suggestion: None,
          ghost: Vec::new(),
          decalage: 0,
          overlay_decalage: 0,
          systeme: String::new(),
          overlay: None,
        };
        let mut terminal = RatTerminal::new(TestBackend::new(10, 2)).unwrap();
        terminal.draw(|cadre| vue.dessiner(cadre)).unwrap();
      }
  }
