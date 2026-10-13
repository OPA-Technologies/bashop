//! Génération du prompt et du fichier de démarrage injectés dans le shell.
//!
//! Le shell conserve son édition native (modèle B) : on ne réimplémente pas la
//! saisie, on lui fournit un prompt qui publie les marqueurs `OSC 133` et
//! applique les couleurs du thème. Le mode mono-ligne est forcé ici.

use crate::shells::Shell;
use crate::theme::Palette;

/// Marqueur de début de prompt (OSC 133;A).
const OSC_PROMPT: &str = "\\e]133;A\\e\\\\";
/// Marqueur de début de commande (OSC 133;B).
const OSC_COMMANDE: &str = "\\e]133;B\\e\\\\";

/// SGR `[intensité;]38;2;fg;48;2;fond` : avant-plan **et** fond du cadre, pour
/// que le préfixe du prompt partage la bande de `term_stat`/`term_context`.
fn couleur(rgb: (u8, u8, u8), gras: bool, fond: (u8, u8, u8)) -> String
  {
    let intensite = if gras { 1 } else { 0 };
    format!(
      "\\e[{};38;2;{};{};{};48;2;{};{};{}m",
      intensite, rgb.0, rgb.1, rgb.2, fond.0, fond.1, fond.2
    )
  }

/// SGR `3;38;2;fg;48;2;fond` : italique sur le fond du cadre.
fn couleur_italique(rgb: (u8, u8, u8), fond: (u8, u8, u8)) -> String
  {
    format!(
      "\\e[3;38;2;{};{};{};48;2;{};{};{}m",
      rgb.0, rgb.1, rgb.2, fond.0, fond.1, fond.2
    )
  }

/// SGR `48;2;fond` : fond du cadre seul (pose la bande dès le début du prompt,
/// y compris l'effacement de fin de ligne provoqué par l'épinglage).
fn fond_sgr(fond: (u8, u8, u8)) -> String
  {
    format!("\\e[48;2;{};{};{}m", fond.0, fond.1, fond.2)
  }

/// Variante zsh : mêmes séquences, enveloppées dans `%{ … %}` (largeur nulle).
fn couleur_zsh(rgb: (u8, u8, u8), gras: bool, fond: (u8, u8, u8)) -> String
  {
    let intensite = if gras { 1 } else { 0 };
    format!(
      "%{{\\e[{};38;2;{};{};{};48;2;{};{};{}m%}}",
      intensite, rgb.0, rgb.1, rgb.2, fond.0, fond.1, fond.2
    )
  }

fn couleur_italique_zsh(rgb: (u8, u8, u8), fond: (u8, u8, u8)) -> String
  {
    format!(
      "%{{\\e[3;38;2;{};{};{};48;2;{};{};{}m%}}",
      rgb.0, rgb.1, rgb.2, fond.0, fond.1, fond.2
    )
  }

fn fond_zsh(fond: (u8, u8, u8)) -> String
  {
    format!("%{{\\e[48;2;{};{};{}m%}}", fond.0, fond.1, fond.2)
  }

/// Réinitialisation complète : la saisie retrouve le fond par défaut.
const REINIT: &str = "\\e[0m";
/// Réinitialise avant-plan, graisse et italique **sans** toucher au fond : la
/// bande du cadre se poursuit d'un segment à l'autre.
const REINIT_ATTR: &str = "\\e[39m\\e[22m\\e[23m";
/// Réinitialisation complète zsh.
const REINIT_ZSH: &str = "%{\\e[0m%}";
/// Réinitialisation zsh sans toucher au fond.
const REINIT_ATTR_ZSH: &str = "%{\\e[39m\\e[22m\\e[23m%}";
/// Épingle le prompt sur la dernière ligne de la zone du shell (avant-dernière
/// ligne de l'écran) puis efface la fin de ligne.
const PIN: &str = "\\e[${LINES};1H\\e[K";
/// Variante zsh de l'épinglage (délimiteurs `%{ %}`).
const PIN_ZSH: &str = "%{\\e[${LINES};1H\\e[K%}";

/// Cadre (cadrage à gauche) du suffixe de prompt sur la largeur normalisée : le
/// glyphe colle au segment précédent, le remplissage suit, la largeur restant
/// constante pour ne pas décaler le curseur.
pub fn suffixe_cadre(glyphe: &str, largeur: u16) -> String
  {
    format!("{:<largeur$}", glyphe, largeur = largeur as usize)
  }

