//! Détection de l'environnement : OS, shell, terminal, privilège, locale.

/// Décision de démarrage du cadre, calculée avant `bashop run`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demarrage
  {
    /// Le cadre peut être lancé.
    Cadre,
    /// Bashop est déjà actif (env) ou présent dans les ancêtres.
    DejaActif,
    /// Un multiplexeur (`tmux`/`screen`) occupe le terminal.
    Multiplexeur,
    /// La sortie standard n'est pas un terminal interactif.
    SansTty,
  }

/// Nom du binaire recherché dans la chaîne des ancêtres.
const PROGRAMME: &str = "bashop";

/// Indique si l'environnement porte le marqueur d'activation.
pub fn deja_actif() -> bool
  {
    std::env::var_os("BASHOP_ACTIVE").is_some_and(|v| v == "1")
  }

/// Indique si un multiplexeur occupe le terminal.
pub fn sous_multiplexeur() -> bool
  {
    std::env::var_os("TMUX").is_some() || std::env::var_os("STY").is_some()
  }

/// Identifiant du processus parent, lu au mieux.
#[cfg(target_os = "linux")]
fn parent_de(pid: u32) -> Option<u32>
  {
    let contenu = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for ligne in contenu.lines()
      {
        if let Some(valeur) = ligne.strip_prefix("PPid:")
          {
            return valeur.trim().parse::<u32>().ok();
          }
      }
    None
  }

/// Identifiant du processus parent via `ps` (Unix sans `/proc`).
#[cfg(all(unix, not(target_os = "linux")))]
fn parent_de(pid: u32) -> Option<u32>
  {
    let sortie = std::process::Command::new("ps")
      .args(["-o", "ppid=", "-p", &pid.to_string()])
      .output()
      .ok()?;
    String::from_utf8_lossy(&sortie.stdout).trim().parse::<u32>().ok()
  }

/// Pas d'inspection de processus hors Unix.
#[cfg(not(unix))]
fn parent_de(_pid: u32) -> Option<u32>
  {
    None
  }

/// Nom du programme d'un processus, lu au mieux.
#[cfg(target_os = "linux")]
fn nom_processus(pid: u32) -> Option<String>
  {
    let nom = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let nom = nom.trim();
    if nom.is_empty()
      {
        return None;
      }
    Some(nom.to_string())
  }

/// Nom du programme d'un processus via `ps`.
#[cfg(all(unix, not(target_os = "linux")))]
fn nom_processus(pid: u32) -> Option<String>
  {
    let sortie = std::process::Command::new("ps")
      .args(["-o", "comm=", "-p", &pid.to_string()])
      .output()
      .ok()?;
    let nom = String::from_utf8_lossy(&sortie.stdout).trim().to_string();
    if nom.is_empty()
      {
        None
      }
    else
      {
        Some(nom)
      }
  }

/// Pas d'inspection de processus hors Unix.
#[cfg(not(unix))]
fn nom_processus(_pid: u32) -> Option<String>
  {
    None
  }

/// Remonte la chaîne des ancêtres et indique si l'un d'eux est Bashop.
pub fn ancetre_bashop() -> bool
  {
    let mut pid = std::process::id();
    for _ in 0..64
      {
        let Some(ppid) = parent_de(pid)
          else
            {
              return false;
            };
        if ppid <= 1 || ppid == pid
          {
            return false;
          }
        if nom_processus(ppid).is_some_and(|nom| nom.contains(PROGRAMME))
          {
            return true;
          }
        pid = ppid;
      }
    false
  }

/// Décide si le cadre peut être lancé dans le contexte courant.
pub fn decider() -> Demarrage
  {
    if deja_actif()
      {
        return Demarrage::DejaActif;
      }
    if sous_multiplexeur()
      {
        return Demarrage::Multiplexeur;
      }
    if ancetre_bashop()
      {
        return Demarrage::DejaActif;
      }
    use std::io::IsTerminal;
    if !std::io::stdout().is_terminal()
      {
        return Demarrage::SansTty;
      }
    Demarrage::Cadre
  }

