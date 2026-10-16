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

/// Programmes ouvrant une session interactive vers une autre machine.
pub const DISTANTS: &[&str] = &["ssh", "slogin", "mosh", "telnet", "rlogin", "sshpass"];

/// Lanceurs derrière lesquels un mot de commande reste un programme réel
/// (évite de confondre `echo ssh …` avec une connexion).
const LANCEURS: &[&str] =
  &["sudo", "doas", "env", "nohup", "exec", "command", "time", "nice", "ionice", "setsid"];

/// Options courtes de `ssh`/`telnet` qui consomment l'argument suivant.
const OPTIONS_A_ARGUMENT: &[&str] =
  &["-p", "-l", "-i", "-o", "-b", "-c", "-D", "-E", "-e", "-F", "-I", "-J", "-L", "-m", "-O", "-Q", "-R", "-S", "-W", "-w", "-P"];

/// Description d'une session distante détectée sur la ligne de commande.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionDistante
  {
    /// Programme de connexion (`ssh`, `telnet`, `mosh`…).
    pub programme: String,
    /// Utilisateur distant, si connu (`user@host` ou `-l user`).
    pub utilisateur: Option<String>,
    /// Hôte distant.
    pub hote: String,
  }

/// Détecte une connexion distante dans une ligne de commande et en extrait le
/// programme, l'utilisateur (facultatif) et l'hôte. Renvoie `None` dès qu'aucun
/// programme connu n'ouvre la session.
pub fn session_distante(commande: &str) -> Option<SessionDistante>
  {
    let mots = decouper(commande);
    let mut index = mots
      .iter()
      .enumerate()
      .position(|(position, mot)| {
        DISTANTS.contains(&dernier_segment(mot))
          && (position == 0 || LANCEURS.contains(&mots[position - 1]))
      })?;
    if dernier_segment(mots[index]) == "sshpass"
      {
        let suivant = mots
          .iter()
          .enumerate()
          .skip(index + 1)
          .find(|(_, mot)| {
            let segment = dernier_segment(mot);
            DISTANTS.contains(&segment) && segment != "sshpass"
          })
          .map(|(position, _)| position);
        index = suivant?;
      }
    let programme = dernier_segment(mots[index]).to_string();
    let mut utilisateur: Option<String> = None;
    let mut i = index + 1;
    while i < mots.len()
      {
        let mot = mots[i];
        if mot == "-l"
          {
            if let Some(nom) = mots.get(i + 1)
              {
                utilisateur = Some((*nom).to_string());
                i += 2;
              }
            else
              {
                i += 1;
              }
            continue;
          }
        if mot.starts_with('-')
          {
            if OPTIONS_A_ARGUMENT.contains(&mot)
              {
                i += 2;
              }
            else
              {
                i += 1;
              }
            continue;
          }
        let (utilisateur_cible, hote) = match mot.split_once('@')
          {
            Some((nom, domaine)) if !nom.is_empty() => (Some(nom.to_string()), domaine.to_string()),
            _ => (None, mot.to_string()),
          };
        if hote.is_empty()
          {
            return None;
          }
        return Some(SessionDistante {
          programme,
          utilisateur: utilisateur_cible.or(utilisateur),
          hote,
        });
      }
    None
  }

/// Dernier segment d'un chemin (`/usr/bin/ssh` → `ssh`).
fn dernier_segment(mot: &str) -> &str
  {
    mot.rsplit('/').next().unwrap_or(mot)
  }

/// Renvoie le dernier segment de commande de la ligne, c'est-à-dire la partie
/// qui suit le dernier séparateur de haut niveau : `|` (couvre `||`, `|&`),
/// `;`, `&&` ou `&` d'arrière-plan. Les séparateurs situés entre guillemets ou
/// échappés sont ignorés, tout comme `&` au sein d'une redirection (`2>&1`,
/// `&>`). Sert à déterminer le programme réellement en cours de saisie dans une
/// commande composée (`find … | wc`).
pub fn segment_courant(commande: &str) -> &str
  {
    let octets = commande.as_bytes();
    let mut debut = 0usize;
    let mut i = 0usize;
    let mut simple = false;
    let mut double = false;
    while i < octets.len()
      {
        let c = octets[i];
        if double && c == b'\\'
          {
            i += 2;
            continue;
          }
        if !double && c == b'\''
          {
            simple = !simple;
            i += 1;
            continue;
          }
        if !simple && c == b'"'
          {
            double = !double;
            i += 1;
            continue;
          }
        if !simple && !double
          {
            let separateur = match c
              {
                b'|' | b';' => true,
                b'&' => {
                  let avant = i.checked_sub(1).map(|p| octets[p]);
                  let apres = octets.get(i + 1).copied();
                  !matches!(avant, Some(b'>') | Some(b'<'))
                    && matches!(apres, Some(b'&') | Some(b' ') | Some(b'\t') | None)
                }
                _ => false,
              };
            if separateur
              {
                debut = i + 1;
              }
          }
        i += 1;
      }
    commande[debut..].trim_start()
  }

