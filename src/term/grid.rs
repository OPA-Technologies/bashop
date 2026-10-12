//! Grille de cellules de la zone centrale émulée.
//!
//! Une cellule porte un caractère et son style complet. Les caractères de
//! largeur 2 occupent deux cellules : la première porte le caractère, la
//! seconde est marquée `continuation` et n'émet rien au rendu, ce qui garde la
//! largeur d'affichage correcte.

use std::collections::VecDeque;

/// Nombre maximal de lignes conservées hors écran (historique défilant).
pub const CAPACITE_ARRIERE_PLAN: usize = 10_000;

/// Couleur d'une cellule, telle que produite par les séquences SGR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Couleur
  {
    /// Couleur par défaut du terminal.
    Defaut,
    /// Couleur de la palette (0–255).
    Index(u8),
    /// Couleur 24 bits.
    Rgb(u8, u8, u8),
  }

/// Cellule d'affichage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cellule
  {
    /// Caractère affiché.
    pub c: char,
    /// Couleur de l'avant-plan.
    pub avant: Couleur,
    /// Couleur de l'arrière-plan.
    pub arriere: Couleur,
    /// Gras (SGR 1).
    pub gras: bool,
    /// Italique (SGR 3).
    pub italique: bool,
    /// Clignotement (SGR 5).
    pub clignote: bool,
    /// Vidéo inverse (SGR 7).
    pub inverse: bool,
    /// Seconde moitié d'un caractère de largeur 2 (n'émet rien).
    pub continuation: bool,
  }

impl Default for Cellule
  {
    fn default() -> Self
      {
        Self {
          c: ' ',
          avant: Couleur::Defaut,
          arriere: Couleur::Defaut,
          gras: false,
          italique: false,
          clignote: false,
          inverse: false,
          continuation: false,
        }
      }
  }

/// Grille de la zone centrale.
#[derive(Clone, Debug)]
pub struct Grid
  {
    /// Largeur en colonnes.
    pub largeur: u16,
    /// Hauteur en lignes.
    pub hauteur: u16,
    /// Ligne du curseur (0-indexée).
    pub curseur_lig: u16,
    /// Colonne du curseur (0-indexée).
    pub curseur_col: u16,
    cellules: Vec<Cellule>,
    arriere_plan: VecDeque<Vec<Cellule>>,
  }