/// Indique si le cadre doit être lancé (utilisé par la sous-commande `_guard`).
pub fn autoriser_cadre() -> bool
  {
    matches!(decider(), Demarrage::Cadre)
  }

/// Niveau de privilège effectif, qui pilote le thème par défaut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Privilege
  {
    /// Utilisateur normal.
    Classique,
    /// Membre du groupe d'administration (wheel / sudo / admin / opérateurs).
    Wheel,
    /// Superutilisateur (root).
    Root,
  }

/// Noms du ou des groupes d'administration selon l'OS hôte.
fn groupes_administration() -> &'static [&'static str]
  {
    #[cfg(target_os = "macos")]
      {
        &["admin", "wheel"]
      }
    #[cfg(target_os = "freebsd")]
      {
        &["wheel", "operator"]
      }
    #[cfg(target_os = "windows")]
      {
        &[]
      }
    #[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "windows")))]
      {
        &["wheel", "sudo", "admin", "adm"]
      }
  }

/// Liste des noms de groupes de l'utilisateur courant (Unix, best-effort).
#[cfg(unix)]
fn groupes() -> Vec<String>
  {
    let Ok(sortie) = std::process::Command::new("id").arg("-Gn").output() else
      {
        return Vec::new();
      };
    if !sortie.status.success()
      {
        return Vec::new();
      }
    String::from_utf8_lossy(&sortie.stdout).split_whitespace().map(str::to_string).collect()
  }

#[cfg(not(unix))]
fn groupes() -> Vec<String>
  {
    Vec::new()
  }

/// Indique si l'utilisateur appartient au groupe d'administration de l'OS.
///
/// Le résultat est mis en cache : l'appartenance aux groupes ne change pas
/// pendant la vie du processus (une reprise sous `sudo -i` démarre un nouveau
/// shell, donc un nouveau calcul par publication OSC).
pub fn est_administrateur() -> bool
  {
    static CACHE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CACHE.get_or_init(|| {
      let administration = groupes_administration();
      if administration.is_empty()
        {
          return false;
        }
      groupes().iter().any(|groupe| administration.contains(&groupe.as_str()))
    })
  }

/// Détermine le privilège courant (mis en cache).
///
/// `uid == 0` → `Root` ; appartenance au groupe d'administration → `Wheel` ;
/// sinon `Classique`.
pub fn privilege() -> Privilege
  {
    static CACHE: std::sync::OnceLock<Privilege> = std::sync::OnceLock::new();
    *CACHE.get_or_init(|| {
      #[cfg(unix)]
        {
          let uid = unsafe { libc::getuid() };
          if uid == 0
            {
              return Privilege::Root;
            }
        }
      if est_administrateur()
        {
          Privilege::Wheel
        }
      else
        {
          Privilege::Classique
        }
    })
  }

/// Indique si l'on tourne sous un vrai superutilisateur.
pub fn est_root() -> bool
  {
    matches!(privilege(), Privilege::Root)
  }

/// Résumé textuel, affiché par `bashop detect`.
pub fn resume() -> String
  {
    format!(
      "bashop {} · os={} · arch={} · privilège={:?}",
      crate::VERSION,
      std::env::consts::OS,
      std::env::consts::ARCH,
      privilege()
    )
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn le_processus_courant_a_un_parent()
      {
        assert!(parent_de(std::process::id()).is_some());
      }

    #[test]
    fn la_decision_de_demarrage_est_definie()
      {
        let decision = decider();
        assert!(matches!(
          decision,
          Demarrage::Cadre | Demarrage::DejaActif | Demarrage::Multiplexeur | Demarrage::SansTty
        ));
        assert_eq!(autoriser_cadre(), decision == Demarrage::Cadre);
      }

    #[test]
    fn un_nom_de_processus_inconnu_ne_remonte_pas_bashop()
      {
        assert!(!nom_processus(0).is_some_and(|nom| nom.contains(PROGRAMME)));
      }
  }