/// Construit la chaîne `PS1` de bash pour un thème donné.
///
/// La ligne affiche l'heure de début de commande (`__BASHOP_HEURE`), le numéro
/// de commande entre parenthèses (`__BASHOP_HIST`, gris italique), `user@host`,
/// le chemin absolu (`${PWD}`) puis le suffixe, sans espace superflu entre le
/// chemin et le suffixe.
///
/// Tout le préfixe (jusqu'au suffixe) porte le **fond du cadre**. Le fond n'est
/// posé qu'**après** l'épinglage, de sorte que l'effacement de fin de ligne
/// (`\e[K`) se fait sur le fond **par défaut** : seule l'invite est surlignée,
/// jamais le reste de la ligne. Une réinitialisation complète précède
/// `OSC 133;B`, donc la saisie de l'utilisateur garde le fond par défaut.
pub fn prompt_bash(palette: &Palette, glyphe: &str, largeur_suffixe: u16) -> String
  {
    let accent = palette.prompt_fg.tuple();
    let fond = palette.prompt_bg.tuple();
    format!(
      "\\[{reinit}\\]\\[{pin}\\]\\[{fond}\\]\\[{osc_prompt}\\]\\[{heure}\\]${{__BASHOP_HEURE}}\\[{ra}\\] \\[{hist}\\]${{__BASHOP_HIST}}\\[{ra}\\] \\[{user}\\]\\u@\\h\\[{ra}\\] \\[{chemin}\\]${{PWD}}\\[{ra}\\] \\[{suffixe_col}\\]{suffixe}\\[{reinit}\\]\\[{osc_commande}\\]",
      fond = fond_sgr(fond),
      pin = PIN,
      osc_prompt = OSC_PROMPT,
      heure = couleur_italique(palette.prompt_heure.tuple(), fond),
      ra = REINIT_ATTR,
      user = couleur(accent, true, fond),
      chemin = couleur(palette.prompt_chemin.tuple(), false, fond),
      hist = couleur_italique(palette.prompt_historique.tuple(), fond),
      suffixe_col = couleur(accent, true, fond),
      suffixe = suffixe_cadre(glyphe, largeur_suffixe),
      reinit = REINIT,
      osc_commande = OSC_COMMANDE,
    )
  }

/// Construit la chaîne `PROMPT` de zsh pour un thème donné.
pub fn prompt_zsh(palette: &Palette, glyphe: &str, largeur_suffixe: u16) -> String
  {
    let accent = palette.prompt_fg.tuple();
    let fond = palette.prompt_bg.tuple();
    format!(
      "{reinit}{pin}{fond}{heure}${{__BASHOP_HEURE}}{ra} {hist}${{__BASHOP_HIST}}{ra} {user}%n@%m{ra} {chemin}${{PWD}}{ra} {suf}{suffixe}{reinit}",
      fond = fond_zsh(fond),
      pin = PIN_ZSH,
      heure = couleur_italique_zsh(palette.prompt_heure.tuple(), fond),
      ra = REINIT_ATTR_ZSH,
      user = couleur_zsh(accent, true, fond),
      chemin = couleur_zsh(palette.prompt_chemin.tuple(), false, fond),
      hist = couleur_italique_zsh(palette.prompt_historique.tuple(), fond),
      suf = couleur_zsh(accent, true, fond),
      suffixe = suffixe_cadre(glyphe, largeur_suffixe),
      reinit = REINIT_ZSH,
    )
  }

/// Marqueur de début du bloc injecté dans les rc (`bashop install`).
pub const MARQUE_DEBUT: &str = "# >>> bashop >>>";
/// Marqueur de fin du bloc injecté dans les rc (`bashop install`).
pub const MARQUE_FIN: &str = "# <<< bashop <<<";

