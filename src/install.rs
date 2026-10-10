//! Injection idempotente du bootstrap dans les rc des shells (`bashop install`).
//!
//! Le bloc est encadré par des marqueurs ; l'installation retire d'abord un
//! éventuel bloc existant, ce qui rend l'opération ré-entrante. Aucune
//! commande shell n'est exécutée : seuls des fichiers rc sont modifiés.

use std::path::PathBuf;

use crate::bootstrap::{MARQUE_DEBUT, MARQUE_FIN, bloc_bash, bloc_zsh};
use crate::shells::Shell;

/// Chemin du fichier rc principal d'un shell (best-effort).
pub fn rc_principal(shell: Shell) -> Option<PathBuf>
  {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    match shell
      {
        Shell::Bash => Some(home.join(".bashrc")),
        Shell::Zsh => Some(home.join(".zshrc")),
        Shell::Sh => Some(home.join(".profile")),
        Shell::PowerShell | Shell::Cmd => None,
      }
  }

/// Bloc d'activation à injecter pour un shell (aucun pour `sh`/PowerShell/cmd).
pub fn bloc(shell: Shell) -> Option<String>
  {
    match shell
      {
        Shell::Bash => Some(bloc_bash()),
        Shell::Zsh => Some(bloc_zsh()),
        Shell::Sh | Shell::PowerShell | Shell::Cmd => None,
      }
  }

/// Retire le bloc Bashop d'un contenu de rc, en conservant le reste.
pub fn retirer_bloc(contenu: &str) -> String
  {
    let mut sortie = String::new();
    let mut dedans = false;
    for ligne in contenu.lines()
      {
        let nue = ligne.trim();
        if nue == MARQUE_DEBUT
          {
            dedans = true;
            continue;
          }
        if nue == MARQUE_FIN
          {
            dedans = false;
            continue;
          }
        if !dedans
          {
            sortie.push_str(ligne);
            sortie.push('\n');
          }
      }
    sortie
  }

/// Contenu après injection idempotente du bloc pour `shell`.
pub fn injecter_bloc(contenu: &str, shell: Shell) -> String
  {
    let Some(bloc) = bloc(shell)
      else
        {
          return contenu.to_string();
        };
    let mut sortie = retirer_bloc(contenu).trim_end().to_string();
    sortie.push_str("\n\n");
    sortie.push_str(&bloc);
    sortie
  }

/// Shells ciblés par défaut (ceux disposant d'un bloc et d'un rc connu).
pub fn shells_cibles() -> Vec<Shell>
  {
    [Shell::Bash, Shell::Zsh, Shell::Sh]
      .into_iter()
      .filter(|shell| bloc(*shell).is_some() && rc_principal(*shell).is_some())
      .collect()
  }

/// Installe le bloc dans le `shell` demandé (ou tous les shells ciblés).
/// Renvoie la liste des fichiers touchés.
pub fn installer(shell: Option<Shell>) -> Vec<PathBuf>
  {
    let cibles = match shell
      {
        Some(s) if bloc(s).is_some() => vec![s],
        Some(_) => Vec::new(),
        None => shells_cibles(),
      };
    let mut touches = Vec::new();
    for cible in cibles
      {
        let Some(chemin) = rc_principal(cible)
          else
            {
              continue;
            };
        let contenu = std::fs::read_to_string(&chemin).unwrap_or_default();
        let nouveau = injecter_bloc(&contenu, cible);
        if nouveau != contenu && std::fs::write(&chemin, nouveau).is_ok()
          {
            touches.push(chemin);
          }
      }
    touches
  }

/// Retire le bloc de tous les rc connus. Renvoie les fichiers touchés.
pub fn desinstaller() -> Vec<PathBuf>
  {
    let mut touches = Vec::new();
    for cible in [Shell::Bash, Shell::Zsh, Shell::Sh]
      {
        let Some(chemin) = rc_principal(cible)
          else
            {
              continue;
            };
        let Ok(contenu) = std::fs::read_to_string(&chemin)
          else
            {
              continue;
            };
        let nouveau = retirer_bloc(&contenu);
        if nouveau != contenu && std::fs::write(&chemin, nouveau).is_ok()
          {
            touches.push(chemin);
          }
      }
    touches
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn l_injection_puis_le_retrait_restaure_le_contenu()
      {
        let initial = "export PATH=$PATH:/x\n# commentaire\n";
        let injecte = injecter_bloc(initial, Shell::Bash);
        assert!(injecte.contains(MARQUE_DEBUT));
        assert!(injecte.contains("export PATH=$PATH:/x"));
        let restaure = retirer_bloc(&injecte);
        assert_eq!(restaure.trim_end(), initial.trim_end());
      }

    #[test]
    fn l_injection_est_idempotente()
      {
        let une = injecter_bloc("alias ll='ls -l'\n", Shell::Bash);
        let deux = injecter_bloc(&une, Shell::Bash);
        assert_eq!(deux.matches(MARQUE_DEBUT).count(), 1);
        assert_eq!(deux, une);
      }

    #[test]
    fn les_shells_sans_bloc_ne_sont_pas_modifies()
      {
        assert_eq!(injecter_bloc("contenu", Shell::Cmd), "contenu");
      }
  }
