//! Interface de configuration — `bashop config`.
//!
//! Formulaire éditable des sept clés reconnues de `~/.opshellrc`, construit
//! sur les primitives de `op-tui` (édition de ligne [`op_tui::edit::gerer`]) et
//! rendu par ratatui. Chaque champ est validé à la sortie ; les clés inconnues
//! du fichier sont conservées telles quelles.
//!
//! Contrats :
//! - `Échap` en liste quitte **et enregistre** ; `F2` enregistre aussi.
//! - `Échap` dans un champ annule l'édition de ce champ ; `Entrée` la valide
//!   et passe au champ suivant.
//! - `bashop config --reload-rc` n'ouvre pas l'interface : il relit le fichier
//!   et affiche le résumé de la configuration active.

use crate::config::Config;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
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
        let champs = vec![
          champ("THEME", &config.theme),
          champ("TRIGGERS", &config.triggers.join(", ")),
          champ("SUFFIX_WIDTH", &config.suffix_width.to_string()),
          champ("CAPTURE", &bouton(config.capture)),
          champ("NOTIFICATION", &bouton(config.notification)),
          champ("NOTIFICATION_SEUIL", &config.seuil_notification.to_string()),
          champ("HORODATAGE", &bouton(config.horodatage)),
        ];
        Self {
          champs,
          index: 0,
          edition: false,
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
                _ => {}
              }
          }
        c
      }

    /// Traite une touche. Retourne `true` quand il faut quitter (et enregistrer).
    pub fn traiter(&mut self, touche: KeyEvent) -> bool
      {
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
            KeyCode::Enter | KeyCode::Tab => self.entre_edition(),
            KeyCode::F(2) | KeyCode::Esc => return true,
            _ => {}
          }
        false
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
        let disponible = usize::from(zone.width).saturating_sub(largeur_prefixe).saturating_sub(3);
        let valeur = tronquer(&champ.valeur, disponible);
        let surligne = i == formulaire.index;
        if surligne && formulaire.edition
          {
            let style = Style::default().fg(Color::Black).bg(Color::Cyan);
            lignes.push(Line::from(vec![
              Span::styled(prefixe, style),
              Span::styled(valeur, style),
            ]));
          }
        else if surligne
          {
            let style = Style::default().add_modifier(Modifier::BOLD);
            lignes.push(Line::from(vec![
              Span::styled(prefixe, style),
              Span::styled(valeur, style),
            ]));
          }
        else
          {
            lignes.push(Line::from(prefixe + &valeur));
          }
      }
    lignes.push(Line::from(""));
    lignes.push(Line::from(if formulaire.edition
      {
        "Entrée valider · Échap annuler · ←/→/Début/Fin / Ctrl+←/→ déplacer · Retour arrière/Suppr supprimer"
      }
    else
      {
        "↑/↓ choisir · Entrée éditer · F2 Enregistrer · Échap Quitter (enregistre)"
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
    fn le_formulaire_compte_les_sept_cles_dans_l_ordre_de_la_spec()
      {
        let f = Formulaire::nouvelle(Config::default());
        let cles: Vec<&str> = f.champs.iter().map(|c| c.cle).collect();
        assert_eq!(cles, ["THEME", "TRIGGERS", "SUFFIX_WIDTH", "CAPTURE", "NOTIFICATION", "NOTIFICATION_SEUIL", "HORODATAGE"]);
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