//! Interface de configuration — `bashop config`.
//!
//! Formulaire éditable des clés reconnues de `~/.opshellrc`, construit sur les
//! primitives de `op-tui` (édition de ligne [`op_tui::edit::gerer`]) et rendu
//! par ratatui. Chaque champ est validé à la sortie ; les clés inconnues du
//! fichier sont conservées telles quelles.
//!
//! Contrats :
//! - `Échap` en liste quitte **et enregistre** ; `F2` enregistre aussi.
//! - `Échap` dans un champ annule l'édition de ce champ ; `Entrée` la valide
//!   et passe au champ suivant.
//! - `Entrée` sur un champ de couleur ouvre le **sélecteur** : grille de couleurs
//!   affichées en aplat, déplacement aux flèches, `Entrée` valide, `Échap` annule ;
//!   l'entrée « défaut » rétablit la couleur du thème.
//! - `bashop config --reload-rc` n'ouvre pas l'interface : il relit le fichier
//!   et affiche le résumé de la configuration active.

use crate::config::{self, Config};
use crate::theme::Rgb;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Terminal;
use std::collections::BTreeMap;
use std::io::{self, Write};
use unicode_width::UnicodeWidthStr;

/// Une ligne du formulaire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Champ
  {
    /// Clé de `~/.opshellrc` éditée.
    pub cle: &'static str,
    /// Valeur courante (texte brut de la clé).
    pub valeur: String,
    /// Position du curseur dans la valeur, en caractères.
    pub curseur: usize,
  }

/// Clés de couleur, dans l'ordre d'affichage.
const CLES_COULEUR: [&str; 6] =
  ["STAT_FG", "STAT_BG", "CONTEXT_FG", "CONTEXT_BG", "PROMPT_FG", "PROMPT_BG"];

/// Vrai si la clé désigne une couleur (champ piloté par le sélecteur).
fn est_couleur(cle: &str) -> bool
  {
    CLES_COULEUR.contains(&cle)
  }

/// Couleurs proposées par le sélecteur : saturées et bien distinctes les unes
/// des autres (contraste), du blanc aux fonds riches, sans case confondue avec
/// le fond du cadre `#2E3440`. L'entrée « défaut » (index 0) est ajoutée
/// séparément pour rétablir la couleur du thème.
const PALETTE: &[Rgb] = &[
  Rgb::new(0xFF, 0xFF, 0xFF),
  Rgb::new(0xDC, 0xE3, 0xEA),
  Rgb::new(0xA8, 0xB2, 0xC2),
  Rgb::new(0x6A, 0x74, 0x82),
  Rgb::new(0x3E, 0x48, 0x56),
  Rgb::new(0xFF, 0x3B, 0x30),
  Rgb::new(0xFF, 0x6A, 0x6A),
  Rgb::new(0xE5, 0x48, 0x4D),
  Rgb::new(0xF2, 0x84, 0x8F),
  Rgb::new(0xFF, 0x3D, 0xD1),
  Rgb::new(0xFF, 0x6E, 0xB4),
  Rgb::new(0xFF, 0x6B, 0x35),
  Rgb::new(0xFF, 0xA1, 0x40),
  Rgb::new(0xFF, 0xB0, 0x20),
  Rgb::new(0xFF, 0xC8, 0x3D),
  Rgb::new(0xFF, 0xE1, 0x4D),
  Rgb::new(0x66, 0xE0, 0x5C),
  Rgb::new(0x00, 0xD1, 0x69),
  Rgb::new(0x2E, 0x9E, 0x44),
  Rgb::new(0x7A, 0xF0, 0xC8),
  Rgb::new(0x1F, 0xE0, 0xCE),
  Rgb::new(0x21, 0xB8, 0xFF),
  Rgb::new(0x00, 0x96, 0x88),
  Rgb::new(0x3A, 0x7B, 0xFF),
  Rgb::new(0x13, 0x57, 0xF5),
  Rgb::new(0x79, 0xC0, 0xFF),
  Rgb::new(0x5B, 0x4D, 0xFF),
  Rgb::new(0x8B, 0x5C, 0xF6),
  Rgb::new(0xB2, 0x4B, 0xFF),
  Rgb::new(0xC0, 0x8D, 0xFF),
  Rgb::new(0xA9, 0x71, 0x3F),
  Rgb::new(0xB4, 0x54, 0x1F),
  Rgb::new(0x9B, 0x1B, 0x30),
  Rgb::new(0x5A, 0x23, 0x23),
  Rgb::new(0x1E, 0x4D, 0x2B),
  Rgb::new(0x19, 0x39, 0x5E),
  Rgb::new(0x3B, 0x2A, 0x5A),
  Rgb::new(0x5A, 0x2A, 0x3E),
  Rgb::new(0x4D, 0x3B, 0x1E),
];

