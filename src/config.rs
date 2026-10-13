//! Lecture de `~/.opshellrc` — format **neutre** `KEY=VALUE`.
//!
//! Le fichier n'est jamais sourcé par un shell : il est lu par le binaire, ce
//! qui permet d'être identique sous bash, zsh, sh, PowerShell et cmd.exe. Une
//! ligne vide ou commençant par `#` est ignorée ; les clés inconnues sont
//! conservées dans `inconnues` pour ne rien perdre à la réécriture.

use crate::theme::{Rgb, Surcharge};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Thème par défaut (`auto` = selon le privilège détecté).
pub const THEME_DEFAUT: &str = "auto";
/// Largeur (colonnes) du champ de suffixe de prompt.
pub const SUFFIX_WIDTH_DEFAUT: u16 = 6;
/// Commandes déclenchant la bascule automatique vers le thème root.
pub const TRIGGERS_DEFAUT: &[&str] = &["sudo", "doas", "su", "sudoedit"];
/// Notification de fin de commande longue active par défaut.
pub const NOTIFICATION_DEFAUT: bool = true;
/// Durée minimale (secondes) d'une commande pour être notifiée.
pub const SEUIL_NOTIFICATION_DEFAUT: u64 = 5;
/// Horodatage des commandes désactivé par défaut.
pub const HORODATAGE_DEFAUT: bool = false;
/// Ajout d'une ligne vide après chaque commande actif par défaut.
pub const LIGNE_VIDE_DEFAUT: bool = true;

/// Configuration utilisateur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config
  {
    /// Nom du thème de base.
    pub theme: String,
    /// Commandes déclenchant le thème root pendant la saisie.
    pub triggers: Vec<String>,
    /// Largeur du champ du suffixe de prompt, en colonnes.
    pub suffix_width: u16,
    /// Mode capture actif (masquage à l'affichage).
    pub capture: bool,
    /// Notification à la fin d'une commande longue.
    pub notification: bool,
    /// Durée minimale (s) déclenchant la notification.
    pub seuil_notification: u64,
    /// Horodatage des commandes dans `term_context`.
    pub horodatage: bool,
    /// Ajout d'une ligne vide après chaque commande (lisibilité).
    pub ligne_vide: bool,
    /// Couleurs personnalisées des zones du cadre et du prompt.
    pub couleurs: Surcharge,
    /// Clés non reconnues, conservées telles quelles.
    pub inconnues: BTreeMap<String, String>,
  }

impl Default for Config
  {
    fn default() -> Self
      {
        Self {
          theme: THEME_DEFAUT.to_string(),
          triggers: TRIGGERS_DEFAUT.iter().map(|s| (*s).to_string()).collect(),
          suffix_width: SUFFIX_WIDTH_DEFAUT,
          capture: true,
          notification: NOTIFICATION_DEFAUT,
          seuil_notification: SEUIL_NOTIFICATION_DEFAUT,
          horodatage: HORODATAGE_DEFAUT,
          ligne_vide: LIGNE_VIDE_DEFAUT,
          couleurs: Surcharge::default(),
          inconnues: BTreeMap::new(),
        }
      }
  }

/// Charge `~/.opshellrc` (repli sur les valeurs par défaut).
pub fn charger() -> Config
  {
    if let Some(home) = std::env::var_os("HOME")
      {
        let chemin = std::path::Path::new(&home).join(".opshellrc");
        if let Ok(contenu) = std::fs::read_to_string(&chemin)
          {
            return analyser(&contenu);
          }
      }
    Config::default()
  }

/// Interprète un booléen neutre (`on`/`off`, `true`/`false`, `1`/`0`).
fn lire_bool(valeur: &str) -> Option<bool>
  {
    match valeur.to_ascii_lowercase().as_str()
      {
        "on" | "true" | "1" | "yes" | "oui" => Some(true),
        "off" | "false" | "0" | "no" | "non" => Some(false),
        _ => None,
      }
  }

