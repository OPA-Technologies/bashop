//! Émulation de la zone centrale : grille, curseur, interprétation des
//! marqueurs de shell (`OSC 133`).
//!
//! `Terminal` est piloté par le parseur VT (`parser.rs`) qui implémente
//! `vte::Perform`. L'application détient le `vte::Parser` et l'avance sur
//! `Terminal`, ce qui permet à l'emprunt des deux champs d'être disjoint.

pub mod grid;
pub mod overlay;
pub mod parser;

use grid::{Cellule, Couleur, Grid};

pub use crate::detect::Privilege;
pub use crate::observe::EtatShell;

/// Protocole de suivi souris activé par le programme enfant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourisMode
  {
    /// Aucun suivi (molette = historique).
    Aucune,
    /// X10 (DECSET 9).
    X10,
    /// Boutons (DECSET 1000).
    Bouton,
    /// Boutons + déplacement (DECSET 1002).
    BoutonMouvement,
    /// Tout déplacement (DECSET 1003).
    Mouvement,
  }

/// Sauvegarde d'écran pour l'écran alterné (DECSET 1049).
#[derive(Clone, Debug)]
struct EcranSauve
  {
    grid: Grid,
    curseur_visible: bool,
  }

/// Émulateur de la zone centrale.
#[derive(Debug)]
pub struct Terminal
  {
    /// Grille affichable.
    pub grid: Grid,
    /// Curseur visible (mode DECSET 25).
    pub curseur_visible: bool,
    /// Avant-plan courant.
    pub avant: Couleur,
    /// Arrière-plan courant.
    pub arriere: Couleur,
    /// Gras courant.
    pub gras: bool,
    /// Italique courant.
    pub italique: bool,
    /// Clignotement courant.
    pub clignote: bool,
    /// Vidéo inverse courante.
    pub inverse: bool,
    /// Retour automatique à la ligne (DECAWM).
    pub autowrap: bool,
    /// État du shell déduit des marqueurs OSC 133.
    pub etat: EtatShell,
    /// Colonne où débute la commande en cours (marqueur OSC 133;B).
    pub col_commande: u16,
    /// Dernier code retour annoncé (OSC 133;D), si connu.
    pub dernier_statut: Option<i32>,
    /// Texte de la commande en cours d'exécution (écho à ignorer).
    pub commande_lancee: String,
    /// Première ligne de sortie non vide de la commande en cours.
    premiere_sortie: Option<String>,
    /// Copie de la première ligne d'erreur (code retour non nul).
    pub derniere_erreur: Option<String>,
    /// Titre de fenêtre demandé par l'enfant (OSC 0/2), si connu.
    pub titre: String,
    /// Identité publiée par le shell (OSC privé `1337;Bashop;user=…`).
    pub utilisateur: Option<String>,
    /// Privilège publié par le shell (OSC privé `1337;Bashop;priv=…`).
    pub privilege: Option<Privilege>,
    /// Protocole de suivi souris actif.
    pub souris: SourisMode,
    /// Encodage souris SGR (DECSET 1006).
    pub souris_sgr: bool,
    /// Encodage souris urxvt (DECSET 1015).
    pub souris_utf8: bool,
    /// Collage encadré actif (DECSET 2004).
    pub paste_bracketed: bool,
    ecran_alterne: Option<EcranSauve>,
    sauvegarde: Option<(u16, u16)>,
  }