/// Calcule (une fois) l'identité et le privilège du shell courant. Ces valeurs
/// ne changent qu'à l'ouverture d'un nouveau shell (donc d'une nouvelle
/// session sous `sudo -i`), jamais pendant la vie d'un prompt.
const CALCUL_PRIVILEGE: &str = "\
if [ -z \"${__bashop_priv:-}\" ]\n\
then\n\
  if [ \"$(id -u 2>/dev/null)\" = \"0\" ]\n\
  then\n\
    __bashop_priv=root\n\
  else\n\
    case \" $(id -Gn 2>/dev/null) \" in\n\
      *\" wheel \"*|*\" sudo \"*|*\" admin \"*|*\" adm \"*) __bashop_priv=wheel ;;\n\
      *) __bashop_priv=classique ;;\n\
    esac\n\
  fi\n\
  __bashop_user=\"${USER:-$(id -un 2>/dev/null)}\"\n\
fi\n";

/// Publie identité et privilège par l'OSC privé `1337;Bashop`.
const PUBLIER_PRIVILEGE: &str = "printf '\\033]1337;Bashop;user=%s;priv=%s\\007' \"$__bashop_user\" \"$__bashop_priv\"\n";

/// En-tête bash : émule le démarrage d'un shell de connexion (profils) **puis**
/// lit le rc interactif, de sorte que `.profile` **et** `.bashrc` de
/// l'utilisateur soient disponibles (alias, fonctions, variables d'env).
///
/// Le bloc d'activation Bashop présent dans `.bashrc` est inoffensif ici :
/// `BASHOP_ACTIVE=1` est transmis au shell enfant, donc le cadre n'est pas
/// relancé. `PS1`/`PROMPT_COMMAND` sont posés **après** cet en-tête, ce qui
/// neutralise d'éventuels réglages de prompt issus des rc.
const ENTETE_BASH: &str = "\
if [ -f /etc/profile ]; then . /etc/profile; fi\n\
for __bashop_profil in \"${HOME}/.bash_profile\" \"${HOME}/.bash_login\" \"${HOME}/.profile\"\n\
do\n\
  if [ -f \"$__bashop_profil\" ]\n\
  then\n\
    . \"$__bashop_profil\"\n\
    break\n\
  fi\n\
done\n\
if [ -f /etc/bash.bashrc ]; then . /etc/bash.bashrc; fi\n\
if [ -f /etc/bashrc ]; then . /etc/bashrc; fi\n\
if [ -f \"${HOME}/.bashrc\" ]; then . \"${HOME}/.bashrc\"; fi\n\
unset __bashop_profil\n";

/// Bloc inséré dans `precmd` pour ajouter une ligne vide entre les commandes.
///
/// La toute première invite ne reçoit pas de ligne vide : un témoin
/// `__bashop_vide` n'est posé qu'après le premier `precmd`.
fn bloc_ligne_vide(ligne_vide: bool) -> &'static str
  {
    if ligne_vide
      {
        "  if [ -n \"${__bashop_vide:-}\" ]\n  then\n    printf '\\n'\n  fi\n  __bashop_vide=1\n"
      }
    else
      {
        ""
      }
  }

/// Corps de démarrage bash : `precmd` (heure, historique, statut, privilège),
/// `PROMPT_COMMAND` et `PS1`. Ne lit **pas** les profils : c'est la partie
/// commune au rc généré (`--rcfile`) et à la commande `bashop prompt`.
pub fn script_prompt_bash(palette: &Palette, glyphe: &str, largeur_suffixe: u16, ligne_vide: bool) -> String
  {
    let mut rc = String::new();
    rc.push_str("__bashop_precmd()\n{\n");
    rc.push_str("  local s=$?\n");
    rc.push_str(bloc_ligne_vide(ligne_vide));
    rc.push_str("  __BASHOP_HEURE=$(date +%H:%M:%S)\n");
    rc.push_str("  __BASHOP_HIST=$(printf '(%3d)' \"${HISTCMD:-0}\")\n");
    rc.push_str("  printf '\\033]133;D;%s\\007' \"$s\"\n");
    rc.push_str(CALCUL_PRIVILEGE);
    rc.push_str(PUBLIER_PRIVILEGE);
    rc.push_str("}\n");
    rc.push_str("PROMPT_COMMAND=\"__bashop_precmd${PROMPT_COMMAND:+; ${PROMPT_COMMAND//__bashop_osc/}}\"\n");
    rc.push_str("bind 'set horizontal-scroll-mode on'\n");
    rc.push_str(&format!("PS1='{}'\n", prompt_bash(palette, glyphe, largeur_suffixe)));
    rc
  }