/// Interprète une couleur `#RRGGBB` (dièse facultatif, forme courte `#RGB`).
/// Renvoie `None` si la chaîne n'est pas une couleur valide.
pub fn parse_couleur(valeur: &str) -> Option<Rgb>
  {
    let brut = valeur.trim();
    let v = brut.strip_prefix('#').unwrap_or(brut);
    if !v.is_ascii()
      {
        return None;
      }
    let octet = |s: &str| u8::from_str_radix(s, 16).ok();
    match v.len()
      {
        6 => Some(Rgb::new(octet(&v[0..2])?, octet(&v[2..4])?, octet(&v[4..6])?)),
        3 => {
          let doublet = |s: &str| octet(&format!("{s}{s}"));
          Some(Rgb::new(doublet(&v[0..1])?, doublet(&v[1..2])?, doublet(&v[2..3])?))
        }
        _ => None,
      }
  }

/// Formate une couleur RVB en `#RRGGBB`.
pub fn format_couleur(couleur: Rgb) -> String
  {
    format!("#{:02X}{:02X}{:02X}", couleur.r, couleur.g, couleur.b)
  }

/// Récrit `~/.opshellrc` avec la valeur de `CAPTURE` demandée, en préservant
/// toutes les autres lignes (commentaires, clés inconnues, autres réglages).
pub fn fixer_capture(actif: bool) -> std::io::Result<()>
  {
    let home = std::env::var_os("HOME").ok_or_else(|| std::io::Error::other("HOME absent"))?;
    fixer_capture_dans(PathBuf::from(home).join(".opshellrc"), actif)
  }

/// Contenu canonique de `~/.opshellrc` pour une configuration : les clés
/// reconnues dans un ordre stable, puis les clés inconnues triées.
pub fn assembler(config: &Config) -> String
  {
    let mut lignes = Vec::new();
    lignes.push(format!("THEME={}", config.theme.trim()));
    lignes.push(format!(
      "TRIGGERS={}",
      config.triggers.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(", ")
    ));
    lignes.push(format!("SUFFIX_WIDTH={}", config.suffix_width.max(1)));
    lignes.push(format!("CAPTURE={}", if config.capture { "on" } else { "off" }));
    lignes.push(format!("NOTIFICATION={}", if config.notification { "on" } else { "off" }));
    lignes.push(format!("NOTIFICATION_SEUIL={}", config.seuil_notification.max(1)));
    lignes.push(format!("HORODATAGE={}", if config.horodatage { "on" } else { "off" }));
    lignes.push(format!("LIGNE_VIDE={}", if config.ligne_vide { "on" } else { "off" }));
    if let Some(c) = config.couleurs.stat_fg
      {
        lignes.push(format!("STAT_FG={}", format_couleur(c)));
      }
    if let Some(c) = config.couleurs.stat_bg
      {
        lignes.push(format!("STAT_BG={}", format_couleur(c)));
      }
    if let Some(c) = config.couleurs.contexte_fg
      {
        lignes.push(format!("CONTEXT_FG={}", format_couleur(c)));
      }
    if let Some(c) = config.couleurs.contexte_bg
      {
        lignes.push(format!("CONTEXT_BG={}", format_couleur(c)));
      }
    if let Some(c) = config.couleurs.prompt_fg
      {
        lignes.push(format!("PROMPT_FG={}", format_couleur(c)));
      }
    if let Some(c) = config.couleurs.prompt_bg
      {
        lignes.push(format!("PROMPT_BG={}", format_couleur(c)));
      }
    for (cle, valeur) in &config.inconnues
      {
        lignes.push(format!("{cle}={valeur}"));
      }
    lignes.join("\n") + "\n"
  }

