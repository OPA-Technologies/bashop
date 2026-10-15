//! Injection idempotente du bootstrap dans les rc des shells (`bashop install`).
//!
//! Le bloc est encadré par des marqueurs ; l'installation retire d'abord un
//! éventuel bloc existant, ce qui rend l'opération ré-entrante. Aucune
//! commande shell n'est exécutée : seuls des fichiers rc sont modifiés.

use std::path::PathBuf;

use crate::bootstrap::{
  MARQUE_DEBUT, MARQUE_FIN, bloc_bash, bloc_powershell, bloc_systeme, bloc_zsh, prompt_cmd,
};
use crate::shells::Shell;

/// Chemin du bloc système sous Linux (sourcé par les shells de connexion).
pub const CHEMIN_SYSTEME: &str = "/etc/profile.d/bashop.sh";

/// Nom du fichier `AutoRun` écrit pour cmd.exe (Windows).
pub const FICHIER_CMD: &str = "bashop-cmd.cmd";

/// Clé de registre `AutoRun` de cmd.exe (HKCU, utilisateur courant).
#[cfg(target_os = "windows")]
const CLE_AUTORUN: &str = r"HKCU\Software\Microsoft\Command Processor";

/// Chemin du fichier rc principal d'un shell (best-effort).
pub fn rc_principal(shell: Shell) -> Option<PathBuf>
  {
    let home = crate::detect::dossier_utilisateur()?;
    match shell
      {
        Shell::Bash => Some(home.join(".bashrc")),
        Shell::Zsh => Some(home.join(".zshrc")),
        Shell::Sh => Some(home.join(".profile")),
        Shell::PowerShell => {
          #[cfg(target_os = "windows")]
            {
              Some(home.join("Documents").join("PowerShell").join("Microsoft.PowerShell_profile.ps1"))
            }
          #[cfg(not(target_os = "windows"))]
            {
              Some(home.join(".config").join("powershell").join("Microsoft.PowerShell_profile.ps1"))
            }
        }
        Shell::Cmd => None,
      }
  }

