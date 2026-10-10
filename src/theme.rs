//! Thèmes et palettes.
//!
//! `classique` est la base dont dérivent `wheel` et `root` avant toute
//! personnalisation. Le suffixe de fin de prompt est cadré sur un champ de
//! largeur commune (voir `config::suffix_width`) pour que la bascule
//! automatique sous `sudo` ne décale jamais la commande en cours de frappe.

/// Couleur RVB 24 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb
  {
    /// Composante rouge.
    pub r: u8,
    /// Composante verte.
    pub g: u8,
    /// Composante bleue.
    pub b: u8,
  }

impl Rgb
  {
    /// Construit une couleur RVB.
    pub const fn new(r: u8, g: u8, b: u8) -> Self
      {
        Self { r, g, b }
      }
  }

/// Suffixe de fin de prompt : un glyphe et sa couleur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Suffixe
  {
    /// Chaîne affichée (cadrée à droite dans le champ commun).
    pub glyphe: &'static str,
    /// Couleur de l'avant-plan.
    pub couleur: Rgb,
  }

/// Thème complet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme
  {
    /// Nom du thème.
    pub nom: &'static str,
    /// Couleur d'accent (identité utilisateur).
    pub accent: Rgb,
    /// Suffixe de fin de prompt.
    pub suffixe: Suffixe,
  }

/// Thème classique (utilisateur normal) — base des deux autres.
pub const CLASSIQUE: Theme =
  Theme {
    nom: "classique",
    accent: Rgb::new(0xA6, 0xE3, 0xA1),
    suffixe: Suffixe { glyphe: ">>", couleur: Rgb::new(0x4E, 0x9A, 0x06) },
  };

/// Thème wheel (membre du groupe wheel / administrateurs).
pub const WHEEL: Theme =
  Theme {
    nom: "wheel",
    accent: Rgb::new(0xCB, 0xA6, 0xF7),
    suffixe: Suffixe { glyphe: ">>>>", couleur: Rgb::new(0xCB, 0xA6, 0xF7) },
  };

/// Thème root (superutilisateur).
pub const ROOT: Theme =
  Theme {
    nom: "root",
    accent: Rgb::new(0xF3, 0x8B, 0xA8),
    suffixe: Suffixe { glyphe: "###>", couleur: Rgb::new(0xF3, 0x8B, 0xA8) },
  };

/// Résout un thème par son nom (repli sur `classique`).
pub fn par_nom(nom: &str) -> Theme
  {
    match nom.to_ascii_lowercase().as_str()
      {
        "wheel" => WHEEL,
        "root" => ROOT,
        _ => CLASSIQUE,
      }
  }

/// Résout le thème effectif : `auto` (ou vide) suit le privilège détecté.
///
/// Le groupe d'administration est paramétré selon l'OS hôte (voir
/// `detect::privilege`) : `wheel`/`sudo` sous Linux, `admin` sous macOS,
/// `operator` sous FreeBSD, `Administrateurs` sous Windows.
pub fn resoudre(nom: &str, privilege: crate::detect::Privilege) -> Theme
  {
    match nom.to_ascii_lowercase().as_str()
      {
        "" | "auto" => match privilege
          {
            crate::detect::Privilege::Root => ROOT,
            crate::detect::Privilege::Wheel => WHEEL,
            crate::detect::Privilege::Classique => CLASSIQUE,
          },
        _ => par_nom(nom),
      }
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn le_theme_inconnu_retombe_sur_classique()
      {
        assert_eq!(par_nom("inconnu"), CLASSIQUE);
      }

    #[test]
    fn les_noms_resolvent_les_bons_themes()
      {
        assert_eq!(par_nom("wheel"), WHEEL);
        assert_eq!(par_nom("ROOT"), ROOT);
      }

    #[test]
    fn les_glyphes_de_suffixe_sont_distincts()
      {
        assert_eq!(CLASSIQUE.suffixe.glyphe, ">>");
        assert_eq!(WHEEL.suffixe.glyphe, ">>>>");
        assert_eq!(ROOT.suffixe.glyphe, "###>");
      }

    #[test]
    fn le_theme_auto_suit_le_privilege()
      {
        use crate::detect::Privilege;
        assert_eq!(resoudre("auto", Privilege::Classique), CLASSIQUE);
        assert_eq!(resoudre("auto", Privilege::Wheel), WHEEL);
        assert_eq!(resoudre("", Privilege::Root), ROOT);
      }

    #[test]
    fn le_theme_explicite_prime_sur_le_privilege()
      {
        use crate::detect::Privilege;
        assert_eq!(resoudre("classique", Privilege::Root), CLASSIQUE);
      }
  }