/// Renvoie le premier mot du segment courant (le programme à documenter/aider).
pub fn programme_courant(commande: &str) -> Option<&str>
  {
    segment_courant(commande).split_whitespace().next()
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

    #[test]
    fn le_segment_suit_la_derniere_pipe()
      {
        assert_eq!(segment_courant("find / -name x -print | wc"), "wc");
        assert_eq!(segment_courant("a | b | c"), "c");
      }

    #[test]
    fn le_segment_suit_les_autres_separateurs()
      {
        assert_eq!(segment_courant("cd /tmp && wc -l f"), "wc -l f");
        assert_eq!(segment_courant("a || b ; c"), "c");
        assert_eq!(segment_courant("sleep 1 & wc"), "wc");
      }

    #[test]
    fn une_pipe_entre_guillemets_est_ignoree()
      {
        assert_eq!(segment_courant("echo \"a | b\" | wc"), "wc");
        assert_eq!(segment_courant("echo 'a ; b'"), "echo 'a ; b'");
      }

    #[test]
    fn une_redirection_ne_separe_pas()
      {
        assert_eq!(segment_courant("cmd 2>&1"), "cmd 2>&1");
        assert_eq!(segment_courant("cmd 2>&1 | wc"), "wc");
        assert_eq!(segment_courant("cmd &> f"), "cmd &> f");
      }

    #[test]
    fn sans_separateur_le_segment_est_la_ligne()
      {
        assert_eq!(segment_courant("wc -l"), "wc -l");
        assert_eq!(programme_courant("wc -l"), Some("wc"));
      }

    #[test]
    fn le_programme_courant_suit_la_pipe()
      {
        assert_eq!(programme_courant("find / -name x | wc -l"), Some("wc"));
        assert_eq!(programme_courant("  "), None);
      }

    #[test]
    fn une_connexion_ssh_simple_est_detectee()
      {
        let session = session_distante("ssh root@srv").expect("ssh détecté");
        assert_eq!(session.programme, "ssh");
        assert_eq!(session.utilisateur.as_deref(), Some("root"));
        assert_eq!(session.hote, "srv");
      }

    #[test]
    fn les_options_ssh_sont_ignorees_avant_l_hote()
      {
        let session = session_distante("ssh -p 2222 deploy@srv -X").expect("ssh détecté");
        assert_eq!(session.utilisateur.as_deref(), Some("deploy"));
        assert_eq!(session.hote, "srv");
      }

    #[test]
    fn l_option_l_fournit_l_utilisateur()
      {
        let session = session_distante("ssh -l deploy srv").expect("ssh détecté");
        assert_eq!(session.utilisateur.as_deref(), Some("deploy"));
        assert_eq!(session.hote, "srv");
      }

    #[test]
    fn telnet_et_lanceur_sont_reconnus()
      {
        let session = session_distante("sudo telnet routeur").expect("telnet détecté");
        assert_eq!(session.programme, "telnet");
        assert_eq!(session.utilisateur, None);
        assert_eq!(session.hote, "routeur");
      }

    #[test]
    fn sshpass_retient_ssh_et_sa_cible()
      {
        let session = session_distante("sshpass -p secret ssh user@srv").expect("ssh détecté");
        assert_eq!(session.programme, "ssh");
        assert_eq!(session.utilisateur.as_deref(), Some("user"));
        assert_eq!(session.hote, "srv");
      }

    #[test]
    fn un_mot_ssh_argument_ne_declenche_pas()
      {
        assert_eq!(session_distante("echo ssh bonjour"), None);
        assert_eq!(session_distante("ls -la"), None);
        assert_eq!(session_distante("ssh"), None);
      }
  }