impl Terminal
  {
    /// Crée un terminal vierge.
    pub fn nouveau(largeur: u16, hauteur: u16) -> Self
      {
        Self {
          grid: Grid::nouvelle(largeur, hauteur),
          curseur_visible: true,
          avant: Couleur::Defaut,
          arriere: Couleur::Defaut,
          gras: false,
          italique: false,
          clignote: false,
          inverse: false,
          autowrap: true,
          etat: EtatShell::Inconnu,
          col_commande: 0,
          dernier_statut: None,
          commande_lancee: String::new(),
          premiere_sortie: None,
          derniere_erreur: None,
          titre: String::new(),
          utilisateur: None,
          privilege: None,
          souris: SourisMode::Aucune,
          souris_sgr: false,
          souris_utf8: false,
          paste_bracketed: false,
          ecran_alterne: None,
          sauvegarde: None,
        }
      }

    /// Avance le parseur sur un lot d'octets.
    pub fn alimenter(&mut self, parser: &mut vte::Parser, octets: &[u8])
      {
        parser.advance(self, octets);
      }

    /// Redimensionne la grille.
    pub fn redimensionner(&mut self, largeur: u16, hauteur: u16)
      {
        self.grid.redimensionner(largeur, hauteur);
        self.col_commande = self.col_commande.min(largeur.saturating_sub(1));
      }

    /// Cellule au style courant.
    pub(crate) fn cellule_courante(&self, c: char) -> Cellule
      {
        Cellule {
          c,
          avant: self.avant,
          arriere: self.arriere,
          gras: self.gras,
          italique: self.italique,
          clignote: self.clignote,
          inverse: self.inverse,
          continuation: false,
        }
      }

    /// Cellule servant à l'effacement (fond courant, avant-plan par défaut).
    pub(crate) fn cellule_effacement(&self) -> Cellule
      {
        Cellule {
          arriere: self.arriere,
          ..Cellule::default()
        }
      }

    /// Réinitialise les attributs SGR.
    pub(crate) fn reinitialiser_attributs(&mut self)
      {
        self.avant = Couleur::Defaut;
        self.arriere = Couleur::Defaut;
        self.gras = false;
        self.italique = false;
        self.clignote = false;
        self.inverse = false;
      }

    /// Imprime un caractère à la position du curseur.
    pub(crate) fn imprimer(&mut self, c: char)
      {
        let largeur = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) as u16;
        if largeur == 0
          {
            return;
          }
        if self.grid.curseur_col + largeur > self.grid.largeur
          {
            if self.autowrap
              {
                self.grid.curseur_col = 0;
                self.nouvelle_ligne();
              }
            else
              {
                self.grid.curseur_col = self.grid.largeur.saturating_sub(largeur);
              }
          }
        let lig = self.grid.curseur_lig;
        let col = self.grid.curseur_col;
        self.grid.poser(lig, col, self.cellule_courante(c));
        if largeur == 2 && col + 1 < self.grid.largeur
          {
            self.grid.poser(lig, col + 1, Cellule { continuation: true, ..Cellule::default() });
          }
        self.grid.curseur_col = (self.grid.curseur_col + largeur).min(self.grid.largeur);
      }

    /// Retour chariot puis saut de ligne (comportement d'un `\n` en sortie).
    pub(crate) fn nouvelle_ligne(&mut self)
      {
        let lig = self.grid.curseur_lig;
        self.capturer_sortie(lig);
        if lig + 1 >= self.grid.hauteur
          {
            self.grid.defiler_haut();
          }
        else
          {
            self.grid.curseur_lig += 1;
          }
      }

    /// Retient la **première** ligne de sortie non vide d'une commande en cours,
    /// en ignorant l'écho de la commande elle-même. Pure observation : aucune
    /// donnée du shell n'est altérée (redirections, `tee`, code retour intacts).
    fn capturer_sortie(&mut self, lig: u16)
      {
        if self.premiere_sortie.is_some() || self.etat != EtatShell::Commande
          {
            return;
          }
        let brut = self.grid.texte_ligne(lig);
        let complet = brut.trim();
        let debut = (self.col_commande as usize).min(brut.chars().count());
        let queue: String = brut.chars().skip(debut).collect();
        let queue = queue.trim();
        if complet.is_empty() || complet == self.commande_lancee.trim()
          || queue == self.commande_lancee.trim()
          {
            return;
          }
        let borne: String = complet.chars().take(120).collect();
        self.premiere_sortie = Some(borne);
      }

    /// Efface la copie d'erreur affichée par le cadre.
    pub fn effacer_erreur(&mut self)
      {
        self.derniere_erreur = None;
      }

    /// Index (ESC D) : ligne suivante, sans retour chariot.
    pub(crate) fn index(&mut self)
      {
        self.nouvelle_ligne();
      }

    /// Reverse index (ESC M) : ligne précédente, ou défilement vers le bas.
    pub(crate) fn index_reverse(&mut self)
      {
        if self.grid.curseur_lig == 0
          {
            self.grid.defiler_bas();
          }
        else
          {
            self.grid.curseur_lig -= 1;
          }
      }

    /// Sauvegarde la position du curseur (ESC 7).
    pub(crate) fn sauver_curseur(&mut self)
      {
        self.sauvegarde = Some((self.grid.curseur_lig, self.grid.curseur_col));
      }

    /// Restaure la position du curseur (ESC 8).
    pub(crate) fn restaurer_curseur(&mut self)
      {
        if let Some((lig, col)) = self.sauvegarde
          {
            self.grid.curseur_lig = lig.min(self.grid.hauteur.saturating_sub(1));
            self.grid.curseur_col = col.min(self.grid.largeur.saturating_sub(1));
          }
      }

    /// Ajuste la ligne du curseur d'un décalage relatif.
    pub(crate) fn bouger_lig(&mut self, delta: i32)
      {
        let cible = self.grid.curseur_lig as i32 + delta;
        self.grid.curseur_lig = cible.clamp(0, self.grid.hauteur as i32 - 1) as u16;
      }

    /// Ajuste la colonne du curseur d'un décalage relatif.
    pub(crate) fn bouger_col(&mut self, delta: i32)
      {
        let cible = self.grid.curseur_col as i32 + delta;
        self.grid.curseur_col = cible.clamp(0, self.grid.largeur as i32 - 1) as u16;
      }

    /// Efface l'écran selon le mode ED (0, 1, 2).
    pub(crate) fn effacer_ecran_mode(&mut self, mode: u16)
      {
        let lig = self.grid.curseur_lig;
        let col = self.grid.curseur_col;
        let modele = self.cellule_effacement();
        match mode
          {
            0 => {
              self.grid.effacer_ligne(lig, col, modele);
              for l in (lig + 1)..self.grid.hauteur
                {
                  self.grid.effacer_ligne(l, 0, modele);
                }
            }
            1 => {
              self.grid.effacer_ligne_jusqua(lig, col, modele);
              for l in 0..lig
                {
                  self.grid.effacer_ligne(l, 0, modele);
                }
            }
            _ => self.grid.effacer_ecran(modele),
          }
      }

    /// Efface la ligne selon le mode EL (0, 1, 2).
    pub(crate) fn effacer_ligne_mode(&mut self, mode: u16)
      {
        let lig = self.grid.curseur_lig;
        let col = self.grid.curseur_col;
        let modele = self.cellule_effacement();
        match mode
          {
            0 => self.grid.effacer_ligne(lig, col, modele),
            1 => self.grid.effacer_ligne_jusqua(lig, col, modele),
            _ => self.grid.effacer_ligne(lig, 0, modele),
          }
      }

    /// Bascule l'écran alterné : sauvegarde à l'entrée, restaure à la sortie.
    pub(crate) fn ecran_alterne(&mut self, actif: bool)
      {
        if actif
          {
            if self.ecran_alterne.is_none()
              {
                self.ecran_alterne = Some(EcranSauve {
                  grid: self.grid.clone(),
                  curseur_visible: self.curseur_visible,
                });
              }
            let modele = self.cellule_effacement();
            self.grid.effacer_ecran(modele);
            self.grid.curseur_lig = 0;
            self.grid.curseur_col = 0;
          }
        else if let Some(sauve) = self.ecran_alterne.take()
          {
            self.grid = sauve.grid;
            self.curseur_visible = sauve.curseur_visible;
          }
      }

    /// Indique si le programme enfant a activé un suivi souris.
    pub fn suivi_souris(&self) -> bool
      {
        self.souris != SourisMode::Aucune
      }

    /// Extrait la commande en cours de saisie (après OSC 133;B).
    pub fn commande_courante(&self) -> String
      {
        let texte = self.grid.texte_ligne(self.grid.curseur_lig);
        let debut = (self.col_commande as usize).min(texte.chars().count());
        texte.chars().skip(debut).collect::<String>().trim().to_string()
      }
  }
