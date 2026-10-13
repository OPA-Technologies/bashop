//! Thèmes et palettes.
//!
//! `classique` est la base dont dérivent `wheel` et `root` avant toute
//! personnalisation. Le suffixe de fin de prompt est cadré sur un champ de
//! largeur commune (voir `config::suffix_width`) pour que la bascule
//! automatique sous `sudo` ne décale jamais la commande en cours de frappe.

/// Couleur RVB 24 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb
  {
    /// Composante rouge.
    pub r: u8,
    /// Composante verte.
    pub g: u8,
    /// Composante bleue.
    pub b: u8,
  }

impl Rgb
  {
    /// Construit une couleur RVB.
    pub const fn new(r: u8, g: u8, b: u8) -> Self
      {
        Self { r, g, b }
      }

    /// Représentation en triplet `(r, g, b)`.
    pub const fn tuple(self) -> (u8, u8, u8)
      {
        (self.r, self.g, self.b)
      }
  }

/// Suffixe de fin de prompt : un glyphe et sa couleur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Suffixe
  {
    /// Chaîne affichée (cadrée à droite dans le champ commun).
    pub glyphe: &'static str,
    /// Couleur de l'avant-plan.
    pub couleur: Rgb,
  }

/// Thème complet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme
  {
    /// Nom du thème.
    pub nom: &'static str,
    /// Couleur d'accent (identité utilisateur).
    pub accent: Rgb,
    /// Suffixe de fin de prompt.
    pub suffixe: Suffixe,
    /// Fond des deux lignes du cadre (`term_stat`, `term_context`).
    pub fond: Rgb,
  }

/// Thème classique (utilisateur normal) — base des deux autres.
pub const CLASSIQUE: Theme =
  Theme {
    nom: "classique",
    accent: Rgb::new(0x8A, 0xE6, 0x8F),
    suffixe: Suffixe { glyphe: ">>", couleur: Rgb::new(0x4E, 0x9A, 0x06) },
    fond: Rgb::new(0x2E, 0x34, 0x40),
  };

/// Thème wheel (membre du groupe wheel / administrateurs).
pub const WHEEL: Theme =
  Theme {
    nom: "wheel",
    accent: Rgb::new(0xCB, 0xA6, 0xF7),
    suffixe: Suffixe { glyphe: ">>>>", couleur: Rgb::new(0xCB, 0xA6, 0xF7) },
    fond: Rgb::new(0x2E, 0x34, 0x40),
  };

/// Thème root (superutilisateur) : fond rouge sombre pour signaler le danger.
pub const ROOT: Theme =
  Theme {
    nom: "root",
    accent: Rgb::new(0xF3, 0x8B, 0xA8),
    suffixe: Suffixe { glyphe: "###>", couleur: Rgb::new(0xF3, 0x8B, 0xA8) },
    fond: Rgb::new(0x5A, 0x23, 0x23),
  };

/// Texte par défaut des lignes `term_stat` et `term_context`.
pub const TEXTE_CADRE: Rgb = Rgb::new(0xE6, 0xED, 0xF3);
/// Fond par défaut du cadre (`term_stat`, `term_context`, prompt).
pub const FOND_CADRE: Rgb = Rgb::new(0x2E, 0x34, 0x40);
/// Couleur par défaut du chemin dans le prompt.
pub const COULEUR_CHEMIN: Rgb = Rgb::new(0x6E, 0xA8, 0xFF);
/// Couleur par défaut du numéro d'historique dans le prompt.
pub const COULEUR_HISTORIQUE: Rgb = Rgb::new(0xA6, 0xB0, 0xCC);
/// Couleur par défaut de l'heure de début de commande dans le prompt.
pub const COULEUR_HEURE: Rgb = Rgb::new(0x7E, 0xB0, 0xDE);
/// Part de blanc appliquée aux couleurs du thème `wheel` (classique éclairci).
pub const ECLAIRCISSEMENT_WHEEL: f32 = 0.30;
/// Part de blanc appliquée aux **fonds** du thème `wheel` (stat, contexte,
/// prompt) : plus faible, pour resserrer l'écart de fond avec `classique`.
pub const ECLAIRCISSEMENT_WHEEL_FOND: f32 = 0.20;