/// Corps de démarrage zsh : `precmd` (heure, historique, statut, privilège) et
/// `PROMPT`. Ne lit **pas** `~/.zshrc` : partie commune au `.zshrc` généré et à
/// `bashop prompt`.
pub fn script_prompt_zsh(palette: &Palette, glyphe: &str, largeur_suffixe: u16, ligne_vide: bool) -> String
  {
    let mut rc = String::new();
    rc.push_str("__bashop_precmd()\n{\n");
    rc.push_str("  local s=$?\n");
    rc.push_str(bloc_ligne_vide(ligne_vide));
    rc.push_str("  __BASHOP_HEURE=$(date +%H:%M:%S)\n");
    rc.push_str("  __BASHOP_HIST=$(printf '(%3d)' \"${HISTCMD:-0}\")\n");
    rc.push_str("  printf '\\033]133;D;%s\\007' \"$s\"\n");
    rc.push_str(CALCUL_PRIVILEGE);
    rc.push_str(PUBLIER_PRIVILEGE);
    rc.push_str("}\n");
    rc.push_str("typeset -ga precmd_functions\n");
    rc.push_str("precmd_functions+=(__bashop_precmd)\n");
    rc.push_str(&format!("PROMPT='{}'\n", prompt_zsh(palette, glyphe, largeur_suffixe)));
    rc
  }

/// Contenu du fichier de démarrage de bash (`--rcfile`).
pub fn rc_bash(palette: &Palette, glyphe: &str, largeur_suffixe: u16, ligne_vide: bool) -> String
  {
    let mut rc = String::from("# Généré par Bashop. Ne pas éditer à la main.\n");
    rc.push_str(ENTETE_BASH);
    rc.push_str("shopt -s checkwinsize\n");
    rc.push_str(&script_prompt_bash(palette, glyphe, largeur_suffixe, ligne_vide));
    rc
  }

/// Contenu du `.zshrc` de démarrage de zsh (via `ZDOTDIR`).
pub fn zshrc_zsh(palette: &Palette, glyphe: &str, largeur_suffixe: u16, ligne_vide: bool) -> String
  {
    let mut rc = String::from("# Généré par Bashop. Ne pas éditer à la main.\n");
    rc.push_str("setopt prompt_subst\n");
    rc.push_str("setopt singlelinezle\n");
    rc.push_str("if [ -f \"${HOME}/.zshrc\" ]; then source \"${HOME}/.zshrc\"; fi\n");
    rc.push_str(&script_prompt_zsh(palette, glyphe, largeur_suffixe, ligne_vide));
    rc
  }

/// Fichier `.zshenv` du `ZDOTDIR` temporaire : relaie le vrai `.zshenv`.
pub fn zshenv_zsh() -> String
  {
    "if [ -f \"${HOME}/.zshenv\" ]; then source \"${HOME}/.zshenv\"; fi\n".to_string()
  }

/// Fichier `.zprofile` du `ZDOTDIR` temporaire : relaie le vrai `.zprofile`.
pub fn zprofile_zsh() -> String
  {
    "if [ -f \"${HOME}/.zprofile\" ]; then source \"${HOME}/.zprofile\"; fi\n".to_string()
  }

/// Fichier `.zlogin` du `ZDOTDIR` temporaire : relaie le vrai `.zlogin`.
pub fn zlogin_zsh() -> String
  {
    "if [ -f \"${HOME}/.zlogin\" ]; then source \"${HOME}/.zlogin\"; fi\n".to_string()
  }

