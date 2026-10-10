//! `term_stat` — ligne d'état générale (ligne 1).
//!
//! Gauche : `user@host [IP]` · centre : OS+flavor · droite : pays, date/heure,
//! UTC et charge. Le contenu est réparti sur la largeur du terminal.

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
/// `identite` et `charge` sont fournies par l'appelant (identité dynamique sous
/// reprise root, charge rafraîchie au tick) pour éviter des résolutions
/// coûteuses à chaque rendu.
pub fn contenu(largeur: u16, identite: &str, charge: &str) -> String
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
    if !charge.is_empty()
      {
        droite.push_str(&format!(" · {}", charge));
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
  }