impl Grid
  {
    /// Crée une grille vierge aux dimensions données.
    pub fn nouvelle(largeur: u16, hauteur: u16) -> Self
      {
        let largeur = largeur.max(1);
        let hauteur = hauteur.max(1);
        Self {
          largeur,
          hauteur,
          curseur_lig: 0,
          curseur_col: 0,
          cellules: vec![Cellule::default(); largeur as usize * hauteur as usize],
          arriere_plan: VecDeque::new(),
        }
      }

    fn indice(&self, lig: u16, col: u16) -> usize
      {
        lig as usize * self.largeur as usize + col as usize
      }

    /// Lit une cellule (espace par défaut hors grille).
    pub fn cellule(&self, lig: u16, col: u16) -> Cellule
      {
        if lig >= self.hauteur || col >= self.largeur
          {
            return Cellule::default();
          }
        self.cellules[self.indice(lig, col)]
      }

    /// Écrit une cellule (sans effet hors grille).
    pub fn poser(&mut self, lig: u16, col: u16, cellule: Cellule)
      {
        if lig < self.hauteur && col < self.largeur
          {
            let i = self.indice(lig, col);
            self.cellules[i] = cellule;
          }
      }

    /// Efface une ligne à partir d'une colonne, avec le style d'effacement donné.
    pub fn effacer_ligne(&mut self, lig: u16, depuis: u16, modele: Cellule)
      {
        if lig >= self.hauteur
          {
            return;
          }
        for col in depuis..self.largeur
          {
            self.poser(lig, col, modele);
          }
      }

    /// Efface une ligne jusqu'à une colonne incluse, avec le style donné.
    pub fn effacer_ligne_jusqua(&mut self, lig: u16, jusqua: u16, modele: Cellule)
      {
        if lig >= self.hauteur
          {
            return;
          }
        for col in 0..=jusqua.min(self.largeur.saturating_sub(1))
          {
            self.poser(lig, col, modele);
          }
      }

    /// Supprime `n` caractères à partir d'une colonne : la fin de ligne se
    /// décale vers la gauche et les dernières cellules deviennent vierges
    /// (DCH, `CSI P`). Utilisé par readline pour raccourcir la ligne.
    pub fn supprimer_caracteres(&mut self, lig: u16, depuis: u16, n: u16, modele: Cellule)
      {
        if lig >= self.hauteur || depuis >= self.largeur
          {
            return;
          }
        let n = n.min(self.largeur - depuis);
        for col in depuis..self.largeur
          {
            let source = col + n;
            let cellule = if source < self.largeur { self.cellule(lig, source) } else { modele };
            self.poser(lig, col, cellule);
          }
      }

    /// Insère `n` caractères vierges à partir d'une colonne : la fin de ligne
    /// se décale vers la droite, les débordements sont perdus (ICH, `CSI @`).
    pub fn inserer_caracteres(&mut self, lig: u16, depuis: u16, n: u16, modele: Cellule)
      {
        if lig >= self.hauteur || depuis >= self.largeur
          {
            return;
          }
        let n = n.min(self.largeur - depuis);
        for col in (depuis..self.largeur).rev()
          {
            let cellule = if col >= depuis + n { self.cellule(lig, col - n) } else { modele };
            self.poser(lig, col, cellule);
          }
      }

    /// Efface toute la grille.
    pub fn effacer_ecran(&mut self, modele: Cellule)
      {
        self.cellules.fill(modele);
      }

    /// Nombre de lignes conservées dans l'historique défilant.
    pub fn arriere_plan_len(&self) -> usize
      {
        self.arriere_plan.len()
      }

    /// Ligne de l'historique défilant (0 = la plus ancienne conservée).
    pub fn arriere_plan(&self, lig: usize) -> Option<&[Cellule]>
      {
        self.arriere_plan.get(lig).map(|l| l.as_slice())
      }

    /// Texte d'une ligne, sans les cellules de continuation.
    pub fn texte_ligne(&self, lig: u16) -> String
      {
        if lig >= self.hauteur
          {
            return String::new();
          }
        let mut texte = String::new();
        for col in 0..self.largeur
          {
            let cellule = self.cellule(lig, col);
            if !cellule.continuation
              {
                texte.push(cellule.c);
              }
          }
        texte
      }

    /// Fait défiler la grille d'une ligne vers le haut.
    pub fn defiler_haut(&mut self)
      {
        let largeur = self.largeur as usize;
        let premiere: Vec<Cellule> = self.cellules[0..largeur].to_vec();
        self.arriere_plan.push_back(premiere);
        while self.arriere_plan.len() > CAPACITE_ARRIERE_PLAN
          {
            self.arriere_plan.pop_front();
          }
        self.cellules.drain(0..largeur);
        self.cellules.resize(largeur * self.hauteur as usize, Cellule::default());
      }

    /// Fait défiler la grille d'une ligne vers le bas.
    pub fn defiler_bas(&mut self)
      {
        let largeur = self.largeur as usize;
        for _ in 0..largeur
          {
            self.cellules.insert(0, Cellule::default());
          }
        self.cellules.truncate(largeur * self.hauteur as usize);
      }

    /// Redimensionne en conservant le contenu recouvrant (coin haut-gauche).
    pub fn redimensionner(&mut self, largeur: u16, hauteur: u16)
      {
        let largeur = largeur.max(1);
        let hauteur = hauteur.max(1);
        if largeur == self.largeur && hauteur == self.hauteur
          {
            return;
          }
        let mut nouvelle = Grid::nouvelle(largeur, hauteur);
        for lig in 0..hauteur.min(self.hauteur)
          {
            for col in 0..largeur.min(self.largeur)
              {
                nouvelle.poser(lig, col, self.cellule(lig, col));
              }
          }
        nouvelle.curseur_lig = self.curseur_lig.min(hauteur - 1);
        nouvelle.curseur_col = self.curseur_col.min(largeur - 1);
        nouvelle.arriere_plan = std::mem::take(&mut self.arriere_plan);
        *self = nouvelle;
      }
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn une_grille_neuve_est_vide()
      {
        let grille = Grid::nouvelle(10, 3);
        assert_eq!(grille.texte_ligne(0), " ".repeat(10));
      }

    #[test]
    fn poser_puis_lire_rend_le_caractere()
      {
        let mut grille = Grid::nouvelle(5, 2);
        grille.poser(0, 1, Cellule { c: 'A', ..Cellule::default() });
        assert_eq!(grille.texte_ligne(0), " A   ");
      }

    #[test]
    fn le_defilement_remonte_les_lignes()
      {
        let mut grille = Grid::nouvelle(3, 2);
        grille.poser(1, 0, Cellule { c: 'X', ..Cellule::default() });
        grille.defiler_haut();
        assert_eq!(grille.texte_ligne(0), "X  ");
      }

    #[test]
    fn la_ligne_evincee_est_conservee_en_arriere_plan()
      {
        let mut grille = Grid::nouvelle(3, 2);
        grille.poser(0, 0, Cellule { c: 'A', ..Cellule::default() });
        grille.defiler_haut();
        assert_eq!(grille.arriere_plan_len(), 1);
        assert_eq!(grille.arriere_plan(0).and_then(|l| l.first()).map(|c| c.c), Some('A'));
      }

    #[test]
    fn la_suppression_decale_la_fin_de_ligne_vers_la_gauche()
      {
        let mut grille = Grid::nouvelle(6, 1);
        for (col, c) in "ABCDEF".chars().enumerate()
          {
            grille.poser(0, col as u16, Cellule { c, ..Cellule::default() });
          }
        grille.supprimer_caracteres(0, 1, 2, Cellule::default());
        assert_eq!(grille.texte_ligne(0), "ADEF  ");
      }

    #[test]
    fn l_insertion_decale_la_fin_de_ligne_vers_la_droite()
      {
        let mut grille = Grid::nouvelle(6, 1);
        for (col, c) in "ABCDEF".chars().enumerate()
          {
            grille.poser(0, col as u16, Cellule { c, ..Cellule::default() });
          }
        grille.inserer_caracteres(0, 1, 2, Cellule::default());
        assert_eq!(grille.texte_ligne(0), "A  BCD");
      }

    #[test]
    fn le_redimensionnement_conserve_le_coin()
      {
        let mut grille = Grid::nouvelle(4, 2);
        grille.poser(0, 0, Cellule { c: 'A', ..Cellule::default() });
        grille.redimensionner(6, 3);
        assert_eq!(grille.largeur, 6);
        assert_eq!(grille.texte_ligne(0), "A     ");
      }
  }
