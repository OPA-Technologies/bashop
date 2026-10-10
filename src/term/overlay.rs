//! Surcouche de la zone centrale : aide contextuelle, aide de Bashop,
//! confirmation de sécurité.
//!
//! Une surcouche ne recouvre **que** la zone centrale : les trois lignes du
//! cadre restent visibles.

/// Nature de la surcouche, qui détermine la gestion des touches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nature
  {
    /// Aide ou suggestion : toute touche ferme.
    Information,
    /// Demande de confirmation : `o`/Entrée exécute, `n`/Échap annule.
    Confirmation,
  }

/// Contenu affiché au-dessus de la zone centrale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlay
  {
    /// Titre du panneau.
    pub titre: String,
    /// Lignes de contenu.
    pub lignes: Vec<String>,
    /// Nature de la surcouche.
    pub nature: Nature,
    /// Touches d'aide affichées en pied.
    pub aide: String,
  }

impl Overlay
  {
    /// Surcouche d'information.
    pub fn information(titre: impl Into<String>, lignes: Vec<String>) -> Self
      {
        Self {
          titre: titre.into(),
          lignes,
          nature: Nature::Information,
          aide: "toute touche ferme".to_string(),
        }
      }

    /// Surcouche de confirmation.
    pub fn confirmation(titre: impl Into<String>, lignes: Vec<String>) -> Self
      {
        Self {
          titre: titre.into(),
          lignes,
          nature: Nature::Confirmation,
          aide: "[o] exécuter   [n] annuler".to_string(),
        }
      }
  }
