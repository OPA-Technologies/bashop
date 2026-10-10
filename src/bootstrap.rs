//! Génération du prompt et du fichier de démarrage injectés dans le shell.
//!
//! Le shell conserve son édition native (modèle B) : on ne réimplémente pas la
//! saisie, on lui fournit un prompt qui publie les marqueurs `OSC 133` et
//! applique les couleurs du thème. Le mode mono-ligne est forcé ici.

use crate::shells::Shell;
use crate::theme::Theme;

/// Marqueur de début de prompt (OSC 133;A).
const OSC_PROMPT: &str = "\\e]133;A\\e\\\\";
/// Marqueur de début de commande (OSC 133;B).
const OSC_COMMANDE: &str = "\\e]133;B\\e\\\\";

fn couleur(rgb: (u8, u8, u8), gras: bool) -> String
  {
    let intensite = if gras { 1 } else { 0 };
    format!("\\e[{};38;2;{};{};{}m", intensite, rgb.0, rgb.1, rgb.2)
  }

fn couleur_italique(rgb: (u8, u8, u8)) -> String
  {
    format!("\\e[3;38;2;{};{};{}m", rgb.0, rgb.1, rgb.2)
  }

const REINIT: &str = "\\e[0m";
const COULEUR_CHEMIN: (u8, u8, u8) = (0x89, 0xB4, 0xFA);
const COULEUR_HISTORIQUE: (u8, u8, u8) = (0x6C, 0x70, 0x86);
const COULEUR_HEURE: (u8, u8, u8) = (0x8B, 0xA1, 0xB9);
/// Épingle le prompt sur la dernière ligne de la zone du shell (avant-dernière
/// ligne de l'écran) puis efface la fin de ligne.
const PIN: &str = "\\e[${LINES};1H\\e[K";
/// Variante zsh de l'épinglage (délimiteurs `%{ %}`).
const PIN_ZSH: &str = "%{\\e[${LINES};1H\\e[K%}";

/// Cadre (cadrage à droite) du suffixe de prompt sur la largeur normalisée.
pub fn suffixe_cadre(theme: &Theme, largeur: u16) -> String
  {
    format!("{:>largeur$}", theme.suffixe.glyphe, largeur = largeur as usize)
  }

/// Construit la chaîne `PS1` de bash pour un thème donné.
///
/// La ligne affiche l'heure de début de commande (`__BASHOP_HEURE`), `user@host`,
/// le chemin absolu (`${PWD}`), le numéro de commande entre parenthèses
/// (`__BASHOP_HIST`, gris italique, cadré à droite) et le suffixe.
pub fn prompt_bash(theme: &Theme, largeur_suffixe: u16) -> String
  {
    let accent = (theme.accent.r, theme.accent.g, theme.accent.b);
    let suffixe = (theme.suffixe.couleur.r, theme.suffixe.couleur.g, theme.suffixe.couleur.b);
    format!(
      "\\[{pin}\\]\\[{osc_prompt}\\]\\[{heure}\\]${{__BASHOP_HEURE}}\\[{reinit}\\] \\[{user}\\]\\u@\\h\\[{reinit}\\] \\[{chemin}\\]${{PWD}}\\[{reinit}\\] \\[{hist}\\]${{__BASHOP_HIST}}\\[{reinit}\\] \\[{suffixe_col}\\]{suffixe}\\[{reinit}\\] \\[{osc_commande}\\]",
      pin = PIN,
      osc_prompt = OSC_PROMPT,
      heure = couleur_italique(COULEUR_HEURE),
      user = couleur(accent, true),
      reinit = REINIT,
      chemin = couleur(COULEUR_CHEMIN, false),
      hist = couleur_italique(COULEUR_HISTORIQUE),
      suffixe_col = couleur(suffixe, true),
      suffixe = suffixe_cadre(theme, largeur_suffixe),
      osc_commande = OSC_COMMANDE,
    )
  }

/// Construit la chaîne `PROMPT` de zsh pour un thème donné.
pub fn prompt_zsh(theme: &Theme, largeur_suffixe: u16) -> String
  {
    let accent = (theme.accent.r, theme.accent.g, theme.accent.b);
    let suffixe = (theme.suffixe.couleur.r, theme.suffixe.couleur.g, theme.suffixe.couleur.b);
    let accent_hex = format!("#{:02x}{:02x}{:02x}", accent.0, accent.1, accent.2);
    format!(
      "{pin}%{{\\e[3;38;2;{heure_r};{heure_g};{heure_b}m%}}${{__BASHOP_HEURE}}%{{\\e[0m%}} %{{%B%F{{{accent}}}%}}%n@%m%{{\\e[0m%}} %{{\\e[38;2;{chemin_r};{chemin_g};{chemin_b}m%}}/%{{\\e[0m%}} %{{\\e[3;38;2;{hist_r};{hist_g};{hist_b}m%}}${{__BASHOP_HIST}}%{{\\e[0m%}} %{{\\e[1;38;2;{suf_r};{suf_g};{suf_b}m%}}{suffixe}%{{\\e[0m%}} ",
      pin = PIN_ZSH,
      heure_r = COULEUR_HEURE.0,
      heure_g = COULEUR_HEURE.1,
      heure_b = COULEUR_HEURE.2,
      accent = accent_hex,
      chemin_r = COULEUR_CHEMIN.0,
      chemin_g = COULEUR_CHEMIN.1,
      chemin_b = COULEUR_CHEMIN.2,
      hist_r = COULEUR_HISTORIQUE.0,
      hist_g = COULEUR_HISTORIQUE.1,
      hist_b = COULEUR_HISTORIQUE.2,
      suf_r = suffixe.0,
      suf_g = suffixe.1,
      suf_b = suffixe.2,
      suffixe = suffixe_cadre(theme, largeur_suffixe),
    )
  }

