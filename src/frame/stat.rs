//! `term_stat` — ligne d'état générale (ligne 1).
//!
//! Gauche : OS+flavor · centre : `user@host [IP]` · droite : pays, date/heure
//! et UTC. Le contenu est réparti sur la largeur du terminal.

use crate::providers;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Surlignage par défaut de `term_stat`.
pub const FOND: (u8, u8, u8) = super::FOND_CADRE;
/// Avant-plan par défaut de `term_stat`.
pub const TEXTE: (u8, u8, u8) = (0xE6, 0xED, 0xF3);

/// Largeur d'affichage (colonnes) d'une chaîne, glyphes larges compris.
fn largeur_affichage(texte: &str) -> usize
  {
    UnicodeWidthStr::width(texte)
  }

/// Tronque une chaîne à `colonnes` colonnes d'affichage, sans couper un glyphe
/// large (drapeau, emoji).
fn tronquer(texte: &str, colonnes: usize) -> String
  {
    let mut sortie = String::new();
    let mut utilisees = 0;
    for c in texte.chars()
      {
        let largeur = UnicodeWidthChar::width(c).unwrap_or(0);
        if utilisees + largeur > colonnes
          {
            break;
          }
        sortie.push(c);
        utilisees += largeur;
      }
    sortie
  }

/// Répartit trois fragments (gauche, centre, droite) sur une ligne.
///
/// Le placement raisonne en **colonnes d'affichage** (`unicode-width`) : la
/// chaîne produite occupe exactement `largeur` colonnes, que les fragments
/// contiennent ou non des glyphes larges.
pub fn repartir(gauche: &str, centre: &str, droite: &str, largeur: usize) -> String
  {
    if largeur == 0
      {
        return String::new();
      }
    let reserve = if !droite.is_empty() && largeur > 1 { 1 } else { 0 };
    let g = tronquer(gauche, largeur - reserve);
    let largeur_gauche = largeur_affichage(&g);
    let place_droite = largeur - largeur_gauche;
    let d = tronquer(droite, place_droite);
    let largeur_droite = largeur_affichage(&d);
    let zone_centre = largeur.saturating_sub(largeur_gauche + largeur_droite);
    let c = tronquer(centre, zone_centre);
    let largeur_centre = largeur_affichage(&c);
    let reste = zone_centre.saturating_sub(largeur_centre);
    let avant_centre = reste / 2;
    let apres_centre = reste - avant_centre;
    let mut sortie = String::with_capacity(largeur);
    sortie.push_str(&g);
    sortie.extend(std::iter::repeat_n(' ', avant_centre));
    sortie.push_str(&c);
    sortie.extend(std::iter::repeat_n(' ', apres_centre));
    sortie.push_str(&d);
    sortie
  }

/// Compose la ligne d'état complète.
///
/// `identite` est fournie par l'appelant (identité dynamique sous reprise root)
/// pour éviter des résolutions coûteuses à chaque rendu.
///
/// Sous `capture` (mode secret), chaque fragment est masqué par
/// [`providers::masquer`] **avant** la répartition : `term_stat` reste
/// synchronisé avec la bascule `F2` (nom d'hôte, FQDN, IP et secrets masqués)
/// tout en conservant une ligne pleine largeur, car `repartir` calcule les
/// espacements sur le texte déjà masqué.
pub fn contenu(largeur: u16, identite: &str, capture: bool) -> String
  {
    let fqdn = providers::fqdn();
    let mut gauche = providers::os_flavor();
    let mut centre = match providers::adresse_ip()
      {
        Some(ip) => format!("{} [{} {}]", identite, fqdn, ip),
        None => format!("{} [{}]", identite, fqdn),
      };
    let (local, utc) = providers::horloge();
    let pays = providers::code_pays();
    let drapeau = providers::drapeau(&pays);
    let mut droite = if drapeau.is_empty()
      {
        format!("[{}] {}", pays, local)
      }
    else
      {
        format!("{} [{}] {}", drapeau, pays, local)
      };
    if utc != local
      {
        droite.push_str(&format!(" · UTC {}", utc));
      }
    if capture
      {
        gauche = providers::masquer(&gauche);
        centre = providers::masquer(&centre);
        droite = providers::masquer(&droite);
      }
    repartir(&gauche, &centre, &droite, largeur as usize)
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn la_repartition_respecte_la_largeur()
      {
        let ligne = repartir("gauche", "centre", "droite", 30);
        assert_eq!(ligne.chars().count(), 30);
        assert!(ligne.starts_with("gauche"));
        assert!(ligne.ends_with("droite"));
      }

    #[test]
    fn le_centre_est_place_au_milieu()
      {
        let ligne = repartir("", "X", "", 11);
        assert_eq!(ligne.chars().nth(5), Some('X'));
      }

    #[test]
    fn une_ligne_trop_longue_est_tronquee()
      {
        let ligne = repartir("aaaaaaaaaa", "", "", 4);
        assert_eq!(ligne, "aaaa");
      }

    #[test]
    fn la_repartition_compte_les_glyphes_larges()
      {
        let ligne = repartir("", "🇫🇷", "", 4);
        assert_eq!(largeur_affichage(&ligne), 4, "{ligne}");
        let ligne = repartir("🇫🇷", "X", "Y", 6);
        assert_eq!(largeur_affichage(&ligne), 6, "{ligne}");
        assert!(ligne.starts_with("🇫🇷"), "{ligne}");
        assert!(ligne.ends_with('Y'), "{ligne}");
      }

    #[test]
    fn la_ligne_est_masquee_sous_capture()
      {
        let clair = contenu(80, "user@10.0.0.1", false);
        assert!(clair.contains("10.0.0.1"), "l'IP doit apparaître hors capture : {clair}");
        assert_eq!(largeur_affichage(&clair), 80, "{clair}");
        let masque = contenu(80, "user@10.0.0.1", true);
        assert!(!masque.contains("10.0.0.1"), "l'IP doit être masquée sous capture : {masque}");
        assert!(masque.contains("x.x.x.x"), "masque IPv4 attendu : {masque}");
        assert_eq!(largeur_affichage(&masque), 80, "la ligne doit rester pleine largeur : {masque}");
      }

    #[test]
    fn la_ligne_atteint_toujours_la_derniere_colonne()
      {
        for largeur in [1u16, 12, 40, 80, 200]
          {
            for capture in [false, true]
              {
                let ligne = contenu(largeur, "user@192.168.100.200", capture);
                assert_eq!(
                  largeur_affichage(&ligne),
                  largeur as usize,
                  "largeur {largeur}, capture {capture} : {ligne:?}"
                );
              }
          }
      }
  }