/// Bloc d'activation idempotent à injecter dans le `.bashrc` de l'utilisateur
/// (`bashop install`). Il publie le privilège puis lance le cadre, sauf si
/// Bashop est déjà actif (env ou ancêtre) ou sous un multiplexeur.
///
/// Sous un cadre **déjà actif** (cas `sudo -i` : l'environnement est
/// réinitialisé mais un ancêtre `bashop` subsiste), le bloc ne relance pas le
/// cadre : il repose seulement le prompt épinglé et les marqueurs `OSC 133` via
/// `bashop prompt`, de sorte que le cadre parent se réactualise (thème root).
pub fn bloc_bash() -> String
  {
    format!(
      "{debut}\n{calcul}\n__bashop_osc()\n{{\n  printf '\\033]133;D;%s\\007' \"$?\"\n  {publier}}}\nif [ -n \"${{BASH_VERSION:-}}\" ]\nthen\n  case \"${{PROMPT_COMMAND:-}}\" in\n    *__bashop_osc*) : ;;\n    *) PROMPT_COMMAND=\"__bashop_osc${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}\" ;;\n  esac\nfi\nif [ -z \"${{BASHOP_ACTIVE:-}}\" ] && [ -z \"${{BASHOP_DISABLE:-}}\" ]\nthen\n  case \"$-\" in\n    *i*)\n      if command -v bashop >/dev/null 2>&1\n      then\n        if bashop _guard\n        then\n          exec bashop run --shell bash\n        elif bashop _nested\n        then\n          eval \"$(bashop prompt --shell bash)\"\n        fi\n      fi\n      ;;\n  esac\nfi\n{fin}\n",
      debut = MARQUE_DEBUT,
      calcul = CALCUL_PRIVILEGE,
      publier = PUBLIER_PRIVILEGE,
      fin = MARQUE_FIN,
    )
  }

/// Bloc d'activation idempotent pour le `.zshrc` de l'utilisateur. Voir
/// [`bloc_bash`] pour la branche « cadre déjà actif ».
pub fn bloc_zsh() -> String
  {
    format!(
      "{debut}\n{calcul}\n__bashop_osc()\n{{\n  printf '\\033]133;D;%s\\007' \"$?\"\n  {publier}}}\ntypeset -ga precmd_functions\nprecmd_functions+=(__bashop_osc)\nif [ -z \"${{BASHOP_ACTIVE:-}}\" ] && [ -z \"${{BASHOP_DISABLE:-}}\" ]\nthen\n  if [[ -o interactive ]] && command -v bashop >/dev/null 2>&1\n  then\n    if bashop _guard\n    then\n      exec bashop run --shell zsh\n    elif bashop _nested\n    then\n      eval \"$(bashop prompt --shell zsh)\"\n    fi\n  fi\nfi\n{fin}\n",
      debut = MARQUE_DEBUT,
      calcul = CALCUL_PRIVILEGE,
      publier = PUBLIER_PRIVILEGE,
      fin = MARQUE_FIN,
    )
  }