/// Couleurs choisies par l'utilisateur dans `~/.opshellrc`. Une valeur absente
/// (`None`) laisse le défaut du thème. Les surcharges ne s'appliquent qu'aux
/// thèmes `classique` et `wheel` ; `root` reste toujours fixe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Surcharge
  {
    /// Avant-plan de `term_stat`.
    pub stat_fg: Option<Rgb>,
    /// Fond de `term_stat`.
    pub stat_bg: Option<Rgb>,
    /// Avant-plan de `term_context`.
    pub contexte_fg: Option<Rgb>,
    /// Fond de `term_context`.
    pub contexte_bg: Option<Rgb>,
    /// Couleur d'accent du prompt (identité et suffixe).
    pub prompt_fg: Option<Rgb>,
    /// Fond du prompt.
    pub prompt_bg: Option<Rgb>,
  }

impl Surcharge
  {
    /// Vrai si aucune couleur n'est personnalisée.
    pub fn vide(&self) -> bool
      {
        *self == Self::default()
      }
  }

/// Palette effective (cadre et prompt) après résolution du thème et des
/// surcharges utilisateur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette
  {
    /// Avant-plan de `term_stat`.
    pub stat_fg: Rgb,
    /// Fond de `term_stat`.
    pub stat_bg: Rgb,
    /// Avant-plan de `term_context`.
    pub contexte_fg: Rgb,
    /// Fond de `term_context`.
    pub contexte_bg: Rgb,
    /// Couleur d'accent du prompt (identité et suffixe).
    pub prompt_fg: Rgb,
    /// Fond du prompt.
    pub prompt_bg: Rgb,
    /// Chemin absolu du prompt.
    pub prompt_chemin: Rgb,
    /// Numéro d'historique du prompt.
    pub prompt_historique: Rgb,
    /// Heure de début de commande du prompt.
    pub prompt_heure: Rgb,
  }

impl Palette
  {
    /// Palette de base (`classique` sans personnalisation).
    pub fn defaut() -> Self
      {
        Self {
          stat_fg: TEXTE_CADRE,
          stat_bg: FOND_CADRE,
          contexte_fg: TEXTE_CADRE,
          contexte_bg: FOND_CADRE,
          prompt_fg: CLASSIQUE.accent,
          prompt_bg: FOND_CADRE,
          prompt_chemin: COULEUR_CHEMIN,
          prompt_historique: COULEUR_HISTORIQUE,
          prompt_heure: COULEUR_HEURE,
        }
      }

    /// Applique les surcharges non nulles.
    fn appliquer(&mut self, surcharge: &Surcharge)
      {
        if let Some(c) = surcharge.stat_fg
          {
            self.stat_fg = c;
          }
        if let Some(c) = surcharge.stat_bg
          {
            self.stat_bg = c;
          }
        if let Some(c) = surcharge.contexte_fg
          {
            self.contexte_fg = c;
          }
        if let Some(c) = surcharge.contexte_bg
          {
            self.contexte_bg = c;
          }
        if let Some(c) = surcharge.prompt_fg
          {
            self.prompt_fg = c;
          }
        if let Some(c) = surcharge.prompt_bg
          {
            self.prompt_bg = c;
          }
      }

    /// Éclaircit les couleurs vers le blanc : texte à `ECLAIRCISSEMENT_WHEEL`,
    /// fonds (stat, contexte, prompt) à `ECLAIRCISSEMENT_WHEEL_FOND`.
    fn eclaircir(&mut self)
      {
        let texte = ECLAIRCISSEMENT_WHEEL;
        let fond = ECLAIRCISSEMENT_WHEEL_FOND;
        self.stat_fg = eclaircir_rgb(self.stat_fg, texte);
        self.stat_bg = eclaircir_rgb(self.stat_bg, fond);
        self.contexte_fg = eclaircir_rgb(self.contexte_fg, texte);
        self.contexte_bg = eclaircir_rgb(self.contexte_bg, fond);
        self.prompt_fg = eclaircir_rgb(self.prompt_fg, texte);
        self.prompt_bg = eclaircir_rgb(self.prompt_bg, fond);
        self.prompt_chemin = eclaircir_rgb(self.prompt_chemin, texte);
        self.prompt_historique = eclaircir_rgb(self.prompt_historique, texte);
        self.prompt_heure = eclaircir_rgb(self.prompt_heure, texte);
      }
  }

