//! `term_context` — ligne dynamique (dernière ligne).
//!
//! Barre pleine largeur à trois zones : à gauche les commandes/raccourcis, au
//! centre le code retour (vert + « ok » si nul, rouge sinon), à droite les
//! compteurs système compacts (io / réseau / mémoire / cpu), rafraîchis en
//! continu.

/// Surlignage par défaut de `term_context`.
pub const FOND: (u8, u8, u8) = super::FOND_CADRE;
/// Avant-plan par défaut de `term_context`.
pub const TEXTE: (u8, u8, u8) = (0xE6, 0xED, 0xF3);
/// Couleur du code retour réussi.
pub const VERT: (u8, u8, u8) = (0x89, 0xD1, 0x85);
/// Couleur du code retour en erreur.
pub const ROUGE: (u8, u8, u8) = (0xFF, 0x6B, 0x6B);
/// Couleur d'une notification de fin de commande longue.
pub const JAUNE: (u8, u8, u8) = (0xF9, 0xE2, 0xAF);
/// Rappel compact des raccourcis du chrome. Le libellé de `F2` reflète le mode
/// **actif** : `F2 - Secret` quand la capture (masquage) est on, `F2 - Capture`
/// sinon.
pub fn rappel_touches(capture: bool) -> String
  {
    let f2 = if capture { "F2 - Secret" } else { "F2 - Capture" };
    format!("F1 - Aide rapide   {f2}   F10 - Aide")
  }

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
    /// Une surcouche d'aide est ouverte.
    pub overlay: bool,
    /// Compteurs système compacts (io / réseau / mémoire / cpu), déjà mis en
    /// forme.
    pub systeme: String,
    /// Notification de fin de commande longue (transitoire).
    pub notification: Option<String>,
    /// Horodatage de lancement de la dernière commande, le cas échéant.
    pub horodatage: Option<String>,
    /// Alerte de risque sur la commande en cours de saisie (risque détecté ou
    /// simulation disponible).
    pub alerte: Option<String>,
    /// Copie de la première ligne du message d'erreur de la dernière commande
    /// (affichée avec le code retour), sans jamais altérer la sortie réelle.
    pub derniere_erreur: Option<String>,
  }

/// Zone gauche : rappel des touches et options découvertes.
pub fn gauche(contexte: &Contexte) -> String
  {
    if contexte.overlay
      {
        return "Échap ferme".to_string();
      }
    let mut fragments: Vec<String> = Vec::new();
    if !contexte.options.is_empty()
      {
        fragments.push(contexte.options.iter().take(4).cloned().collect::<Vec<_>>().join(" · "));
      }
    fragments.push(rappel_touches(contexte.capture));
    fragments.join("   ")
  }

/// Zone centre : alerte de risque, alerte root, code retour (`ok` / `✗ N`),
/// notification de fin de commande longue et horodatage de lancement.
pub fn centre(contexte: &Contexte) -> String
  {
    let mut fragments: Vec<String> = Vec::new();
    if let Some(alerte) = &contexte.alerte
      {
        fragments.push(alerte.clone());
      }
    if contexte.root
      {
        fragments.push("ROOT!".to_string());
      }
    if let Some(code) = contexte.statut
      {
        if code == 0
          {
            fragments.push("✓ ok".to_string());
          }
        else if let Some(erreur) = &contexte.derniere_erreur
          {
            fragments.push(format!("✗ {code} — {erreur}"));
          }
        else
          {
            fragments.push(format!("✗ {code}"));
          }
      }
    if let Some(notification) = &contexte.notification
      {
        fragments.push(notification.clone());
      }
    if let Some(horodatage) = &contexte.horodatage
      {
        fragments.push(horodatage.clone());
      }
    fragments.join(" ")
  }

/// Zone droite : compteurs système compacts (io / réseau / mémoire / cpu).
pub fn droite(contexte: &Contexte) -> String
  {
    contexte.systeme.clone()
  }

/// Couleur de la zone centre selon le code retour et la notification.
pub fn couleur_centre(contexte: &Contexte) -> (u8, u8, u8)
  {
    match contexte.statut
      {
        Some(0) => VERT,
        Some(_) => ROUGE,
        None if contexte.root => ROUGE,
        None if contexte.alerte.is_some() => JAUNE,
        None if contexte.notification.is_some() => JAUNE,
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
        let secret = gauche(&Contexte { capture: true, ..Contexte::default() });
        assert!(secret.contains("F1 - Aide rapide"));
        assert!(secret.contains("F2 - Secret"));
        assert!(secret.contains("F10 - Aide"));
      }

    #[test]
    fn le_libelle_de_f2_suit_le_mode_actif()
      {
        let secret = gauche(&Contexte { capture: true, ..Contexte::default() });
        assert!(secret.contains("F2 - Secret"), "{secret}");
        assert!(!secret.contains("F2 - Capture"), "{secret}");
        let clair = gauche(&Contexte { capture: false, ..Contexte::default() });
        assert!(clair.contains("F2 - Capture"), "{clair}");
        assert!(!clair.contains("F2 - Secret"), "{clair}");
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
    fn le_message_d_erreur_accompagne_le_code_retour()
      {
        let contexte = Contexte {
          statut: Some(127),
          derniere_erreur: Some("bash: foo: command not found".to_string()),
          ..Contexte::default()
        };
        let centre = centre(&contexte);
        assert!(centre.starts_with("✗ 127"), "{centre}");
        assert!(centre.contains("command not found"), "{centre}");
        assert_eq!(couleur_centre(&contexte), ROUGE);
      }

    #[test]
    fn le_root_est_annonce()
      {
        let contexte = Contexte { root: true, ..Contexte::default() };
        assert!(centre(&contexte).contains("ROOT!"));
        assert_eq!(couleur_centre(&contexte), ROUGE);
      }

    #[test]
    fn la_notification_de_fin_de_commande_est_affichee_en_jaune()
      {
        let contexte = Contexte {
          notification: Some("⏱ terminée en 12 s".to_string()),
          ..Contexte::default()
        };
        assert!(centre(&contexte).contains("12 s"));
        assert_eq!(couleur_centre(&contexte), JAUNE);
      }

    #[test]
    fn l_horodatage_et_la_notification_cohabitent()
      {
        let contexte = Contexte {
          notification: Some("⏱ terminée en 8 s".to_string()),
          horodatage: Some("🕐 18:05:33".to_string()),
          ..Contexte::default()
        };
        let centre = centre(&contexte);
        assert!(centre.contains("8 s"));
        assert!(centre.contains("18:05:33"));
      }

    #[test]
    fn une_alerte_de_risque_est_affichee_en_jaune()
      {
        let contexte = Contexte {
          alerte: Some("⚠ suppression récursive d'une racine".to_string()),
          ..Contexte::default()
        };
        assert!(centre(&contexte).contains("suppression récursive"));
        assert_eq!(couleur_centre(&contexte), JAUNE);
      }
  }
