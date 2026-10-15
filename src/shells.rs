//! Adaptateurs par shell : templates de prompt, marqueurs OSC 133, édition
//! mono-ligne (`horizontal-scroll-mode`, `singlelinezle`, PSReadLine).

/// Shells pris en charge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell
  {
    /// GNU Bash.
    Bash,
    /// Z shell.
    Zsh,
    /// Shell POSIX (`sh`, dash, ash).
    Sh,
    /// PowerShell (Windows et multiplateforme).
    PowerShell,
    /// Interpréteur de commandes Windows.
    Cmd,
  }

impl Shell
  {
    /// Tous les shells pris en charge.
    pub const TOUS: [Shell; 5] =
      [Shell::Bash, Shell::Zsh, Shell::Sh, Shell::PowerShell, Shell::Cmd];

    /// Nom canonique du shell.
    pub fn nom(self) -> &'static str
      {
        match self
          {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Sh => "sh",
            Shell::PowerShell => "powershell",
            Shell::Cmd => "cmd",
          }
      }

    /// Résout un shell par nom (insensible à la casse).
    pub fn par_nom(nom: &str) -> Option<Shell>
      {
        Self::TOUS.iter().copied().find(|s| s.nom() == nom.to_ascii_lowercase())
      }

    /// Nom du programme exécutable.
    pub fn programme(self) -> &'static str
      {
        match self
          {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Sh => "sh",
            Shell::PowerShell => "pwsh",
            Shell::Cmd => "cmd",
          }
      }

    /// Nom du programme à lancer, en tenant compte de la plateforme.
    ///
    /// PowerShell : `pwsh` (PowerShell 7+) s'il est présent dans le `PATH`,
    /// sinon `powershell` (Windows PowerShell, fourni avec Windows). Ailleurs,
    /// le nom canonique de [`Shell::programme`].
    pub fn programme_lance(self) -> std::ffi::OsString
      {
        if self == Shell::PowerShell && !dans_path("pwsh")
          {
            return std::ffi::OsString::from("powershell");
          }
        std::ffi::OsString::from(self.programme())
      }

    /// Déduit le shell courant de `$SHELL` (repli : PowerShell sous Windows,
    /// bash ailleurs).
    pub fn depuis_environnement() -> Shell
      {
        Self::depuis_valeurs(std::env::var("SHELL").ok().as_deref(), std::env::consts::OS)
      }

    /// Version pure et testable de [`Shell::depuis_environnement`].
    ///
    /// `shell` est la valeur brute de `$SHELL` (chemin éventuel), `os` la valeur
    /// de `std::env::consts::OS` (`"windows"`, `"linux"`, …).
    pub fn depuis_valeurs(shell: Option<&str>, os: &str) -> Shell
      {
        if let Some(chemin) = shell
          {
            let base = chemin.rsplit(['/', '\\']).next().unwrap_or("");
            let base = base.strip_suffix(".exe").unwrap_or(base);
            if let Some(shell) = Self::par_nom(base)
              {
                return shell;
              }
          }
        if os == "windows"
          {
            Shell::PowerShell
          }
        else
          {
            Shell::Bash
          }
      }
  }

/// Indique si un exécutable (nu ou suffixé `.exe`/`.cmd`) est présent dans le
/// `PATH`.
fn dans_path(programme: &str) -> bool
  {
    let Some(chemin) = std::env::var_os("PATH")
      else
        {
          return false;
        };
    let noms = [programme.to_string(), format!("{programme}.exe"), format!("{programme}.cmd")];
    std::env::split_paths(&chemin).any(|dossier| noms.iter().any(|nom| dossier.join(nom).is_file()))
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn les_noms_resolvent_les_shells()
      {
        assert_eq!(Shell::par_nom("bash"), Some(Shell::Bash));
        assert_eq!(Shell::par_nom("PowerShell"), Some(Shell::PowerShell));
        assert_eq!(Shell::par_nom("inconnu"), None);
      }

    #[test]
    fn le_shell_par_defaut_depend_de_l_os()
      {
        assert_eq!(Shell::depuis_valeurs(None, "windows"), Shell::PowerShell);
        assert_eq!(Shell::depuis_valeurs(None, "linux"), Shell::Bash);
      }

    #[test]
    fn la_variable_shell_prime_avec_ou_sans_chemin_ni_exe()
      {
        assert_eq!(Shell::depuis_valeurs(Some("/usr/bin/zsh"), "linux"), Shell::Zsh);
        assert_eq!(Shell::depuis_valeurs(Some("C:\\Program Files\\PowerShell\\pwsh.exe"), "windows"), Shell::PowerShell);
        assert_eq!(Shell::depuis_valeurs(Some("/bin/sh"), "linux"), Shell::Sh);
      }

    #[test]
    fn une_variable_shell_vide_retombe_sur_le_defaut()
      {
        assert_eq!(Shell::depuis_valeurs(Some(""), "windows"), Shell::PowerShell);
        assert_eq!(Shell::depuis_valeurs(Some("inconnu"), "linux"), Shell::Bash);
      }
  }