/// Mélange une couleur vers le blanc (`facteur` : 0 → inchangée, 1 → blanc).
fn eclaircir_rgb(couleur: Rgb, facteur: f32) -> Rgb
  {
    let f = facteur.clamp(0.0, 1.0);
    let melange = |v: u8| (f32::from(v) + (255.0 - f32::from(v)) * f).round() as u8;
    Rgb::new(melange(couleur.r), melange(couleur.g), melange(couleur.b))
  }

/// Résout la palette effective d'un thème, surcharges utilisateur appliquées.
///
/// `classique` rend exactement les couleurs choisies ; `wheel` reprend
/// `classique` (personnalisé ou non) nettement éclairci ; `root` reste **fixe**
/// et ignore toute personnalisation.
pub fn palette(theme: &Theme, surcharge: &Surcharge) -> Palette
  {
    if theme.nom == "root"
      {
        return Palette {
          stat_fg: TEXTE_CADRE,
          stat_bg: ROOT.fond,
          contexte_fg: TEXTE_CADRE,
          contexte_bg: ROOT.fond,
          prompt_fg: ROOT.accent,
          prompt_bg: ROOT.fond,
          prompt_chemin: COULEUR_CHEMIN,
          prompt_historique: COULEUR_HISTORIQUE,
          prompt_heure: COULEUR_HEURE,
        };
      }
    let mut palette = Palette::defaut();
    palette.appliquer(surcharge);
    if theme.nom == "wheel"
      {
        palette.eclaircir();
      }
    palette
  }

/// Résout un thème par son nom (repli sur `classique`).
pub fn par_nom(nom: &str) -> Theme
  {
    match nom.to_ascii_lowercase().as_str()
      {
        "wheel" => WHEEL,
        "root" => ROOT,
        _ => CLASSIQUE,
      }
  }

