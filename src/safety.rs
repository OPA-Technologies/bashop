//! Garde-fous de saisie (gating de la touche Entrée au prompt).
//!
//! L'analyse est volontairement explicable : chaque motif déclenché est
//! renvoyé, pour que la confirmation affiche *pourquoi* elle s'interpose.

use crate::lexer;

/// Niveau de risque d'une ligne de commande.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Niveau
  {
    /// Aucun risque détecté.
    Sur,
    /// À signaler sans bloquer.
    Avertissement,
    /// Nécessite une confirmation avant exécution.
    Danger,
  }

/// Résultat de l'évaluation d'une ligne.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict
  {
    /// Niveau de risque.
    pub niveau: Niveau,
    /// Motifs ayant conduit au verdict.
    pub motifs: Vec<String>,
  }

impl Verdict
  {
    /// Verdict sûr, sans motif.
    pub fn sur() -> Self
      {
        Self { niveau: Niveau::Sur, motifs: Vec::new() }
      }
  }

fn est_danger(motif: &str) -> bool
  {
    motif.contains("suppression")
      || motif.contains("bombe")
      || motif.contains("périphérique")
      || motif.contains("disque")
  }

/// Évalue une ligne de commande.
pub fn evaluer(commande: &str) -> Verdict
  {
    let ligne = commande.trim();
    if ligne.is_empty()
      {
        return Verdict::sur();
      }
    let mots = lexer::decouper(ligne);
    let mut motifs: Vec<String> = Vec::new();

    let sans_espaces: String = ligne.chars().filter(|c| !c.is_whitespace()).collect();
    if sans_espaces.contains(":(){:|:&};:")
      {
        motifs.push("bombe à fork".to_string());
      }

    if mots.contains(&"rm")
      {
        let drapeaux: String =
          mots.iter().filter(|m| m.starts_with('-')).flat_map(|m| m.chars()).collect();
        let recursif = drapeaux.contains('r') || drapeaux.contains('R');
        let force = drapeaux.contains('f');
        let cible_racine = mots
          .iter()
          .any(|m| matches!(*m, "/" | "/*" | "~" | "~/" | "/etc" | "/usr" | "/var" | "/home"));
        if recursif && force && cible_racine
          {
            motifs.push("suppression récursive forcée d'une racine".to_string());
          }
        else if recursif && cible_racine
          {
            motifs.push("suppression récursive d'une racine".to_string());
          }
      }

    if mots.contains(&"dd")
      && mots.iter().any(|m| m.starts_with("of=/dev/"))
      {
        motifs.push("écriture directe sur un périphérique (dd)".to_string());
      }

    if ligne.contains("> /dev/sd")
      || ligne.contains(">/dev/sd")
      || ligne.contains("> /dev/nvme")
      || ligne.contains(">/dev/nvme")
      {
        motifs.push("redirection vers un disque".to_string());
      }

    if mots.iter().any(|m| matches!(*m, "shutdown" | "reboot" | "poweroff" | "halt"))
      {
        motifs.push("arrêt ou redémarrage du système".to_string());
      }

    if mots.contains(&"chmod") || mots.contains(&"chown")
      {
        let recursif = mots.iter().any(|m| m.starts_with('-') && m.contains('R'));
        if recursif && mots.iter().any(|m| matches!(*m, "/" | "/etc" | "/usr"))
          {
            motifs.push("changement récursif de permissions sur une racine".to_string());
          }
      }

    let minuscule = ligne.to_ascii_lowercase();
    if ["password=", "--password", "token=", "secret=", "api_key="]
      .iter()
      .any(|m| minuscule.contains(m))
      {
        motifs.push("secret en clair dans la ligne".to_string());
      }

    if mots.first().copied() == Some("rsync")
      && mots.iter().any(|m| matches!(*m, "--delete" | "--del"))
      {
        motifs.push("rsync avec écrasement à la destination (--delete)".to_string());
      }

    if mots.first().copied() == Some("git")
      && mots.get(1).copied() == Some("clean")
      && mots.iter().any(|m| m.starts_with('-') && m.contains('f'))
      {
        motifs.push("git clean force (effacement de fichiers non suivis)".to_string());
      }

    let niveau = if motifs.iter().any(|m| est_danger(m))
      {
        Niveau::Danger
      }
    else if motifs.is_empty()
      {
        Niveau::Sur
      }
    else
      {
        Niveau::Avertissement
      };
    Verdict { niveau, motifs }
  }

