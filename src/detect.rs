//! Détection de l'environnement : OS, shell, terminal, privilège, locale.

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
pub fn est_administrateur() -> bool
  {
    let administration = groupes_administration();
    if administration.is_empty()
      {
        return false;
      }
    groupes().iter().any(|groupe| administration.contains(&groupe.as_str()))
  }

/// Détermine le privilège courant.
///
/// `uid == 0` → `Root` ; appartenance au groupe d'administration → `Wheel` ;
/// sinon `Classique`.
pub fn privilege() -> Privilege
  {
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