/// Résout le thème effectif : `auto` (ou vide) suit le privilège détecté.
///
/// Le groupe d'administration est paramétré selon l'OS hôte (voir
/// `detect::privilege`) : `wheel`/`sudo` sous Linux, `admin` sous macOS,
/// `operator` sous FreeBSD, `Administrateurs` sous Windows.
pub fn resoudre(nom: &str, privilege: crate::detect::Privilege) -> Theme
  {
    match nom.to_ascii_lowercase().as_str()
      {
        "" | "auto" => match privilege
          {
            crate::detect::Privilege::Root => ROOT,
            crate::detect::Privilege::Wheel => WHEEL,
            crate::detect::Privilege::Classique => CLASSIQUE,
          },
        _ => par_nom(nom),
      }
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn le_theme_inconnu_retombe_sur_classique()
      {
        assert_eq!(par_nom("inconnu"), CLASSIQUE);
      }

    #[test]
    fn les_noms_resolvent_les_bons_themes()
      {
        assert_eq!(par_nom("wheel"), WHEEL);
        assert_eq!(par_nom("ROOT"), ROOT);
      }

    #[test]
    fn les_glyphes_de_suffixe_sont_distincts()
      {
        assert_eq!(CLASSIQUE.suffixe.glyphe, ">>");
        assert_eq!(WHEEL.suffixe.glyphe, ">>>>");
        assert_eq!(ROOT.suffixe.glyphe, "###>");
      }

    #[test]
    fn le_theme_auto_suit_le_privilege()
      {
        use crate::detect::Privilege;
        assert_eq!(resoudre("auto", Privilege::Classique), CLASSIQUE);
        assert_eq!(resoudre("auto", Privilege::Wheel), WHEEL);
        assert_eq!(resoudre("", Privilege::Root), ROOT);
      }

    #[test]
    fn le_theme_explicite_prime_sur_le_privilege()
      {
        use crate::detect::Privilege;
        assert_eq!(resoudre("classique", Privilege::Root), CLASSIQUE);
      }

    #[test]
    fn le_theme_root_a_un_fond_rouge_distinct()
      {
        fn tire_vers_le_rouge(couleur: Rgb) -> bool
          {
            couleur.r > couleur.g && couleur.r > couleur.b
          }
        const FOND_NEUTRE: Rgb = Rgb::new(0x2E, 0x34, 0x40);
        assert_eq!(CLASSIQUE.fond, FOND_NEUTRE);
        assert_eq!(WHEEL.fond, FOND_NEUTRE);
        assert_ne!(ROOT.fond, FOND_NEUTRE);
        assert!(tire_vers_le_rouge(ROOT.fond), "le fond root doit tirer vers le rouge");
      }

    #[test]
    fn la_palette_classique_rend_les_surcharges_telles_quelles()
      {
        let rouge = Rgb::new(0xFF, 0x00, 0x00);
        let surcharge = Surcharge { stat_fg: Some(rouge), prompt_bg: Some(rouge), ..Surcharge::default() };
        let p = palette(&CLASSIQUE, &surcharge);
        assert_eq!(p.stat_fg, rouge, "la surcharge est rendue sans altération");
        assert_eq!(p.prompt_bg, rouge);
        assert_eq!(p.stat_bg, FOND_CADRE, "les couleurs non surchargées restent par défaut");
        assert_eq!(p.prompt_fg, CLASSIQUE.accent);
      }

    #[test]
    fn la_palette_wheel_eclaircit_le_classique()
      {
        let classique = palette(&CLASSIQUE, &Surcharge::default());
        let wheel = palette(&WHEEL, &Surcharge::default());
        assert!(
          wheel.stat_bg.r > classique.stat_bg.r
            && wheel.stat_bg.g > classique.stat_bg.g
            && wheel.stat_bg.b > classique.stat_bg.b,
          "le fond wheel doit être nettement plus clair : {wheel:?}"
        );
        assert!(
          wheel.prompt_fg.r >= classique.prompt_fg.r
            && wheel.prompt_fg.g >= classique.prompt_fg.g
            && wheel.prompt_fg.b >= classique.prompt_fg.b,
          "l'accent wheel doit être plus clair : {wheel:?}"
        );
      }

    #[test]
    fn le_fond_wheel_est_moins_eclairci_que_le_texte()
      {
        let classique = palette(&CLASSIQUE, &Surcharge::default());
        let wheel = palette(&WHEEL, &Surcharge::default());
        assert_eq!(wheel.stat_bg, eclaircir_rgb(classique.stat_bg, ECLAIRCISSEMENT_WHEEL_FOND));
        assert_eq!(wheel.contexte_bg, eclaircir_rgb(classique.contexte_bg, ECLAIRCISSEMENT_WHEEL_FOND));
        assert_eq!(wheel.prompt_bg, eclaircir_rgb(classique.prompt_bg, ECLAIRCISSEMENT_WHEEL_FOND));
        assert_eq!(wheel.stat_fg, eclaircir_rgb(classique.stat_fg, ECLAIRCISSEMENT_WHEEL));
      }

    #[test]
    fn la_palette_root_ignore_la_personnalisation()
      {
        let vert = Rgb::new(0x00, 0xFF, 0x00);
        let surcharge = Surcharge { stat_bg: Some(vert), prompt_fg: Some(vert), ..Surcharge::default() };
        let p = palette(&ROOT, &surcharge);
        assert_eq!(p.stat_bg, ROOT.fond, "root garde son fond rouge");
        assert_eq!(p.prompt_fg, ROOT.accent, "root garde son accent");
        assert_eq!(p.prompt_bg, ROOT.fond);
      }

    #[test]
    fn eclaircir_borne_les_extremes()
      {
        assert_eq!(eclaircir_rgb(Rgb::new(0, 0, 0), 0.0), Rgb::new(0, 0, 0));
        assert_eq!(eclaircir_rgb(Rgb::new(10, 20, 30), 1.0), Rgb::new(255, 255, 255));
        assert_eq!(eclaircir_rgb(Rgb::new(0, 0, 0), 0.5), Rgb::new(128, 128, 128));
      }
  }
