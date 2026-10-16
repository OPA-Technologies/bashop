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

    /// Lit une cellule par index absolu « source » : d'abord l'historique
    /// défilant (`0..arriere_plan_len`), puis la grille visible. Hors bornes,
    /// renvoie une cellule vide.
    pub fn cellule_source(&self, source: usize, col: u16) -> Cellule
      {
        if col >= self.largeur
          {
            return Cellule::default();
          }
        let arriere = self.arriere_plan.len();
        if source < arriere
          {
            self.arriere_plan[source].get(col as usize).copied().unwrap_or_default()
          }
        else
          {
            self.cellule((source - arriere) as u16, col)
          }
      }

    /// Texte d'une sélection exprimée en coordonnées absolues `(source, col)`.
    /// Les lignes sélectionnées sont jointes par `\n`, les caractères de
    /// continuation sont ignorés et les espaces de fin de ligne sont rognés.
    pub fn texte_selection(&self, debut: (usize, u16), fin: (usize, u16)) -> String
      {
        let (mut s0, mut c0) = debut;
        let (mut s1, mut c1) = fin;
        if (s0, c0) > (s1, c1)
          {
            std::mem::swap(&mut s0, &mut s1);
            std::mem::swap(&mut c0, &mut c1);
          }
        let derniere_col = self.largeur.saturating_sub(1);
        let mut lignes: Vec<String> = Vec::new();
        for source in s0..=s1
          {
            let col_debut = if source == s0 { c0 } else { 0 };
            let col_fin = if source == s1 { c1 } else { derniere_col };
            let mut texte = String::new();
            let mut col = col_debut;
            while col <= col_fin && col <= derniere_col
              {
                let cellule = self.cellule_source(source, col);
                if !cellule.continuation
                  {
                    texte.push(cellule.c);
                  }
                col += 1;
              }
            lignes.push(texte.trim_end().to_string());
          }
        lignes.join("\n")
      }

    /// Étendue du « mot » contenant la colonne `col` sur la ligne `source`,
    /// sous forme `(début inclus, fin exclue)`. Un caractère non-mot isole la
    /// colonne elle-même ; les caractères de continuation sont traités comme
    /// appartenant au mot.
    pub fn mot_a(&self, source: usize, col: u16) -> (u16, u16)
      {
        let est_mot = |cellule: Cellule| {
          cellule.continuation
            || cellule.c.is_alphanumeric()
            || matches!(cellule.c, '_' | '-' | '.' | '/' | '~' | ':' | '@' | '+' | '=')
        };
        if !est_mot(self.cellule_source(source, col))
          {
            return (col, (col + 1).min(self.largeur.max(1)));
          }
        let mut debut = col;
        while debut > 0 && est_mot(self.cellule_source(source, debut - 1))
          {
            debut -= 1;
          }
        let mut fin = col + 1;
        while fin < self.largeur && est_mot(self.cellule_source(source, fin))
          {
            fin += 1;
          }
        (debut, fin)
      }

    /// Fait défiler la grille d'une ligne vers le haut, sur tout l'écran.
    pub fn defiler_haut(&mut self)
      {
        self.defiler_zone_haut(0, self.hauteur.saturating_sub(1), true);
      }

    /// Fait défiler la grille d'une ligne vers le bas, sur tout l'écran.
    pub fn defiler_bas(&mut self)
      {
        self.defiler_zone_bas(0, self.hauteur.saturating_sub(1));
      }

    /// Fait défiler vers le haut la zone `[haut, bas]` d'une ligne : les lignes
    /// montent, la ligne `bas` est vidée. Si `historique` est vrai **et** que la
    /// zone commence à la ligne 0, la ligne évincée rejoint l'arrière-plan
    /// défilant ; sinon elle est perdue (plein écran ou marge haute).
    pub fn defiler_zone_haut(&mut self, haut: u16, bas: u16, historique: bool)
      {
        let haut = haut.min(self.hauteur.saturating_sub(1));
        let bas = bas.min(self.hauteur.saturating_sub(1));
        if haut > bas
          {
            return;
          }
        let largeur = self.largeur as usize;
        let l0 = haut as usize;
        let b0 = bas as usize;
        if historique && haut == 0
          {
            let premiere: Vec<Cellule> = self.cellules[0..largeur].to_vec();
            self.arriere_plan.push_back(premiere);
            while self.arriere_plan.len() > CAPACITE_ARRIERE_PLAN
              {
                self.arriere_plan.pop_front();
              }
          }
        if l0 < b0
          {
            self.cellules.copy_within((l0 + 1) * largeur..(b0 + 1) * largeur, l0 * largeur);
          }
        for cellule in &mut self.cellules[b0 * largeur..(b0 + 1) * largeur]
          {
            *cellule = Cellule::default();
          }
      }

    /// Fait défiler vers le bas la zone `[haut, bas]` d'une ligne : les lignes
    /// descendent, la ligne `haut` est vidée et rien n'alimente l'historique.
    pub fn defiler_zone_bas(&mut self, haut: u16, bas: u16)
      {
        let haut = haut.min(self.hauteur.saturating_sub(1));
        let bas = bas.min(self.hauteur.saturating_sub(1));
        if haut > bas
          {
            return;
          }
        let largeur = self.largeur as usize;
        let l0 = haut as usize;
        let b0 = bas as usize;
        if l0 < b0
          {
            self.cellules.copy_within(l0 * largeur..b0 * largeur, (l0 + 1) * largeur);
          }
        for cellule in &mut self.cellules[l0 * largeur..(l0 + 1) * largeur]
          {
            *cellule = Cellule::default();
          }
      }

    /// Insère `n` lignes vierges à partir de la ligne `lig`, dans la zone
    /// `[haut, bas]` : les lignes basses sortent de la zone (IL, `CSI L`). Ne
    /// fait rien si `lig` est hors de la zone.
    pub fn inserer_lignes(&mut self, lig: u16, n: u16, haut: u16, bas: u16, modele: Cellule)
      {
        let haut = haut.min(self.hauteur.saturating_sub(1));
        let bas = bas.min(self.hauteur.saturating_sub(1));
        if lig < haut || lig > bas
          {
            return;
          }
        let largeur = self.largeur as usize;
        let l0 = lig as usize;
        let b0 = bas as usize;
        let n0 = (n.max(1) as usize).min(b0 - l0 + 1);
        if l0 + n0 <= b0
          {
            self.cellules.copy_within(l0 * largeur..(b0 - n0 + 1) * largeur, (l0 + n0) * largeur);
          }
        for cellule in &mut self.cellules[l0 * largeur..(l0 + n0) * largeur]
          {
            *cellule = modele;
          }
      }

    /// Supprime `n` lignes à partir de la ligne `lig`, dans la zone `[haut,
    /// bas]` : les lignes basses remontent, la fin de zone est vidée (DL,
    /// `CSI M`). Ne fait rien si `lig` est hors de la zone.
    pub fn supprimer_lignes(&mut self, lig: u16, n: u16, haut: u16, bas: u16, modele: Cellule)
      {
        let haut = haut.min(self.hauteur.saturating_sub(1));
        let bas = bas.min(self.hauteur.saturating_sub(1));
        if lig < haut || lig > bas
          {
            return;
          }
        let largeur = self.largeur as usize;
        let l0 = lig as usize;
        let b0 = bas as usize;
        let n0 = (n.max(1) as usize).min(b0 - l0 + 1);
        if l0 + n0 <= b0
          {
            self.cellules.copy_within((l0 + n0) * largeur..(b0 + 1) * largeur, l0 * largeur);
          }
        for cellule in &mut self.cellules[(b0 + 1 - n0) * largeur..(b0 + 1) * largeur]
          {
            *cellule = modele;
          }
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

    fn ecrire(grille: &mut Grid, lig: u16, texte: &str)
      {
        for (col, c) in texte.chars().enumerate()
          {
            grille.poser(lig, col as u16, Cellule { c, ..Cellule::default() });
          }
      }

    #[test]
    fn la_selection_extrait_des_lignes_entieres()
      {
        let mut grille = Grid::nouvelle(8, 2);
        ecrire(&mut grille, 0, "abc");
        ecrire(&mut grille, 1, "de");
        assert_eq!(grille.texte_selection((0, 0), (1, 7)), "abc\nde");
      }

    #[test]
    fn la_selection_borne_les_colonnes()
      {
        let mut grille = Grid::nouvelle(8, 1);
        ecrire(&mut grille, 0, "abcdef");
        assert_eq!(grille.texte_selection((0, 1), (0, 3)), "bcd");
      }

    #[test]
    fn la_selection_normalise_l_ordre_des_bornes()
      {
        let mut grille = Grid::nouvelle(8, 1);
        ecrire(&mut grille, 0, "abcdef");
        assert_eq!(grille.texte_selection((0, 3), (0, 1)), "bcd");
      }

    #[test]
    fn la_selection_traverse_l_arriere_plan()
      {
        let mut grille = Grid::nouvelle(6, 1);
        ecrire(&mut grille, 0, "haut");
        grille.defiler_haut();
        ecrire(&mut grille, 0, "bas");
        assert_eq!(grille.texte_selection((0, 0), (1, 5)), "haut\nbas");
      }

    #[test]
    fn la_selection_ignore_les_continuations_de_largeur_deux()
      {
        let mut grille = Grid::nouvelle(4, 1);
        grille.poser(0, 0, Cellule { c: '你', ..Cellule::default() });
        grille.poser(0, 1, Cellule { continuation: true, ..Cellule::default() });
        grille.poser(0, 2, Cellule { c: 'x', ..Cellule::default() });
        assert_eq!(grille.texte_selection((0, 0), (0, 3)), "你x");
      }

    #[test]
    fn le_mot_couvre_les_caracteres_contigus()
      {
        let mut grille = Grid::nouvelle(12, 1);
        ecrire(&mut grille, 0, "ls -l /tmp/x");
        assert_eq!(grille.mot_a(0, 4), (3, 5));
        assert_eq!(grille.mot_a(0, 7), (6, 12));
      }

    #[test]
    fn un_caractere_isole_n_est_pas_un_mot()
      {
        let mut grille = Grid::nouvelle(6, 1);
        ecrire(&mut grille, 0, "a b");
        assert_eq!(grille.mot_a(0, 1), (1, 2));
      }

    #[test]
    fn le_defilement_de_zone_ne_touche_pas_hors_zone()
      {
        let mut grille = Grid::nouvelle(3, 4);
        ecrire(&mut grille, 0, "AAA");
        ecrire(&mut grille, 1, "BBB");
        ecrire(&mut grille, 2, "CCC");
        ecrire(&mut grille, 3, "DDD");
        grille.defiler_zone_haut(1, 2, false);
        assert_eq!(grille.texte_ligne(0), "AAA");
        assert_eq!(grille.texte_ligne(1), "CCC");
        assert_eq!(grille.texte_ligne(2), "   ");
        assert_eq!(grille.texte_ligne(3), "DDD");
        assert_eq!(grille.arriere_plan_len(), 0);
      }

    #[test]
    fn le_defilement_de_zone_depuis_le_haut_alimente_l_historique()
      {
        let mut grille = Grid::nouvelle(3, 2);
        ecrire(&mut grille, 0, "AAA");
        ecrire(&mut grille, 1, "BBB");
        grille.defiler_zone_haut(0, 1, true);
        assert_eq!(grille.arriere_plan_len(), 1);
        assert_eq!(grille.texte_ligne(0), "BBB");
      }

    #[test]
    fn inserer_et_supprimer_lignes_dans_la_zone()
      {
        let mut grille = Grid::nouvelle(3, 4);
        ecrire(&mut grille, 0, "AAA");
        ecrire(&mut grille, 1, "BBB");
        ecrire(&mut grille, 2, "CCC");
        ecrire(&mut grille, 3, "DDD");
        grille.inserer_lignes(1, 1, 0, 3, Cellule::default());
        assert_eq!(grille.texte_ligne(1), "   ");
        assert_eq!(grille.texte_ligne(2), "BBB");
        assert_eq!(grille.texte_ligne(3), "CCC");
        grille.supprimer_lignes(1, 1, 0, 3, Cellule::default());
        assert_eq!(grille.texte_ligne(1), "BBB");
        assert_eq!(grille.texte_ligne(2), "CCC");
        assert_eq!(grille.texte_ligne(3), "   ");
      }
  }