/// Commande équivalente en **simulation** (`--dry-run` / `-n`) quand le
/// programme en propose une, sinon `None`.
///
/// La liste est volontairement restreinte aux programmes dont l'option de
/// simulation est stable et sans effet de bord (`rsync`, `make`, `apt`,
/// `apt-get`, `pip`, `pip3`, `npm`, `git clean`). Le reste de la ligne est
/// conservé intact (espaces, guillemets, redirections).
pub fn simulation(commande: &str) -> Option<String>
  {
    let ligne = commande.trim();
    let mots = lexer::decouper(ligne);
    let programme = *mots.first()?;
    let (position, option) = match programme
      {
        "rsync" | "pip" | "pip3" | "npm" => (0, "--dry-run"),
        "make" => (0, "-n"),
        "apt" | "apt-get" => (0, "-s"),
        "git" if mots.get(1).copied() == Some("clean") => (1, "--dry-run"),
        _ => return None,
      };
    Some(inserer_apres_jeton(ligne, position, option))
  }

/// Insère `option` juste après le `jeton`-ième mot (base 0) de `ligne`, sans
/// altérer le reste de la chaîne.
fn inserer_apres_jeton(ligne: &str, jeton: usize, option: &str) -> String
  {
    let mut vus = 0usize;
    let mut dans_mot = false;
    for (i, c) in ligne.char_indices()
      {
        if c.is_whitespace()
          {
            if dans_mot
              {
                if vus == jeton
                  {
                    return format!("{} {}{}", &ligne[..i], option, &ligne[i..]);
                  }
                vus += 1;
                dans_mot = false;
              }
          }
        else
          {
            dans_mot = true;
          }
      }
    format!("{ligne} {option}")
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn une_commande_ordinaire_est_sure()
      {
        assert_eq!(evaluer("ls -la /tmp"), Verdict::sur());
      }

    #[test]
    fn rm_rf_sur_la_racine_est_dangereux()
      {
        let verdict = evaluer("rm -rf /");
        assert_eq!(verdict.niveau, Niveau::Danger);
        assert!(!verdict.motifs.is_empty());
      }

    #[test]
    fn un_arret_est_un_avertissement()
      {
        assert_eq!(evaluer("shutdown -h now").niveau, Niveau::Avertissement);
      }

    #[test]
    fn un_secret_en_clair_est_signale()
      {
        assert_eq!(evaluer("export TOKEN=abc").niveau, Niveau::Avertissement);
      }

    #[test]
    fn une_bombe_a_fork_est_dangereuse()
      {
        assert_eq!(evaluer(":(){ :|:& };:").niveau, Niveau::Danger);
      }

    #[test]
    fn rsync_propose_une_simulation_dry_run()
      {
        assert_eq!(
          simulation("rsync --delete -a src/ dst/"),
          Some("rsync --dry-run --delete -a src/ dst/".to_string())
        );
      }

    #[test]
    fn make_apt_pip_proposent_leur_option_de_simulation()
      {
        assert_eq!(simulation("make -j4"), Some("make -n -j4".to_string()));
        assert_eq!(simulation("apt-get remove foo"), Some("apt-get -s remove foo".to_string()));
        assert_eq!(simulation("pip install bar"), Some("pip --dry-run install bar".to_string()));
      }

    #[test]
    fn git_clean_insere_l_option_apres_la_sous_commande()
      {
        assert_eq!(
          simulation("git clean -fdx"),
          Some("git clean --dry-run -fdx".to_string())
        );
      }

    #[test]
    fn une_commande_non_simulable_n_a_pas_de_simulation()
      {
        assert_eq!(simulation("ls -la"), None);
        assert_eq!(simulation("rm -rf /"), None);
        assert_eq!(simulation("git status"), None);
      }

    #[test]
    fn rsync_delete_et_git_clean_force_sont_signales()
      {
        assert_eq!(evaluer("rsync --delete -a src/ dst/").niveau, Niveau::Avertissement);
        assert_eq!(evaluer("git clean -fdx").niveau, Niveau::Avertissement);
      }
  }