/// Écrit `~/.opshellrc` depuis une configuration, sans perte des clés
/// inconnues (voir [`assembler`]).
pub fn ecrire(config: &Config) -> std::io::Result<PathBuf>
  {
    let home = std::env::var_os("HOME").ok_or_else(|| std::io::Error::other("HOME absent"))?;
    let chemin = PathBuf::from(home).join(".opshellrc");
    ecrire_dans(&chemin, config)?;
    Ok(chemin)
  }

/// Variante testable de [`ecrire`], sur un chemin explicite.
pub fn ecrire_dans(chemin: &PathBuf, config: &Config) -> std::io::Result<()>
  {
    std::fs::write(chemin, assembler(config))
  }

/// Variante testable de [`fixer_capture`], sur un chemin explicite.
pub fn fixer_capture_dans(chemin: PathBuf, actif: bool) -> std::io::Result<()>
  {
    let valeur = if actif { "on" } else { "off" };
    let contenu = std::fs::read_to_string(&chemin).unwrap_or_default();
    let mut trouvée = false;
    let mut lignes: Vec<String> = Vec::new();
    for ligne in contenu.lines()
      {
        let cle = ligne
          .trim_start()
          .split(['=', ' '])
          .next()
          .unwrap_or("")
          .trim()
          .to_ascii_uppercase();
        if cle == "CAPTURE"
          {
            lignes.push(format!("CAPTURE={valeur}"));
            trouvée = true;
          }
        else
          {
            lignes.push(ligne.to_string());
          }
      }
    if !trouvée
      {
        lignes.push(format!("CAPTURE={valeur}"));
      }
    std::fs::write(&chemin, lignes.join("\n") + "\n")
  }