/// Contenu du fichier de démarrage de bash (`--rcfile`).
pub fn rc_bash(theme: &Theme, largeur_suffixe: u16) -> String
  {
    format!(
      "# Généré par Bashop. Ne pas éditer à la main.\n\
       if [ -f \"${{HOME}}/.bashrc\" ]; then . \"${{HOME}}/.bashrc\"; fi\n\
       shopt -s checkwinsize\n\
       __bashop_precmd() {{ local s=$?; __BASHOP_HEURE=$(date +%H:%M:%S); __BASHOP_HIST=$(printf '(%3d)' \"${{HISTCMD:-0}}\"); printf '\\033]133;D;%s\\007' \"$s\"; }}\n\
       PROMPT_COMMAND='__bashop_precmd'\n\
       PS1='{ps1}'\n",
      ps1 = prompt_bash(theme, largeur_suffixe),
    )
  }

/// Contenu du `.zshrc` de démarrage de zsh (via `ZDOTDIR`).
pub fn zshrc_zsh(theme: &Theme, largeur_suffixe: u16) -> String
  {
    format!(
      "# Généré par Bashop. Ne pas éditer à la main.\n\
       setopt prompt_subst\n\
       setopt singlelinezle\n\
       if [ -f \"${{HOME}}/.zshrc\" ]; then source \"${{HOME}}/.zshrc\"; fi\n\
       __bashop_precmd() {{ local s=$?; __BASHOP_HEURE=$(date +%H:%M:%S); __BASHOP_HIST=$(printf '(%3d)' \"${{HISTCMD:-0}}\"); printf '\\033]133;D;%s\\007' \"$s\"; }}\n\
       precmd_functions+=(__bashop_precmd)\n\
       PROMPT='{prompt}'\n",
      prompt = prompt_zsh(theme, largeur_suffixe),
    )
  }

/// Indique si un fichier de démarrage est généré pour ce shell.
pub fn supporte(shell: Shell) -> bool
  {
    matches!(shell, Shell::Bash | Shell::Zsh)
  }

#[cfg(test)]
mod tests
  {
    use super::*;
    use crate::theme::CLASSIQUE;

    #[test]
    fn le_suffixe_est_cadre_sur_la_largeur()
      {
        assert_eq!(suffixe_cadre(&CLASSIQUE, 6), "    >>");
      }

    #[test]
    fn le_prompt_bash_publie_les_marqueurs_osc()
      {
        let prompt = prompt_bash(&CLASSIQUE, 6);
        assert!(prompt.contains("133;A"));
        assert!(prompt.contains("133;B"));
        assert!(prompt.contains(">>"));
      }

    #[test]
    fn le_prompt_bash_affiche_user_at_host_le_chemin_absolu_et_le_numero()
      {
        let prompt = prompt_bash(&CLASSIQUE, 6);
        assert!(prompt.contains("\\u@\\h"), "user@host absent : {prompt}");
        assert!(prompt.contains("${PWD}"), "chemin absolu absent : {prompt}");
        assert!(prompt.contains("${__BASHOP_HIST}"), "numéro absent : {prompt}");
      }

    #[test]
    fn le_prompt_porte_l_heure_en_premiere_position()
      {
        let prompt = prompt_bash(&CLASSIQUE, 6);
        let heure = prompt.find("${__BASHOP_HEURE}");
        let identite = prompt.find("\\u@\\h");
        assert!(heure.is_some(), "heure absente : {prompt}");
        assert!(heure < identite, "l'heure doit précéder user@host : {prompt}");
        assert!(prompt.contains("\\e[3;38;2;"), "le numéro d'historique doit être italique : {prompt}");
      }

    #[test]
    fn le_prompt_est_epingle_sur_la_derniere_ligne()
      {
        assert!(prompt_bash(&CLASSIQUE, 6).contains("${LINES};1H"));
        assert!(prompt_zsh(&CLASSIQUE, 6).contains("${LINES};1H"));
      }

    #[test]
    fn le_rc_bash_force_le_mode_mono_ligne_par_prompt()
      {
        let rc = rc_bash(&CLASSIQUE, 6);
        assert!(rc.contains("PROMPT_COMMAND"));
        assert!(rc.contains("PS1="));
        assert!(rc.contains("__BASHOP_HIST"));
        assert!(rc.contains("checkwinsize"));
      }

    #[test]
    fn le_zshrc_active_prompt_subst()
      {
        let rc = zshrc_zsh(&CLASSIQUE, 6);
        assert!(rc.contains("prompt_subst"));
        assert!(rc.contains("singlelinezle"));
      }
  }