/// Colonnes de la grille du sélecteur.
const PALETTE_COLONNES: usize = 6;
/// Largeur (colonnes) d'une case de couleur.
const PALETTE_CASE: usize = 8;

/// État du sélecteur de couleur ouvert sur un champ.
#[derive(Clone, Copy, Debug)]
pub struct PaletteEtat
  {
    /// Index dans la liste « défaut » (0) suivie des couleurs.
    index: usize,
  }

impl PaletteEtat
  {
    /// Nombre d'entrées (défaut compris).
    fn total() -> usize
      {
        PALETTE.len() + 1
      }

    /// Couleur de l'entrée courante (`None` = défaut).
    fn entree(&self) -> Option<Rgb>
      {
        if self.index == 0 { None } else { PALETTE.get(self.index - 1).copied() }
      }

    /// Sélection correspondant à la valeur d'un champ (`""` = défaut).
    fn depuis_valeur(valeur: &str) -> Self
      {
        if valeur.trim().is_empty()
          {
            return Self { index: 0 };
          }
        let index = config::parse_couleur(valeur)
          .and_then(|c| PALETTE.iter().position(|p| *p == c))
          .map(|i| i + 1)
          .unwrap_or(0);
        Self { index }
      }
  }

/// État du formulaire.
#[derive(Debug)]
pub struct Formulaire
  {
    /// Lignes du formulaire.
    pub champs: Vec<Champ>,
    /// Index du champ sélectionné.
    pub index: usize,
    /// `true` pendant l'édition du champ sélectionné.
    pub edition: bool,
    /// Sélecteur de couleur ouvert, le cas échéant.
    pub palette: Option<PaletteEtat>,
    /// Valeur avant l'édition (pour l'annulation par `Échap`).
    avant: String,
    /// Clés inconnues lues dans le fichier, préservées à l'écriture.
    inconnues: BTreeMap<String, String>,
  }