/// Analyse le contenu de `~/.opshellrc` et complète les valeurs par défaut.
pub fn analyser(contenu: &str) -> Config
  {
    let mut config = Config::default();
    for ligne_brute in contenu.lines()
      {
        let ligne = ligne_brute.trim();
        if ligne.is_empty() || ligne.starts_with('#')
          {
            continue;
          }
        let Some((cle_brute, valeur_brute)) = ligne.split_once('=')
          else
            {
              continue;
            };
        let cle = cle_brute.trim().to_ascii_uppercase();
        let valeur = valeur_brute.trim();
        match cle.as_str()
          {
            "THEME" => config.theme = valeur.to_string(),
            "TRIGGERS" | "DECLENCHEURS" => {
              config.triggers = valeur
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            }
            "SUFFIX_WIDTH" | "LARGEUR_SUFFIXE" => {
              let largeur = valeur.parse::<u16>().unwrap_or(SUFFIX_WIDTH_DEFAUT);
              config.suffix_width = largeur.max(1);
            }
            "CAPTURE" => {
              if let Some(actif) = lire_bool(valeur)
                {
                  config.capture = actif;
                }
            }
            "NOTIFICATION" | "NOTIF" => {
              if let Some(actif) = lire_bool(valeur)
                {
                  config.notification = actif;
                }
            }
            "NOTIFICATION_SEUIL" | "SEUIL_NOTIFICATION" => {
              let seuil = valeur.parse::<u64>().unwrap_or(SEUIL_NOTIFICATION_DEFAUT);
              config.seuil_notification = seuil.max(1);
            }
            "HORODATAGE" | "TIMESTAMP" => {
              if let Some(actif) = lire_bool(valeur)
                {
                  config.horodatage = actif;
                }
            }
            "LIGNE_VIDE" | "BLANK_LINE" => {
              if let Some(actif) = lire_bool(valeur)
                {
                  config.ligne_vide = actif;
                }
            }
            "STAT_FG" => config.couleurs.stat_fg = parse_couleur(valeur),
            "STAT_BG" => config.couleurs.stat_bg = parse_couleur(valeur),
            "CONTEXT_FG" | "CONTEXTE_FG" => config.couleurs.contexte_fg = parse_couleur(valeur),
            "CONTEXT_BG" | "CONTEXTE_BG" => config.couleurs.contexte_bg = parse_couleur(valeur),
            "PROMPT_FG" => config.couleurs.prompt_fg = parse_couleur(valeur),
            "PROMPT_BG" => config.couleurs.prompt_bg = parse_couleur(valeur),
            _ => {
              config.inconnues.insert(cle, valeur.to_string());
            }
          }
      }
    config
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn les_defauts_sont_charges_quand_le_fichier_est_vide()
      {
        let c = analyser("");
        assert_eq!(c, Config::default());
        assert!(c.capture, "le mode capture est actif par défaut");
      }

    #[test]
    fn les_commentaires_et_lignes_vides_sont_ignores()
      {
        let c = analyser("# un commentaire\n\n   \n");
        assert_eq!(c, Config::default());
      }

    #[test]
    fn theme_et_suffixe_sont_lus()
      {
        let c = analyser("THEME = wheel\nSUFFIX_WIDTH=8\nCAPTURE=off");
        assert_eq!(c.theme, "wheel");
        assert_eq!(c.suffix_width, 8);
        assert!(!c.capture);
      }

    #[test]
    fn les_declencheurs_sont_decoupes_sur_la_virgule()
      {
        let c = analyser("TRIGGERS=sudo, doas , su");
        assert_eq!(c.triggers, vec!["sudo", "doas", "su"]);
      }

    #[test]
    fn les_cles_inconnues_sont_conservees()
      {
        let c = analyser("COULEUR_PERSO=#112233");
        assert_eq!(c.inconnues.get("COULEUR_PERSO").map(String::as_str), Some("#112233"));
      }

    #[test]
    fn notification_et_horodatage_sont_lus()
      {
        let c = analyser("NOTIFICATION=on\nNOTIFICATION_SEUIL=8\nHORODATAGE=yes");
        assert!(c.notification);
        assert_eq!(c.seuil_notification, 8);
        assert!(c.horodatage);
      }

    #[test]
    fn un_seuil_invalide_revient_a_la_valeur_par_defaut()
      {
        let c = analyser("NOTIFICATION_SEUIL=abc");
        assert_eq!(c.seuil_notification, SEUIL_NOTIFICATION_DEFAUT);
        let c = analyser("NOTIFICATION_SEUIL=0");
        assert_eq!(c.seuil_notification, 1);
      }

    #[test]
    fn le_fichier_de_capture_est_recrit_sans_perte_des_autres_cles()
      {
        let dossier = std::env::temp_dir().join(format!("bashop-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join(".opshellrc");
        std::fs::write(&chemin, "# commentaire\nTHEME=wheel\nCAPTURE=on\n").unwrap();
        fixer_capture_dans(chemin.clone(), false).unwrap();
        let contenu = std::fs::read_to_string(&chemin).unwrap();
        assert!(contenu.contains("THEME=wheel"));
        assert!(contenu.contains("CAPTURE=off"));
        assert!(contenu.contains("# commentaire"));
        let _ = std::fs::remove_file(&chemin);
        let _ = std::fs::remove_dir(dossier);
      }

    #[test]
    fn la_cle_capture_est_ajoutee_quand_absente()
      {
        let dossier = std::env::temp_dir().join(format!("bashop-cfg2-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join(".opshellrc");
        fixer_capture_dans(chemin.clone(), true).unwrap();
        let contenu = std::fs::read_to_string(&chemin).unwrap();
        assert!(contenu.contains("CAPTURE=on"));
        let _ = std::fs::remove_file(&chemin);
        let _ = std::fs::remove_dir(dossier);
      }

    #[test]
    fn le_contenu_canonique_est_ordonne_et_sans_double()
      {
        let c = analyser("# rien\nHORODATAGE=on\nTHEME=wheel\nSUFFIX_WIDTH=8\nCAPTURE=off\nNOTIFICATION=off\nNOTIFICATION_SEUIL=9\nTRIGGERS=sudo, su\n");
        let contenu = assembler(&c);
        let lignes: Vec<&str> = contenu.lines().collect();
        assert_eq!(lignes[0], "THEME=wheel");
        assert!(lignes[1].starts_with("TRIGGERS="));
        assert_eq!(lignes[2], "SUFFIX_WIDTH=8");
        assert_eq!(lignes[3], "CAPTURE=off");
        assert_eq!(lignes[4], "NOTIFICATION=off");
        assert_eq!(lignes[5], "NOTIFICATION_SEUIL=9");
        assert_eq!(lignes[6], "HORODATAGE=on");
        assert_eq!(lignes[7], "LIGNE_VIDE=on");
        assert_eq!(lignes.len(), 8, "aucune clé en double, pas de ligne vide");
      }

    #[test]
    fn une_couleur_hex_est_lue_et_ecrite_de_facon_symetrique()
      {
        assert_eq!(parse_couleur("#112233"), Some(Rgb::new(0x11, 0x22, 0x33)));
        assert_eq!(parse_couleur("112233"), Some(Rgb::new(0x11, 0x22, 0x33)));
        assert_eq!(parse_couleur("#abc"), Some(Rgb::new(0xAA, 0xBB, 0xCC)));
        assert_eq!(format_couleur(Rgb::new(0x11, 0x22, 0x33)), "#112233");
      }

    #[test]
    fn une_couleur_invalide_est_ignoree()
      {
        assert_eq!(parse_couleur("xyz"), None);
        assert_eq!(parse_couleur("#12345"), None);
        assert_eq!(parse_couleur("#gggggg"), None);
        assert_eq!(parse_couleur(""), None);
      }

    #[test]
    fn les_couleurs_et_la_ligne_vide_sont_lues()
      {
        let c = analyser("LIGNE_VIDE=off\nSTAT_FG=#FFFFFF\nPROMPT_BG=#102030\nSTAT_BG=nope\n");
        assert!(!c.ligne_vide);
        assert_eq!(c.couleurs.stat_fg, Some(Rgb::new(0xFF, 0xFF, 0xFF)));
        assert_eq!(c.couleurs.prompt_bg, Some(Rgb::new(0x10, 0x20, 0x30)));
        assert_eq!(c.couleurs.stat_bg, None, "une valeur invalide retombe au défaut");
      }

    #[test]
    fn les_couleurs_definies_sont_ecrites_dans_l_ordre()
      {
        let mut c = Config::default();
        c.couleurs.prompt_bg = Some(Rgb::new(0x10, 0x20, 0x30));
        c.couleurs.stat_fg = Some(Rgb::new(0xAA, 0xBB, 0xCC));
        let contenu = assembler(&c);
        let lignes: Vec<&str> = contenu.lines().collect();
        assert_eq!(lignes[7], "LIGNE_VIDE=on");
        assert_eq!(lignes[8], "STAT_FG=#AABBCC");
        assert!(lignes.contains(&"PROMPT_BG=#102030"));
      }

    #[test]
    fn un_aller_retour_ecriture_lecture_ne_perd_rien()
      {
        let dossier = std::env::temp_dir().join(format!("bashop-cfg3-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join(".opshellrc");
        let c = analyser("# c\nTHEME=classique\nTRIGGERS=sudo, doas\nCAPTURE=off\nNOTIFICATION=on\nNOTIFICATION_SEUIL=3\nHORODATAGE=yes\nCOULEUR_PERSO=#112233\n");
        ecrire_dans(&chemin, &c).unwrap();
        let relu = analyser(&std::fs::read_to_string(&chemin).unwrap());
        assert_eq!(c, relu, "le cycle écriture → relecture est stable");
        let _ = std::fs::remove_file(&chemin);
        let _ = std::fs::remove_dir(dossier);
      }

    #[test]
    fn les_espaces_etroits_du_theme_sont_nettoyes()
      {
        let c = analyser("THEME=  wheel  \nTRIGGERS=  sudo  ,   su ");
        assert_eq!(c.theme, "wheel");
        assert_eq!(c.triggers, vec!["sudo", "su"]);
      }
  }
