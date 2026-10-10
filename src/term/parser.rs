//! Implémentation du parseur VT (`vte`) pour [`Terminal`].
//!
//! Le parseur décompose le flux du shell en caractères, contrôles, séquences
//! CSI/ESC et messages OSC. `OSC 133` alimente l'état du shell (prompt,
//! commande, sortie, code retour) ; `OSC 0/2` alimente le titre.

use vte::{Params, Perform};

use super::grid::Couleur;
use super::{EtatShell, Terminal};

/// Lit le n-ième paramètre, en repliant sur `defaut` s'il est absent ou nul.
fn parametre(params: &Params, n: usize, defaut: u16, zero_autorise: bool) -> u16
  {
    params
      .iter()
      .nth(n)
      .and_then(|sous| sous.first().copied())
      .map(|v| if v == 0 && !zero_autorise { defaut } else { v })
      .unwrap_or(defaut)
  }

impl Terminal
  {
    fn sgr(&mut self, params: &Params)
      {
        let plats: Vec<u16> = params.iter().flat_map(|s| s.iter().copied()).collect();
        let mut i = 0;
        while i < plats.len()
          {
            match plats[i]
              {
                0 => self.reinitialiser_attributs(),
                1 => self.gras = true,
                3 => self.italique = true,
                5 => self.clignote = true,
                7 => self.inverse = true,
                22 => self.gras = false,
                23 => self.italique = false,
                25 => self.clignote = false,
                27 => self.inverse = false,
                30..=37 => self.avant = Couleur::Index((plats[i] - 30) as u8),
                39 => self.avant = Couleur::Defaut,
                40..=47 => self.arriere = Couleur::Index((plats[i] - 40) as u8),
                49 => self.arriere = Couleur::Defaut,
                90..=97 => self.avant = Couleur::Index((plats[i] - 90 + 8) as u8),
                100..=107 => self.arriere = Couleur::Index((plats[i] - 100 + 8) as u8),
                38 | 48 => {
                  let cible_avant = plats[i] == 38;
                  if plats.get(i + 1) == Some(&5)
                    {
                      let couleur = Couleur::Index(plats.get(i + 2).copied().unwrap_or(0) as u8);
                      if cible_avant { self.avant = couleur } else { self.arriere = couleur }
                      i += 2;
                    }
                  else if plats.get(i + 1) == Some(&2)
                    {
                      let couleur = Couleur::Rgb(
                        plats.get(i + 2).copied().unwrap_or(0) as u8,
                        plats.get(i + 3).copied().unwrap_or(0) as u8,
                        plats.get(i + 4).copied().unwrap_or(0) as u8,
                      );
                      if cible_avant { self.avant = couleur } else { self.arriere = couleur }
                      i += 4;
                    }
                }
                _ => {}
              }
            i += 1;
          }
      }

    fn mode(&mut self, params: &Params, prive: bool, actif: bool)
      {
        for valeur in params.iter().filter_map(|s| s.first().copied())
          {
            if !prive
              {
                if valeur == 7
                  {
                    self.autowrap = actif;
                  }
                continue;
              }
            match valeur
              {
                25 => self.curseur_visible = actif,
                1049 => {
                  let modele = self.cellule_effacement();
                  self.grid.effacer_ecran(modele);
                  self.grid.curseur_lig = 0;
                  self.grid.curseur_col = 0;
                }
                _ => {}
              }
          }
      }

    fn osc_133(&mut self, params: &[&[u8]])
      {
        let action = params.get(1).and_then(|p| p.first()).copied();
        match action
          {
            Some(b'B') => {
              self.etat = EtatShell::Prompt;
              self.col_commande = self.grid.curseur_col;
            }
            Some(b'C') => {
              self.etat = EtatShell::Commande;
            }
            Some(b'D') => {
              self.dernier_statut = params
                .get(2)
                .and_then(|p| std::str::from_utf8(p).ok())
                .and_then(|s| s.trim().parse::<i32>().ok());
            }
            _ => {}
          }
      }
  }

impl Perform for Terminal
  {
    fn print(&mut self, c: char)
      {
        self.imprimer(c);
      }

    fn execute(&mut self, byte: u8)
      {
        match byte
          {
            b'\n' | 0x0b | 0x0c => self.nouvelle_ligne(),
            b'\r' => self.grid.curseur_col = 0,
            b'\t' => {
              let suivant = (self.grid.curseur_col + 8) / 8 * 8;
              self.grid.curseur_col = suivant.min(self.grid.largeur.saturating_sub(1));
            }
            0x08 => self.bouger_col(-1),
            0x07 => {}
            _ => {}
          }
      }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char)
      {
        let prive = intermediates.contains(&b'?');
        match action
          {
            'A' => self.bouger_lig(-(parametre(params, 0, 1, false) as i32)),
            'B' => self.bouger_lig(parametre(params, 0, 1, false) as i32),
            'C' => self.bouger_col(parametre(params, 0, 1, false) as i32),
            'D' => self.bouger_col(-(parametre(params, 0, 1, false) as i32)),
            'G' => self.grid.curseur_col = (parametre(params, 0, 1, false) - 1).min(self.grid.largeur.saturating_sub(1)),
            'd' => self.grid.curseur_lig = (parametre(params, 0, 1, false) - 1).min(self.grid.hauteur.saturating_sub(1)),
            'H' | 'f' => {
              self.grid.curseur_lig = (parametre(params, 0, 1, false) - 1).min(self.grid.hauteur.saturating_sub(1));
              self.grid.curseur_col = (parametre(params, 1, 1, false) - 1).min(self.grid.largeur.saturating_sub(1));
            }
            'J' => self.effacer_ecran_mode(parametre(params, 0, 0, true)),
            'K' => self.effacer_ligne_mode(parametre(params, 0, 0, true)),
            'm' => self.sgr(params),
            'h' => self.mode(params, prive, true),
            'l' => self.mode(params, prive, false),
            'X' => {
              let n = parametre(params, 0, 1, false);
              let lig = self.grid.curseur_lig;
              let modele = self.cellule_effacement();
              for col in self.grid.curseur_col..(self.grid.curseur_col + n).min(self.grid.largeur)
                {
                  self.grid.poser(lig, col, modele);
                }
            }
            _ => {}
          }
      }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8)
      {
        match byte
          {
            b'D' => self.index(),
            b'M' => self.index_reverse(),
            b'E' => {
              self.grid.curseur_col = 0;
              self.nouvelle_ligne();
            }
            b'7' => self.sauver_curseur(),
            b'8' => self.restaurer_curseur(),
            b'c' => {
              let modele = self.cellule_effacement();
              self.grid.effacer_ecran(modele);
              self.grid.curseur_lig = 0;
              self.grid.curseur_col = 0;
              self.reinitialiser_attributs();
            }
            _ => {}
          }
      }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell: bool)
      {
        match params.first().and_then(|p| std::str::from_utf8(p).ok())
          {
            Some("0") | Some("2") => {
              if let Some(titre) = params.get(1).and_then(|p| std::str::from_utf8(p).ok())
                {
                  self.titre = titre.to_string();
                }
            }
            Some("133") => self.osc_133(params),
            _ => {}
          }
      }
  }
