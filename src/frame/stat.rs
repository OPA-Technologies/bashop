//! `term_stat` — ligne d'état générale (ligne 1).
//!
//! Gauche : `user@host [IP]` · centre : OS+flavor · droite : pays, date/heure
//! et UTC. Le contenu est réparti sur la largeur du terminal.

use crate::providers;

/// Surlignage par défaut de `term_stat`.
pub const FOND: (u8, u8, u8) = super::FOND_CADRE;
/// Avant-plan par défaut de `term_stat`.
pub const TEXTE: (u8, u8, u8) = (0xE6, 0xED, 0xF3);

/// Tronque une chaîne à `largeur` colonnes (au sens des caractères).
fn tronquer(texte: &str, largeur: usize) -> String
  {
    texte.chars().take(largeur).collect()
  }

/// Répartit trois fragments (gauche, centre, droite) sur une ligne.
pub fn repartir(gauche: &str, centre: &str, droite: &str, largeur: usize) -> String
  {
    if largeur == 0
      {
        return String::new();
      }
    let mut tampon: Vec<char> = vec![' '; largeur];
    let g = tronquer(gauche, largeur);
    let place_gauche = g.chars().count();
    for (i, ch) in g.chars().enumerate()
      {
        tampon[i] = ch;
      }
    let place_droite = largeur.saturating_sub(place_gauche + 1).max(1);
    let d = tronquer(droite, place_droite);
    let largeur_droite = d.chars().count();
    let depart_droite = largeur.saturating_sub(largeur_droite);
    for (i, ch) in d.chars().enumerate()
      {
        let position = depart_droite + i;
        if position < largeur
          {
            tampon[position] = ch;
          }
      }
    let espace = depart_droite.saturating_sub(place_gauche);
    let c = tronquer(centre, espace);
    let largeur_centre = c.chars().count();
    let depart_centre = place_gauche + espace.saturating_sub(largeur_centre) / 2;
    for (i, ch) in c.chars().enumerate()
      {
        let position = depart_centre + i;
        if position < depart_droite && position < largeur
          {
            tampon[position] = ch;
          }
      }
    tampon.into_iter().collect()
  }

/// Compose la ligne d'état complète.
///
/// `identite` est fournie par l'appelant (identité dynamique sous reprise root)
/// pour éviter des résolutions coûteuses à chaque rendu.
///
/// Sous `capture` (mode secret), la ligne est passée par [`providers::masquer`]
/// de sorte que `term_stat` reste synchronisé avec la bascule F2 : le nom
/// d'hôte, le FQDN, l'IP et les secrets n'y apparaissent jamais plus longtemps
/// que dans la zone centrale.
pub fn contenu(largeur: u16, identite: &str, capture: bool) -> String
  {
    let fqdn = providers::fqdn();
    let gauche = match providers::adresse_ip()
      {
        Some(ip) => format!("{} [{} {}]", identite, fqdn, ip),
        None => format!("{} [{}]", identite, fqdn),
      };
    let centre = providers::os_flavor();
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
    let ligne = repartir(&gauche, &centre, &droite, largeur as usize);
    if capture
      {
        providers::masquer(&ligne)
      }
    else
      {
        ligne
      }
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
    fn la_ligne_est_masquee_sous_capture()
      {
        let clair = contenu(80, "user@10.0.0.1", false);
        assert!(clair.contains("10.0.0.1"), "l'IP doit apparaître hors capture : {clair}");
        let masque = contenu(80, "user@10.0.0.1", true);
        assert!(!masque.contains("10.0.0.1"), "l'IP doit être masquée sous capture : {masque}");
        assert!(masque.contains("x.x.x.x"), "masque IPv4 attendu : {masque}");
      }
  }