/// Bloc d'activation à injecter pour un shell (aucun pour `sh`/cmd, qui n'ont
/// pas de fichier rc ; cmd passe par `AutoRun`).
pub fn bloc(shell: Shell) -> Option<String>
  {
    match shell
      {
        Shell::Bash => Some(bloc_bash()),
        Shell::Zsh => Some(bloc_zsh()),
        Shell::PowerShell => Some(bloc_powershell()),
        Shell::Sh | Shell::Cmd => None,
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
///
/// Selon l'OS : bash/zsh/sh sur Unix, PowerShell sous Windows (cmd n'a pas de
/// fichier rc et reste une cible explicite via `AutoRun`).
pub fn shells_cibles() -> Vec<Shell>
  {
    let ordre: [Shell; 3] = if cfg!(target_os = "windows")
      {
        [Shell::PowerShell, Shell::Cmd, Shell::Bash]
      }
    else
      {
        [Shell::Bash, Shell::Zsh, Shell::Sh]
      };
    ordre
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
        Some(Shell::Cmd) => return installer_cmd(),
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
    if shell.is_none()
      {
        touches.extend(installer_cmd());
      }
    touches
  }

/// Contenu du fichier `AutoRun` de cmd.exe (Windows) : pose `PROMPT` (marqueurs
/// `OSC 133;A/B`, couleurs, chemin `$P`, heure `$T`). L'épinglage est omis (la
/// hauteur n'est pas connue à l'installation). Les `%` sont dédoublés pour que
/// cmd les expanse à chaque invite plutôt qu'à la lecture du fichier.
pub fn contenu_cmd_autorun() -> String
  {
    let config = crate::config::charger();
    let theme = crate::theme::resoudre(&config.theme, crate::detect::privilege());
    let palette = crate::theme::palette(&theme, &config.couleurs);
    contenu_cmd_autorun_pour(&palette, theme.suffixe.glyphe, config.suffix_width)
  }

/// Variante pure (testable) de [`contenu_cmd_autorun`].
pub fn contenu_cmd_autorun_pour(
    palette: &crate::theme::Palette,
    glyphe: &str,
    largeur: u16,
  ) -> String
  {
    let prompt = prompt_cmd(palette, glyphe, largeur, None).replace('%', "%%");
    format!("@echo off\r\nset \"PROMPT={prompt}\"\r\n")
  }

/// Installe le bloc cmd via `AutoRun` (Windows). Écrit le fichier et pointe la
/// clé `HKCU\Software\Microsoft\Command Processor\AutoRun` dessus.
#[cfg(target_os = "windows")]
fn installer_cmd() -> Vec<PathBuf>
  {
    let Some(home) = crate::detect::dossier_utilisateur()
      else
        {
          return Vec::new();
        };
    let dossier = home.join(".bashop");
    if std::fs::create_dir_all(&dossier).is_err()
      {
        return Vec::new();
      }
    let fichier = dossier.join(FICHIER_CMD);
    if std::fs::write(&fichier, contenu_cmd_autorun()).is_err()
      {
        return Vec::new();
      }
    let valeur = format!("\"{}\"", fichier.display());
    let ok = std::process::Command::new("reg")
      .args(["add", CLE_AUTORUN, "/v", "AutoRun", "/t", "REG_SZ", "/d", &valeur, "/f"])
      .status()
      .map(|statut| statut.success())
      .unwrap_or(false);
    if ok
      {
        vec![fichier]
      }
    else
      {
        Vec::new()
      }
  }

/// Hors Windows, l'installation cmd n'est pas prise en charge.
#[cfg(not(target_os = "windows"))]
fn installer_cmd() -> Vec<PathBuf>
  {
    Vec::new()
  }

/// Retire le bloc de tous les rc connus. Renvoie les fichiers touchés.
pub fn desinstaller() -> Vec<PathBuf>
  {
    let mut touches = Vec::new();
    for cible in [Shell::Bash, Shell::Zsh, Shell::Sh, Shell::PowerShell]
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
    #[cfg(target_os = "windows")]
      {
        let _ = desinstaller_cmd();
      }
    let systeme = PathBuf::from(CHEMIN_SYSTEME);
    if let Ok(contenu) = std::fs::read_to_string(&systeme)
      {
        let nouveau = retirer_bloc(&contenu);
        if nouveau != contenu && std::fs::write(&systeme, nouveau).is_ok()
          {
            touches.push(systeme);
          }
      }
    touches
  }

/// Retire l'`AutoRun` cmd (Windows) et le fichier associé.
#[cfg(target_os = "windows")]
fn desinstaller_cmd() -> Vec<PathBuf>
  {
    let mut touches = Vec::new();
    std::process::Command::new("reg")
      .args(["delete", CLE_AUTORUN, "/v", "AutoRun", "/f"])
      .status()
      .ok();
    if let Some(home) = crate::detect::dossier_utilisateur()
      {
        let fichier = home.join(".bashop").join(FICHIER_CMD);
        if std::fs::remove_file(&fichier).is_ok()
          {
            touches.push(fichier);
          }
      }
    touches
  }

/// Contenu après injection idempotente du bloc système.
pub fn injecter_bloc_systeme(contenu: &str) -> String
  {
    let mut sortie = retirer_bloc(contenu).trim_end().to_string();
    sortie.push_str("\n\n");
    sortie.push_str(&bloc_systeme());
    sortie
  }

/// Installe le bloc système dans le fichier `chemin` (créé si absent).
pub fn installer_systeme_dans(chemin: &std::path::Path) -> std::io::Result<()>
  {
    let contenu = std::fs::read_to_string(chemin).unwrap_or_default();
    let nouveau = injecter_bloc_systeme(&contenu);
    if nouveau != contenu
      {
        std::fs::write(chemin, nouveau)?;
      }
    Ok(())
  }

/// Installe le bloc système sous Linux (`/etc/profile.d/bashop.sh`).
///
/// Best-effort et **Linux uniquement** : hors Linux, renvoie une erreur
/// `Unsupported`. L'écriture requiert les droits root.
#[cfg(target_os = "linux")]
pub fn installer_systeme() -> std::io::Result<PathBuf>
  {
    let chemin = PathBuf::from(CHEMIN_SYSTEME);
    installer_systeme_dans(&chemin)?;
    Ok(chemin)
  }

/// Hors Linux, l'installation système n'est pas prise en charge.
#[cfg(not(target_os = "linux"))]
pub fn installer_systeme() -> std::io::Result<PathBuf>
  {
    Err(std::io::Error::new(
      std::io::ErrorKind::Unsupported,
      "installation système non prise en charge hors Linux",
    ))
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
        assert_eq!(injecter_bloc("contenu", Shell::Sh), "contenu");
      }

    #[test]
    fn le_profil_powershell_s_injecte_idempotent_et_se_retire()
      {
        let initial = "# profil utilisateur\n";
        let une = injecter_bloc(initial, Shell::PowerShell);
        assert!(une.contains(MARQUE_DEBUT));
        assert!(une.contains("bashop run --shell powershell"));
        assert!(une.contains("# profil utilisateur"));
        let deux = injecter_bloc(&une, Shell::PowerShell);
        assert_eq!(deux, une, "l'injection PowerShell doit être idempotente");
        let restaure = retirer_bloc(&deux);
        assert_eq!(restaure.trim_end(), initial.trim_end());
      }

    #[test]
    fn le_fichier_autorun_cmd_pose_le_prompt_avec_marqueurs()
      {
        let palette = crate::theme::palette(&crate::theme::CLASSIQUE, &crate::theme::Surcharge::default());
        let contenu = contenu_cmd_autorun_pour(&palette, ">>", 6);
        assert!(contenu.starts_with("@echo off"));
        assert!(contenu.contains("set \"PROMPT="));
        assert!(contenu.contains("133;A"), "marqueur A absent : {contenu}");
        assert!(contenu.contains("133;B"), "marqueur B absent : {contenu}");
        assert!(contenu.contains("$P"), "chemin cmd ($P) absent : {contenu}");
        assert!(contenu.contains("%%USERNAME%%"), "les % doivent être dédoublés : {contenu}");
      }

    #[test]
    fn le_bloc_systeme_s_injecte_idempotent_et_se_retire()
      {
        let initial = "# fichier profile.d quelconque\n";
        let une = injecter_bloc_systeme(initial);
        assert!(une.contains(MARQUE_DEBUT));
        assert!(une.contains("bashop _nested"));
        assert!(une.contains("# fichier profile.d quelconque"));
        let deux = injecter_bloc_systeme(&une);
        assert_eq!(deux, une, "l'injection système doit être idempotente");
        let restaure = retirer_bloc(&deux);
        assert_eq!(restaure.trim_end(), initial.trim_end());
      }

    #[test]
    fn installer_systeme_dans_un_fichier_temporaire()
      {
        let dossier = std::env::temp_dir().join(format!("bashop-sys-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join("bashop.sh");
        installer_systeme_dans(&chemin).unwrap();
        let contenu = std::fs::read_to_string(&chemin).unwrap();
        assert!(contenu.contains(MARQUE_DEBUT));
        assert!(contenu.contains("bashop prompt --shell bash"));
        installer_systeme_dans(&chemin).unwrap();
        let relu = std::fs::read_to_string(&chemin).unwrap();
        assert_eq!(relu.matches(MARQUE_DEBUT).count(), 1, "pas de double bloc système");
        let _ = std::fs::remove_file(&chemin);
        let _ = std::fs::remove_dir(dossier);
      }
  }