impl Formulaire
  {
    /// Construit le formulaire depuis une configuration.
    pub fn nouvelle(config: Config) -> Self
      {
        let mut champs = vec![
          champ("THEME", &config.theme),
          champ("TRIGGERS", &config.triggers.join(", ")),
          champ("SUFFIX_WIDTH", &config.suffix_width.to_string()),
          champ("CAPTURE", &bouton(config.capture)),
          champ("NOTIFICATION", &bouton(config.notification)),
          champ("NOTIFICATION_SEUIL", &config.seuil_notification.to_string()),
          champ("HORODATAGE", &bouton(config.horodatage)),
          champ("LIGNE_VIDE", &bouton(config.ligne_vide)),
        ];
        let couleurs = [
          ("STAT_FG", config.couleurs.stat_fg),
          ("STAT_BG", config.couleurs.stat_bg),
          ("CONTEXT_FG", config.couleurs.contexte_fg),
          ("CONTEXT_BG", config.couleurs.contexte_bg),
          ("PROMPT_FG", config.couleurs.prompt_fg),
          ("PROMPT_BG", config.couleurs.prompt_bg),
        ];
        for (cle, couleur) in couleurs
          {
            let valeur = couleur.map(config::format_couleur).unwrap_or_default();
            champs.push(champ(cle, &valeur));
          }
        Self {
          champs,
          index: 0,
          edition: false,
          palette: None,
          avant: String::new(),
          inconnues: config.inconnues,
        }
      }

    /// Reconstitue la configuration depuis le contenu saisi.
    pub fn vers_config(&self) -> Config
      {
        let mut c = Config {
          inconnues: self.inconnues.clone(),
          ..Config::default()
        };
        for champ in &self.champs
          {
            match champ.cle
              {
                "THEME" => c.theme = champ.valeur.trim().to_string(),
                "TRIGGERS" => {
                  c.triggers = champ
                    .valeur
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                }
                "SUFFIX_WIDTH" => {
                  c.suffix_width = champ
                    .valeur
                    .trim()
                    .parse::<u16>()
                    .unwrap_or(crate::config::SUFFIX_WIDTH_DEFAUT)
                    .max(1);
                }
                "CAPTURE" => c.capture = est_active(&champ.valeur),
                "NOTIFICATION" => c.notification = est_active(&champ.valeur),
                "NOTIFICATION_SEUIL" => {
                  c.seuil_notification = champ
                    .valeur
                    .trim()
                    .parse::<u64>()
                    .unwrap_or(crate::config::SEUIL_NOTIFICATION_DEFAUT)
                    .max(1);
                }
                "HORODATAGE" => c.horodatage = est_active(&champ.valeur),
                "LIGNE_VIDE" => c.ligne_vide = est_active(&champ.valeur),
                "STAT_FG" => c.couleurs.stat_fg = config::parse_couleur(&champ.valeur),
                "STAT_BG" => c.couleurs.stat_bg = config::parse_couleur(&champ.valeur),
                "CONTEXT_FG" => c.couleurs.contexte_fg = config::parse_couleur(&champ.valeur),
                "CONTEXT_BG" => c.couleurs.contexte_bg = config::parse_couleur(&champ.valeur),
                "PROMPT_FG" => c.couleurs.prompt_fg = config::parse_couleur(&champ.valeur),
                "PROMPT_BG" => c.couleurs.prompt_bg = config::parse_couleur(&champ.valeur),
                _ => {}
              }
          }
        c
      }

    /// Traite une touche. Retourne `true` quand il faut quitter (et enregistrer).
    pub fn traiter(&mut self, touche: KeyEvent) -> bool
      {
        if self.palette.is_some()
          {
            self.traiter_palette(touche);
            return false;
          }
        if self.edition
          {
            return self.traiter_edition(touche);
          }
        match touche.code
          {
            KeyCode::Up => {
              if self.index > 0
                {
                  self.index -= 1;
                }
            }
            KeyCode::Down => {
              if self.index + 1 < self.champs.len()
                {
                  self.index += 1;
                }
            }
            KeyCode::Enter | KeyCode::Tab => self.ouvrir(),
            KeyCode::F(2) | KeyCode::Esc => return true,
            _ => {}
          }
        false
      }

    /// Ouvre le champ sélectionné : sélecteur de couleur, ou édition de ligne.
    fn ouvrir(&mut self)
      {
        let cle = self.champs.get(self.index).expect("index de sélection hors bornes").cle;
        if est_couleur(cle)
          {
            let valeur = &self.champs[self.index].valeur;
            self.palette = Some(PaletteEtat::depuis_valeur(valeur));
          }
        else
          {
            self.entre_edition();
          }
      }

    /// Traite une touche dans le sélecteur de couleur.
    fn traiter_palette(&mut self, touche: KeyEvent)
      {
        let total = PaletteEtat::total();
        let Some(mut etat) = self.palette else { return; };
        match touche.code
          {
            KeyCode::Left => etat.index = etat.index.saturating_sub(1),
            KeyCode::Right => etat.index = (etat.index + 1).min(total - 1),
            KeyCode::Up => etat.index = etat.index.saturating_sub(PALETTE_COLONNES),
            KeyCode::Down => etat.index = (etat.index + PALETTE_COLONNES).min(total - 1),
            KeyCode::Enter | KeyCode::Tab => {
              let valeur = etat.entree().map(config::format_couleur).unwrap_or_default();
              if let Some(champ) = self.champs.get_mut(self.index)
                {
                  champ.curseur = valeur.chars().count();
                  champ.valeur = valeur;
                }
              self.palette = None;
              return;
            }
            KeyCode::Esc => {
              self.palette = None;
              return;
            }
            _ => {}
          }
        self.palette = Some(etat);
      }

    /// Traite une touche pendant l'édition d'un champ.
    fn traiter_edition(&mut self, touche: KeyEvent) -> bool
      {
        let champ = self
          .champs
          .get_mut(self.index)
          .expect("un index d'édition hors bornes");
        match touche.code
          {
            KeyCode::Esc => {
              champ.valeur = std::mem::take(&mut self.avant);
              champ.curseur = champ.valeur.chars().count();
              self.edition = false;
            }
            KeyCode::Enter => {
              champ.curseur = champ.valeur.chars().count();
              self.edition = false;
              if self.index + 1 < self.champs.len()
                {
                  self.index += 1;
                }
            }
            _ => {
              if let Some(geste) = geste_edition(touche)
                {
                  op_tui::edit::gerer(&mut champ.valeur, &mut champ.curseur, geste);
                }
            }
          }
        false
      }

    /// Passe en mode édition, en mémorisant la valeur pour l'annulation.
    fn entre_edition(&mut self)
      {
        let champ = self.champs.get_mut(self.index).expect("index de sélection hors bornes");
        self.avant = champ.valeur.clone();
        champ.curseur = champ.valeur.chars().count();
        self.edition = true;
      }
  }

