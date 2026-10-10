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

    /// Déduit le shell courant de `$SHELL` (repli sur bash).
    pub fn depuis_environnement() -> Shell
      {
        let chemin = std::env::var("SHELL").unwrap_or_default();
        let base = chemin.rsplit('/').next().unwrap_or("");
        Self::par_nom(base).unwrap_or(Shell::Bash)
      }
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
  }