/// Bloc **système** (`bashop install --system`) à déposer sous
/// `/etc/profile.d/bashop.sh` (Linux). Il rend le prompt Bashop disponible au
/// shell de connexion **root** déclenché par `sudo -i` : l'environnement est
/// réinitialisé (donc `BASHOP_ACTIVE` disparaît) mais un ancêtre `bashop`
/// subsiste, ce que détecte `bashop _nested`. Le bloc est neutre (POSIX) et
/// sans effet hors de ce cas : un shell root ordinaire (aucun ancêtre Bashop)
/// n'est pas modifié.
pub fn bloc_systeme() -> String
  {
    format!(
      "{debut}\nif [ -z \"${{BASHOP_ACTIVE:-}}\" ] && [ -z \"${{BASHOP_DISABLE:-}}\" ] && command -v bashop >/dev/null 2>&1 && bashop _nested\nthen\n  case \"$-\" in\n    *i*)\n      if [ -n \"${{BASH_VERSION:-}}\" ]\n      then\n        eval \"$(bashop prompt --shell bash)\"\n      elif [ -n \"${{ZSH_VERSION:-}}\" ]\n      then\n        eval \"$(bashop prompt --shell zsh)\"\n      fi\n      ;;\n  esac\nfi\n{fin}\n",
      debut = MARQUE_DEBUT,
      fin = MARQUE_FIN,
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
    use crate::theme::Surcharge;

    fn p() -> Palette
      {
        crate::theme::palette(&crate::theme::CLASSIQUE, &Surcharge::default())
      }

    #[test]
    fn le_suffixe_est_cadre_sur_la_largeur()
      {
        assert_eq!(suffixe_cadre(">>", 6), ">>    ");
      }

    #[test]
    fn le_prompt_bash_publie_les_marqueurs_osc()
      {
        let prompt = prompt_bash(&p(), ">>", 6);
        assert!(prompt.contains("133;A"));
        assert!(prompt.contains("133;B"));
        assert!(prompt.contains(">>"));
      }

    #[test]
    fn le_prompt_bash_affiche_user_at_host_le_chemin_absolu_et_le_numero()
      {
        let prompt = prompt_bash(&p(), ">>", 6);
        assert!(prompt.contains("\\u@\\h"), "user@host absent : {prompt}");
        assert!(prompt.contains("${PWD}"), "chemin absolu absent : {prompt}");
        assert!(prompt.contains("${__BASHOP_HIST}"), "numéro absent : {prompt}");
      }

    #[test]
    fn le_prompt_porte_l_heure_en_premiere_position()
      {
        let prompt = prompt_bash(&p(), ">>", 6);
        let heure = prompt.find("${__BASHOP_HEURE}");
        let identite = prompt.find("\\u@\\h");
        assert!(heure.is_some(), "heure absente : {prompt}");
        assert!(heure < identite, "l'heure doit précéder user@host : {prompt}");
        assert!(prompt.contains("\\e[3;38;2;"), "le numéro d'historique doit être italique : {prompt}");
      }

    #[test]
    fn le_prompt_est_epingle_sur_la_derniere_ligne()
      {
        assert!(prompt_bash(&p(), ">>", 6).contains("${LINES};1H"));
        assert!(prompt_zsh(&p(), ">>", 6).contains("${LINES};1H"));
      }

    #[test]
    fn le_prefixe_du_prompt_porte_le_fond_du_cadre()
      {
        let palette = p();
        let fond = format!("48;2;{};{};{}", palette.prompt_bg.r, palette.prompt_bg.g, palette.prompt_bg.b);
        let bash = prompt_bash(&palette, ">>", 6);
        assert!(bash.contains(&fond), "fond du cadre absent : {bash}");
        let zsh = prompt_zsh(&palette, ">>", 6);
        assert!(zsh.contains(&fond), "fond du cadre absent (zsh) : {zsh}");
      }

    #[test]
    fn le_prompt_emploie_l_accent_personnalise()
      {
        let surcharge = Surcharge { prompt_fg: Some(crate::theme::Rgb::new(0x12, 0x34, 0x56)), ..Surcharge::default() };
        let palette = crate::theme::palette(&crate::theme::CLASSIQUE, &surcharge);
        let prompt = prompt_bash(&palette, ">>", 6);
        assert!(prompt.contains("38;2;18;52;86"), "accent personnalisé absent : {prompt}");
      }

    #[test]
    fn l_effacement_de_ligne_reste_sur_le_fond_par_defaut()
      {
        let palette = p();
        let fond = format!("48;2;{};{};{}", palette.prompt_bg.r, palette.prompt_bg.g, palette.prompt_bg.b);
        let bash = prompt_bash(&palette, ">>", 6);
        let pin = bash.find("LINES};1H").expect("épinglage");
        let fond_pos = bash.find(&fond).expect("fond du cadre");
        assert!(pin < fond_pos, "le fond doit être posé après l'effacement : {bash}");
        let zsh = prompt_zsh(&palette, ">>", 6);
        let pin = zsh.find("LINES};1H").expect("épinglage (zsh)");
        let fond_pos = zsh.find(&fond).expect("fond du cadre (zsh)");
        assert!(pin < fond_pos, "le fond doit être posé après l'effacement (zsh) : {zsh}");
      }

    #[test]
    fn le_prompt_reinitialise_le_fond_avant_la_saisie()
      {
        let bash = prompt_bash(&p(), ">>", 6);
        let commande = bash.find("133;B").expect("marqueur de commande");
        let suffixe = bash.find(">>").expect("suffixe");
        let reset = bash[..commande].rfind("\\e[0m").expect("reset avant la commande");
        assert!(suffixe < reset, "le reset doit suivre le suffixe : {bash}");
      }

    #[test]
    fn le_rc_bash_force_le_mode_mono_ligne_par_prompt()
      {
        let rc = rc_bash(&p(), ">>", 6, true);
        assert!(rc.contains("PROMPT_COMMAND"));
        assert!(rc.contains("PS1="));
        assert!(rc.contains("__BASHOP_HIST"));
        assert!(rc.contains("checkwinsize"));
      }

    #[test]
    fn le_zshrc_active_prompt_subst()
      {
        let rc = zshrc_zsh(&p(), ">>", 6, true);
        assert!(rc.contains("prompt_subst"));
        assert!(rc.contains("singlelinezle"));
      }

    #[test]
    fn les_rc_publient_le_privilege_par_osc_prive()
      {
        assert!(rc_bash(&p(), ">>", 6, true).contains("1337;Bashop"));
        assert!(zshrc_zsh(&p(), ">>", 6, true).contains("1337;Bashop"));
      }

    #[test]
    fn le_rc_bash_source_les_profils_de_connexion_et_le_bashrc()
      {
        let rc = rc_bash(&p(), ">>", 6, true);
        assert!(rc.contains("/etc/profile"));
        assert!(rc.contains(".bash_profile"));
        assert!(rc.contains("${HOME}/.bashrc"), "le .bashrc doit toujours être lu : {rc}");
        assert!(rc.contains("/etc/bash.bashrc"));
      }

    #[test]
    fn la_ligne_vide_est_posee_apres_la_premiere_invite_seulement()
      {
        let avec = script_prompt_bash(&p(), ">>", 6, true);
        assert!(avec.contains("__bashop_vide"), "témoin de ligne vide absent : {avec}");
        assert!(avec.contains("printf '\\n'"), "saut de ligne absent : {avec}");
        let sans = script_prompt_bash(&p(), ">>", 6, false);
        assert!(!sans.contains("__bashop_vide"), "ligne vide désactivée attendue : {sans}");
      }

    #[test]
    fn les_blocs_d_installation_sont_encadres_et_idempotents()
      {
        let bash = bloc_bash();
        assert!(bash.starts_with(MARQUE_DEBUT));
        assert!(bash.trim_end().ends_with(MARQUE_FIN));
        assert!(bash.contains("bashop _guard"));
        assert_eq!(bash.matches(MARQUE_DEBUT).count(), 1);
        let zsh = bloc_zsh();
        assert!(zsh.starts_with(MARQUE_DEBUT));
        assert!(zsh.contains("exec bashop run --shell zsh"));
      }

    #[test]
    fn les_blocs_reposent_le_prompt_dans_un_cadre_deja_actif()
      {
        let bash = bloc_bash();
        assert!(bash.contains("bashop _nested"), "branche imbriquée bash absente : {bash}");
        assert!(bash.contains("bashop prompt --shell bash"), "prompt imbriqué bash absent : {bash}");
        let zsh = bloc_zsh();
        assert!(zsh.contains("bashop _nested"), "branche imbriquée zsh absente : {zsh}");
        assert!(zsh.contains("bashop prompt --shell zsh"), "prompt imbriqué zsh absent : {zsh}");
      }

    #[test]
    fn le_bloc_systeme_repose_le_prompt_pour_bash_et_zsh()
      {
        let bloc = bloc_systeme();
        assert!(bloc.starts_with(MARQUE_DEBUT));
        assert!(bloc.trim_end().ends_with(MARQUE_FIN));
        assert!(bloc.contains("bashop _nested"));
        assert!(bloc.contains("bashop prompt --shell bash"));
        assert!(bloc.contains("bashop prompt --shell zsh"));
        assert!(bloc.contains("BASHOP_DISABLE"));
        assert!(bloc.contains("*i*"), "le bloc doit rester réservé aux shells interactifs");
      }

    #[test]
    fn le_script_prompt_bash_contient_precmd_et_ps1()
      {
        let script = script_prompt_bash(&p(), ">>", 6, true);
        assert!(script.contains("__bashop_precmd()"));
        assert!(script.contains("PROMPT_COMMAND"));
        assert!(script.contains("PS1="));
        assert!(script.contains("${__BASHOP_HIST}"));
        assert!(script.contains("133;A"));
        assert!(
          script.contains("horizontal-scroll-mode on"),
          "la saisie doit rester mono-ligne (pas de repli qui casse l'épinglage)"
        );
      }

    #[test]
    fn le_script_prompt_zsh_contient_precmd_et_prompt()
      {
        let script = script_prompt_zsh(&p(), ">>", 6, true);
        assert!(script.contains("__bashop_precmd()"));
        assert!(script.contains("precmd_functions"));
        assert!(script.contains("PROMPT="));
      }
  }