fn champ(cle: &'static str, valeur: &str) -> Champ
  {
    Champ {
      cle,
      valeur: valeur.to_string(),
      curseur: valeur.chars().count(),
    }
  }

/// Représentation neutre d'un booléen dans le fichier.
fn bouton(actif: bool) -> String
  {
    if actif { "on" } else { "off" }.to_string()
  }

/// Interprète une valeur neutre (`on`/`off`, `true`, `1`…).
fn est_active(valeur: &str) -> bool
  {
    !matches!(
      valeur.trim().to_ascii_lowercase().as_str(),
      "off" | "false" | "0" | "no" | "non"
    )
  }

/// Traduit une touche crossterm vers un geste d'édition `op-tui`.
fn geste_edition(touche: KeyEvent) -> Option<op_tui::edit::Touche>
  {
    if touche.modifiers.contains(KeyModifiers::CONTROL)
      {
        return match touche.code
          {
            KeyCode::Left => Some(op_tui::edit::Touche::MotGauche),
            KeyCode::Right => Some(op_tui::edit::Touche::MotDroite),
            _ => None,
          };
      }
    match touche.code
      {
        KeyCode::Left => Some(op_tui::edit::Touche::Gauche),
        KeyCode::Right => Some(op_tui::edit::Touche::Droite),
        KeyCode::Home => Some(op_tui::edit::Touche::Debut),
        KeyCode::End => Some(op_tui::edit::Touche::Fin),
        KeyCode::PageUp => Some(op_tui::edit::Touche::PageHaut),
        KeyCode::PageDown => Some(op_tui::edit::Touche::PageBas),
        KeyCode::Backspace => Some(op_tui::edit::Touche::Retour),
        KeyCode::Delete => Some(op_tui::edit::Touche::Supprimer),
        KeyCode::Char(c)
          if !touche.modifiers.contains(KeyModifiers::ALT) =>
        {
          Some(op_tui::edit::Touche::Caractere(c))
        }
        _ => None,
      }
  }

/// Ouvre l'interface de configuration et renvoie la configuration éditée.
///
/// L'écriture du fichier reste à la charge de l'appelant (`config::ecrire`).
pub fn editer(initial: Config) -> io::Result<Config>
  {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let _garde = Restaurateur;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let mut formulaire = Formulaire::nouvelle(initial);
    let config = boucle(&mut terminal, &mut formulaire)?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(config)
  }

/// Restaure le terminal (écran alterné, mode brut) en cas de sortie abrupte.
struct Restaurateur;

impl Drop for Restaurateur
  {
    fn drop(&mut self)
      {
        let mut sortie = io::stdout();
        let _ = disable_raw_mode();
        let _ = execute!(sortie, LeaveAlternateScreen);
        let _ = sortie.flush();
      }
  }

/// Boucle d'événements : dessine, attend une touche, quitte sur `F2`/`Échap`.
fn boucle(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, formulaire: &mut Formulaire) -> io::Result<Config>
  {
    loop
      {
        terminal.draw(|frame| dessiner(frame, formulaire))?;
        match event::read()?
          {
            Event::Key(touche) => {
              if formulaire.traiter(touche)
                {
                  return Ok(formulaire.vers_config());
                }
            }
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost => continue,
            _ => {}
          }
      }
  }

/// Tronque une valeur à `max_colonnes` sans couper un caractère large.
fn tronquer(valeur: &str, max_colonnes: usize) -> String
  {
    let mut largeur = 0;
    let mut sortie = String::new();
    for c in valeur.chars()
      {
        let l = UnicodeWidthStr::width(c.to_string().as_str());
        if largeur + l > max_colonnes
          {
            break;
          }
        largeur += l;
        sortie.push(c);
      }
    sortie
  }

/// Dessine le formulaire : titre, champs surlignés, barre d'aide du mode.
fn dessiner(frame: &mut ratatui::Frame<'_>, formulaire: &Formulaire)
  {
    let zone = frame.area();
    let titre = if formulaire.edition
      {
        "Configuration ~/.opshellrc — édition"
      }
    else
      {
        "Configuration ~/.opshellrc"
      };
    let mut lignes = Vec::new();
    for (i, champ) in formulaire.champs.iter().enumerate()
      {
        let prefixe = format!("{:>14}  ", champ.cle);
        let largeur_prefixe = UnicodeWidthStr::width(prefixe.as_str());
        let surligne = i == formulaire.index && formulaire.palette.is_none();
        let style = if surligne && formulaire.edition
          {
            Style::default().fg(Color::Black).bg(Color::Cyan)
          }
        else if surligne
          {
            Style::default().add_modifier(Modifier::REVERSED)
          }
        else
          {
            Style::default()
          };
        let mut spans = vec![Span::styled(prefixe, style)];
        if est_couleur(champ.cle)
          {
            match config::parse_couleur(&champ.valeur)
              {
                Some(c) => {
                  spans.push(Span::styled("   ", style.bg(Color::Rgb(c.r, c.g, c.b))));
                  spans.push(Span::styled(format!("  {}", config::format_couleur(c)), style));
                }
                None => spans.push(Span::styled("   défaut", style)),
              }
          }
        else
          {
            let disponible = usize::from(zone.width).saturating_sub(largeur_prefixe).saturating_sub(3);
            spans.push(Span::styled(tronquer(&champ.valeur, disponible), style));
          }
        let mut ligne = Line::from(spans);
        if surligne
          {
            let exacte = ligne.width();
            let disponible = usize::from(zone.width).saturating_sub(2);
            if exacte < disponible
              {
                ligne.push_span(Span::styled(" ".repeat(disponible - exacte), style));
              }
            lignes.push(ligne);
          }
        else
          {
            lignes.push(ligne);
          }
      }
    lignes.push(Line::from(""));
    lignes.push(Line::from(if formulaire.palette.is_some()
      {
        "Flèches choisir · Entrée valider · Échap annuler"
      }
    else if formulaire.edition
      {
        "Entrée valider · Échap annuler · ←/→/Début/Fin / Ctrl+←/→ déplacer · Retour arrière/Suppr supprimer"
      }
    else
      {
        "↑/↓ choisir · Entrée éditer (couleur : palette) · F2 Enregistrer · Échap Quitter (enregistre)"
      }));

    let paragraphe = Paragraph::new(lignes).block(Block::default().borders(Borders::ALL).title(titre));
    frame.render_widget(paragraphe, zone);
    if formulaire.edition
      {
        let champ = formulaire.champs.get(formulaire.index).expect("index de sélection hors bornes");
        let prefixe = format!("{:>14}  ", champ.cle);
        let largeur_prefixe = UnicodeWidthStr::width(prefixe.as_str());
        let colonnes = UnicodeWidthStr::width(champ.valeur.as_str());
        let x = zone.x + 1 + u16::try_from(largeur_prefixe + colonnes).unwrap_or(u16::MAX);
        let y = zone.y + 1 + u16::try_from(formulaire.index).unwrap_or(u16::MAX);
        frame.set_cursor_position(Position::new(x.min(zone.x + zone.width - 2), y.min(zone.y + zone.height - 2)));
      }
    if let Some(palette) = formulaire.palette
      {
        let cle = formulaire.champs.get(formulaire.index).map(|c| c.cle).unwrap_or("COULEUR");
        dessiner_palette(frame, &palette, cle);
      }
  }

/// Dessine le sélecteur de couleur (grille de cases colorées, entrée « défaut »).
fn dessiner_palette(frame: &mut ratatui::Frame<'_>, etat: &PaletteEtat, cle: &str)
  {
    let total = PaletteEtat::total();
    let lignes_nb = total.div_ceil(PALETTE_COLONNES);
    let largeur = (PALETTE_COLONNES * (PALETTE_CASE + 1) + 3) as u16;
    let hauteur = (lignes_nb + 4) as u16;
    let aire = frame.area();
    let largeur = largeur.min(aire.width);
    let hauteur = hauteur.min(aire.height);
    let popup = Rect {
      x: aire.x + (aire.width.saturating_sub(largeur)) / 2,
      y: aire.y + (aire.height.saturating_sub(hauteur)) / 2,
      width: largeur,
      height: hauteur,
    };
    frame.render_widget(Clear, popup);

    let crochet = Style::default().add_modifier(Modifier::BOLD);
    let mut lignes: Vec<Line> = Vec::new();
    for ligne in 0..lignes_nb
      {
        let debut = ligne * PALETTE_COLONNES;
        let fin = ((ligne + 1) * PALETTE_COLONNES).min(total);
        let mut spans: Vec<Span> = Vec::new();
        for index in debut..fin
          {
            let separateur = if index == etat.index
              {
                "["
              }
            else if index == etat.index + 1
              {
                "]"
              }
            else
              {
                " "
              };
            spans.push(Span::styled(separateur, crochet));
            match if index == 0 { None } else { PALETTE.get(index - 1).copied() }
              {
                Some(c) => spans.push(Span::styled(
                  " ".repeat(PALETTE_CASE),
                  Style::default().bg(Color::Rgb(c.r, c.g, c.b)),
                )),
                None => spans.push(Span::styled(
                  format!("{:<width$}", "défaut", width = PALETTE_CASE),
                  Style::default(),
                )),
              }
          }
        if etat.index >= debut && etat.index == fin - 1
          {
            spans.push(Span::styled("]", crochet));
          }
        lignes.push(Line::from(spans));
      }
    lignes.push(Line::from(""));
    let apercu = match etat.entree()
      {
        Some(c) => format!("sélection : {}  ■", config::format_couleur(c)),
        None => "sélection : défaut du thème".to_string(),
      };
    lignes.push(Line::from(apercu));
    lignes.push(Line::from("flèches choisir · Entrée valider · Échap annuler"));

    let bloc = Paragraph::new(lignes)
      .block(Block::default().borders(Borders::ALL).title(format!("{cle} — couleur")));
    frame.render_widget(bloc, popup);
  }

/// Résumé textuel de la configuration active (`--reload-rc`).
pub fn resume(config: &Config) -> String
  {
    let mut lignes = Vec::new();
    lignes.push("Configuration active (~/.opshellrc)".to_string());
    lignes.push(format!("  THEME              : {}", config.theme));
    lignes.push(format!("  TRIGGERS           : {}", config.triggers.join(", ")));
    lignes.push(format!("  SUFFIX_WIDTH       : {}", config.suffix_width));
    lignes.push(format!("  CAPTURE            : {}", if config.capture { "on" } else { "off" }));
    lignes.push(format!("  NOTIFICATION       : {}", if config.notification { "on" } else { "off" }));
    lignes.push(format!("  NOTIFICATION_SEUIL : {}", config.seuil_notification));
    lignes.push(format!("  HORODATAGE         : {}", if config.horodatage { "on" } else { "off" }));
    lignes.push(format!("  LIGNE_VIDE         : {}", if config.ligne_vide { "on" } else { "off" }));
    for (cle, couleur) in [
      ("STAT_FG", config.couleurs.stat_fg),
      ("STAT_BG", config.couleurs.stat_bg),
      ("CONTEXT_FG", config.couleurs.contexte_fg),
      ("CONTEXT_BG", config.couleurs.contexte_bg),
      ("PROMPT_FG", config.couleurs.prompt_fg),
      ("PROMPT_BG", config.couleurs.prompt_bg),
    ]
      {
        let valeur = couleur.map(config::format_couleur).unwrap_or_else(|| "défaut".to_string());
        lignes.push(format!("  {cle:<18}: {valeur}"));
      }
    for (cle, valeur) in &config.inconnues
      {
        lignes.push(format!("  {cle:<18}: {valeur}  (clé inconnue, conservée)"));
      }
    lignes.join("\n")
  }

#[cfg(test)]
mod tests
  {
    use super::*;
    use crossterm::event::KeyEvent;

    fn touche(code: KeyCode) -> KeyEvent
      {
        KeyEvent::new(code, KeyModifiers::NONE)
      }

    #[test]
    fn le_formulaire_compte_les_cles_dans_l_ordre_de_la_spec()
      {
        let f = Formulaire::nouvelle(Config::default());
        let cles: Vec<&str> = f.champs.iter().map(|c| c.cle).collect();
        assert_eq!(
          cles,
          [
            "THEME", "TRIGGERS", "SUFFIX_WIDTH", "CAPTURE", "NOTIFICATION", "NOTIFICATION_SEUIL",
            "HORODATAGE", "LIGNE_VIDE", "STAT_FG", "STAT_BG", "CONTEXT_FG", "CONTEXT_BG",
            "PROMPT_FG", "PROMPT_BG"
          ]
        );
      }

    #[test]
    fn une_couleur_ouvre_la_palette_et_la_selection_est_appliquee()
      {
        let mut f = Formulaire::nouvelle(Config::default());
        for _ in 0..8
          {
            f.traiter(touche(KeyCode::Down));
          }
        assert_eq!(f.champs[f.index].cle, "STAT_FG");
        f.traiter(touche(KeyCode::Enter));
        assert!(f.palette.is_some(), "le champ couleur ouvre la palette");
        assert!(!f.edition, "la palette remplace l'édition de ligne");
        assert_eq!(f.palette.unwrap().index, 0, "on démarre sur « défaut »");
        f.traiter(touche(KeyCode::Right));
        let attendu = config::format_couleur(PALETTE[0]);
        assert_eq!(f.palette.unwrap().entree(), Some(PALETTE[0]));
        f.traiter(touche(KeyCode::Enter));
        assert!(f.palette.is_none());
        assert_eq!(f.champs[f.index].valeur, attendu);
        assert_eq!(f.vers_config().couleurs.stat_fg, Some(PALETTE[0]));
      }

    #[test]
    fn la_palette_se_navigue_aux_fleches_et_s_annule()
      {
        let mut f = Formulaire::nouvelle(Config::default());
        for _ in 0..8
          {
            f.traiter(touche(KeyCode::Down));
          }
        f.traiter(touche(KeyCode::Enter));
        f.traiter(touche(KeyCode::Down));
        assert_eq!(f.palette.unwrap().index, PALETTE_COLONNES, "descend d'une ligne");
        f.traiter(touche(KeyCode::Up));
        assert_eq!(f.palette.unwrap().index, 0, "remonte d'une ligne");
        f.traiter(touche(KeyCode::Left));
        assert_eq!(f.palette.unwrap().index, 0, "la première case ne déborde pas à gauche");
        f.traiter(touche(KeyCode::Esc));
        assert!(f.palette.is_none(), "Échap ferme la palette");
        assert_eq!(f.champs[f.index].valeur, "", "Échap n'altère pas la valeur");
      }

    #[test]
    fn une_couleur_hors_palette_retombe_sur_defaut()
      {
        let config = Config {
          couleurs: crate::theme::Surcharge { stat_fg: Some(Rgb::new(1, 2, 3)), ..crate::theme::Surcharge::default() },
          ..Config::default()
        };
        let mut f = Formulaire::nouvelle(config);
        for _ in 0..8
          {
            f.traiter(touche(KeyCode::Down));
          }
        assert_eq!(f.champs[f.index].valeur, "#010203");
        f.traiter(touche(KeyCode::Enter));
        assert_eq!(f.palette.unwrap().index, 0, "couleur hors palette → défaut");
        f.traiter(touche(KeyCode::Esc));
        assert_eq!(f.champs[f.index].valeur, "#010203", "l'annulation préserve la couleur");
      }

    #[test]
    fn une_couleur_de_la_palette_est_presélectionnée_et_le_defaut_restaure_le_theme()
      {
        let config = Config {
          couleurs: crate::theme::Surcharge { stat_fg: Some(PALETTE[3]), ..crate::theme::Surcharge::default() },
          ..Config::default()
        };
        let mut f = Formulaire::nouvelle(config);
        for _ in 0..8
          {
            f.traiter(touche(KeyCode::Down));
          }
        f.traiter(touche(KeyCode::Enter));
        assert_eq!(f.palette.unwrap().index, 4);
        while f.palette.unwrap().index != 0
          {
            f.traiter(touche(KeyCode::Up));
          }
        f.traiter(touche(KeyCode::Enter));
        assert_eq!(f.champs[f.index].valeur, "", "l'entrée défaut vide le champ");
        assert_eq!(f.vers_config().couleurs.stat_fg, None);
      }

    #[test]
    fn la_navigation_descend_et_remonte()
      {
        let mut f = Formulaire::nouvelle(Config::default());
        f.traiter(touche(KeyCode::Down));
        assert_eq!(f.index, 1);
        f.traiter(touche(KeyCode::Down));
        assert_eq!(f.index, 2);
        f.traiter(touche(KeyCode::Up));
        assert_eq!(f.index, 1);
        f.traiter(touche(KeyCode::Up));
        f.traiter(touche(KeyCode::Up));
        assert_eq!(f.index, 0, "la première ligne ne déborde pas vers le haut");
      }

    #[test]
    fn entrepasse_en_edition_puis_valide_au_champ_suivant()
      {
        let mut f = Formulaire::nouvelle(Config::default());
        f.traiter(touche(KeyCode::Enter));
        assert!(f.edition);
        f.traiter(touche(KeyCode::Enter));
        assert!(!f.edition);
        assert_eq!(f.index, 1);
      }

    #[test]
    fn l_edition_insere_retourne_et_annule()
      {
        let mut f = Formulaire::nouvelle(Config::default());
        f.traiter(touche(KeyCode::Enter));
        f.traiter(touche(KeyCode::Char('x')));
        assert_eq!(f.champs[0].valeur, "autox");
        f.traiter(touche(KeyCode::Backspace));
        assert_eq!(f.champs[0].valeur, "auto");
        f.traiter(touche(KeyCode::Char('z')));
        assert_eq!(f.champs[0].valeur, "autoz");
        f.traiter(touche(KeyCode::Esc));
        assert!(!f.edition);
        assert_eq!(f.champs[0].valeur, "auto", "Échap annule l'édition du champ");
      }

    #[test]
    fn le_ctrl_fleche_traduit_un_saut_de_mot()
      {
        let mut f = Formulaire::nouvelle(Config::default());
        f.traiter(touche(KeyCode::Down));
        f.traiter(touche(KeyCode::Enter));
        assert_eq!(f.champs[1].valeur, "sudo, doas, su, sudoedit");
        let ctrl_droite = KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL);
        f.traiter(ctrl_droite);
        assert_eq!(f.champs[1].curseur, 24, "Ctrl+→ depuis la fin reste à la fin");
        let ctrl_gauche = KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL);
        f.traiter(ctrl_gauche);
        assert_eq!(f.champs[1].curseur, 16, "Ctrl+← saute au début du mot précédent");
      }

    #[test]
    fn les_valeurs_on_off_sont_converties_en_boolens()
      {
        assert!(est_active("on"));
        assert!(!est_active("off"));
        assert!(!est_active("OFF"));
        assert!(est_active("true"));
      }

    #[test]
    fn la_configuration_est_reconstruite_sans_perte_des_inconnues()
      {
        let c = Config {
          capture: false,
          horodatage: true,
          suffix_width: 8,
          inconnues: [("COULEUR_PERSO".to_string(), "#112233".to_string())].into_iter().collect(),
          ..Config::default()
        };
        let f = Formulaire::nouvelle(c);
        let reconstitue = f.vers_config();
        assert!(!reconstitue.capture);
        assert!(reconstitue.horodatage);
        assert_eq!(reconstitue.suffix_width, 8);
        assert_eq!(reconstitue.inconnues.get("COULEUR_PERSO").map(String::as_str), Some("#112233"));
      }

    #[test]
    fn un_suffix_ou_seuil_invalide_revient_a_la_valeur_par_defaut()
      {
        let mut f = Formulaire::nouvelle(Config::default());
        f.traiter(touche(KeyCode::Down));
        f.traiter(touche(KeyCode::Down));
        f.traiter(touche(KeyCode::Enter));
        f.champs[2].valeur = "abc".to_string();
        assert_eq!(f.vers_config().suffix_width, crate::config::SUFFIX_WIDTH_DEFAUT);
      }

    #[test]
    fn le_resume_relie_les_cles_et_signale_les_inconnues()
      {
        let mut c = Config::default();
        c.inconnues.insert("COULEUR_PERSO".to_string(), "#112233".to_string());
        let texte = resume(&c);
        assert!(texte.contains("THEME"));
        assert!(texte.contains("CAPTURE"));
        assert!(texte.contains("clé inconnue, conservée"));
      }
  }