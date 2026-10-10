//! `term_context` — ligne dynamique (dernière ligne).
//!
//! Barre pleine largeur à trois zones : à gauche les commandes/raccourcis, au
//! centre le code retour (vert + « ok » si nul, rouge sinon), à droite les
//! compteurs système compacts (cpu / mémoire / io), rafraîchis en continu.

/// Surlignage par défaut de `term_context`.
pub const FOND: (u8, u8, u8) = super::FOND_CADRE;
/// Avant-plan par défaut de `term_context`.
pub const TEXTE: (u8, u8, u8) = (0xE6, 0xED, 0xF3);
/// Couleur du code retour réussi.
pub const VERT: (u8, u8, u8) = (0x89, 0xD1, 0x85);
/// Couleur du code retour en erreur.
pub const ROUGE: (u8, u8, u8) = (0xFF, 0x6B, 0x6B);

/// Vue du contexte au moment du rendu.
#[derive(Clone, Debug, Default)]
pub struct Contexte
  {
    /// Mode capture actif.
    pub capture: bool,
    /// Bascule root détectée dans la ligne en cours.
    pub root: bool,
    /// Dernier code retour connu.
    pub statut: Option<i32>,
    /// Options découvertes pour la commande en cours.
    pub options: Vec<String>,
    /// Suggestion fantôme (historique).
    pub suggestion: Option<String>,
    /// Une surcouche d'aide est ouverte.
    pub overlay: bool,
    /// Compteurs système compacts (cpu/mémoire/io), déjà mis en forme.
    pub systeme: String,
  }

/// Zone gauche : rappel des touches, options découvertes et suggestion.
pub fn gauche(contexte: &Contexte) -> String
  {
    if contexte.overlay
      {
        return "Échap ferme".to_string();
      }
    let mut fragments: Vec<String> = Vec::new();
    if let Some(suggestion) = &contexte.suggestion
      {
        fragments.push(format!("↪ {suggestion}"));
      }
    if !contexte.options.is_empty()
      {
        fragments.push(contexte.options.iter().take(4).cloned().collect::<Vec<_>>().join(" · "));
      }
    if contexte.suggestion.is_some() || !contexte.options.is_empty()
      {
        fragments.push("F1 - Aide contextuelle   F10 - Aide   F2 - Capture".to_string());
      }
    else
      {
        fragments.push("F1 - Aide contextuelle   F10 - Aide   F2 - Capture   Ctrl+F1 - Suggestion".to_string());
      }
    fragments.join("   ")
  }

/// Zone centre : alerte root et code retour (`ok` / `✗ N`).
pub fn centre(contexte: &Contexte) -> String
  {
    let mut fragments: Vec<String> = Vec::new();
    if contexte.root
      {
        fragments.push("ROOT!".to_string());
      }
    if let Some(code) = contexte.statut
      {
        fragments.push(if code == 0 { "✓ ok".to_string() } else { format!("✗ {code}") });
      }
    fragments.join(" ")
  }

/// Zone droite : compteurs système compacts (cpu/mémoire/io).
pub fn droite(contexte: &Contexte) -> String
  {
    contexte.systeme.clone()
  }

/// Couleur de la zone centre selon le code retour.
pub fn couleur_centre(contexte: &Contexte) -> (u8, u8, u8)
  {
    match contexte.statut
      {
        Some(0) => VERT,
        Some(_) => ROUGE,
        None if contexte.root => ROUGE,
        None => TEXTE,
      }
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn le_rappel_des_touches_est_toujours_present()
      {
        let ligne = gauche(&Contexte::default());
        assert!(ligne.contains("F1 - Aide contextuelle"));
        assert!(ligne.contains("F10 - Aide"));
        assert!(ligne.contains("F2 - Capture"));
        assert!(ligne.contains("Ctrl+F1 - Suggestion"));
      }

    #[test]
    fn le_code_retour_nul_affiche_ok_en_vert()
      {
        let contexte = Contexte { statut: Some(0), ..Contexte::default() };
        assert_eq!(centre(&contexte), "✓ ok");
        assert_eq!(couleur_centre(&contexte), VERT);
      }

    #[test]
    fn un_code_retour_non_nul_est_rouge()
      {
        let contexte = Contexte { statut: Some(2), ..Contexte::default() };
        assert_eq!(centre(&contexte), "✗ 2");
        assert_eq!(couleur_centre(&contexte), ROUGE);
      }

    #[test]
    fn le_root_est_annonce()
      {
        let contexte = Contexte { root: true, ..Contexte::default() };
        assert!(centre(&contexte).contains("ROOT!"));
        assert_eq!(couleur_centre(&contexte), ROUGE);
      }
  }
