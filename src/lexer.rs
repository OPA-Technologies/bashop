//! Lexer shell léger, partagé par le thème (bascule `sudo`) et la sécurité.
//!
//! Il découpe une ligne de commande sur les métacaractères pour détecter des
//! mots de commande, y compris au sein d'une commande composée :
//! `ls ; sudo chmod 700 .` ou `find / -exec sudo rm -fr {} \;`.

/// Caractères qui séparent les mots d'une ligne de commande.
pub const METACARACTERES: &[char] =
  &[' ', '\t', ';', '|', '&', '(', ')', '{', '}', '<', '>', '$', '`', '\n', '\r'];

/// Découpe une ligne en mots, en écartant les jetons vides.
pub fn decouper(commande: &str) -> Vec<&str>
  {
    commande
      .split(|c| METACARACTERES.contains(&c))
      .filter(|t| !t.is_empty())
      .collect()
  }

/// Indique si la ligne contient un des déclencheurs comme mot de commande.
pub fn contient_declencheur(commande: &str, declencheurs: &[String]) -> bool
  {
    decouper(commande)
      .iter()
      .any(|mot| declencheurs.iter().any(|d| d == mot))
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    fn decls() -> Vec<String>
      {
        ["sudo", "doas", "su", "sudoedit"]
          .iter()
          .map(|s| (*s).to_string())
          .collect()
      }

    #[test]
    fn un_simple_ls_n_a_pas_de_declencheur()
      {
        assert!(!contient_declencheur("ls -la", &decls()));
      }

    #[test]
    fn la_commande_composee_est_detectee()
      {
        assert!(contient_declencheur("ls ; sudo chmod 700 .", &decls()));
      }

    #[test]
    fn le_sudo_dans_exec_est_detecte()
      {
        assert!(contient_declencheur("find / -name motd -exec sudo rm -fr {} \\;", &decls()));
      }

    #[test]
    fn le_sudo_dans_une_substitution_est_detecte()
      {
        assert!(contient_declencheur("echo $(sudo reboot)", &decls()));
      }

    #[test]
    fn un_mot_qui_contient_sudo_ne_declenche_pas()
      {
        assert!(!contient_declencheur("echo soudure", &decls()));
      }
  }
