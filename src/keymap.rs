//! Touches du chrome, actives uniquement lorsque le prompt est au premier plan.
//!
//! Hors prompt (commande en cours), aucune touche n'est interceptée : tout est
//! transmis à l'enfant.

/// Action déclenchée par une touche du chrome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action
  {
    /// F1 — aide contextuelle (man / fork `--help`).
    AideContextuelle,
    /// Ctrl+F1 — page `man` de la commande en cours.
    Man,
    /// F10 — aide de Bashop.
    AideBashop,
    /// F2 — bascule du mode capture.
    BasculeCapture,
  }
