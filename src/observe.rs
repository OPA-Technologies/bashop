//! Observation de l'état du shell.
//!
//! M1 : lecture des marqueurs OSC 133 (prompt / commande / sortie / fin) avec
//! repli sur une heuristique d'inactivité du pty pour `cmd.exe`. Cette
//! observation alimente le lexer (bascule `sudo`) et la sécurité.

/// État courant du shell placé dans la zone centrale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EtatShell
  {
    /// Le shell attend une saisie (prompt actif).
    Prompt,
    /// Une commande s'exécute (sortie en cours).
    Commande,
    /// État indéterminé (repli).
    Inconnu,
  }

/// Piste d'observation retenue pour l'implémentation.
pub const STRATEGIE: &str = "OSC 133 + repli idle";
