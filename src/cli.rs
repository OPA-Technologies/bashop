//! Définition de l'interface en ligne de commande.
//!
//! Les sous-commandes sont volontairement minimales en M0 : chacune pose le
//! point d'entrée qui sera rempli aux jalons suivants.

use clap::{Parser, Subcommand};

/// Interface en ligne de commande de Bashop.
///
/// Le drapeau `-V`/`--version` applique le contrat d'op : il publie une seule
/// ligne `<version>_<statut> (<jj-mm-yyyy>)`, lue depuis les constantes de
/// [`crate`] plutôt que déduite du manifeste.
#[derive(Parser, Debug)]
#[command(name = "bashop", about = "Cadre terminal permanent et prompt thémé", disable_version_flag = true)]
pub struct Cli
  {
    /// Imprime la version de la variante (`<version>_<statut> (<date>)`) puis sort.
    #[arg(short = 'V', long = "version")]
    pub version: bool,

    /// Sous-commande à exécuter.
    #[command(subcommand)]
    pub command: Option<Command>,
  }

/// Sous-commandes disponibles.
#[derive(Subcommand, Debug)]
pub enum Command
  {
    /// Lance un shell à l'intérieur du cadre permanent.
    Run
      {
        /// Shell à lancer (défaut : le shell de l'utilisateur).
        #[arg(long)]
        shell: Option<String>,
      },
    /// Installe le bootstrap dans les rc des shells détectés.
    Install
      {
        /// Shell ciblé (défaut : tous les shells détectés).
        #[arg(long)]
        shell: Option<String>,
        /// Installe aussi le bloc système (`/etc/profile.d`, Linux) pour `sudo -i`.
        #[arg(long)]
        system: bool,
      },
    /// Retire le bootstrap des rc des shells.
    Uninstall,
    /// Ouvre l'interface de configuration (op-tui) ou affiche la config active.
    Config
      {
        /// Relit `~/.opshellrc` et affiche le résumé sans ouvrir l'interface.
        #[arg(long)]
        reload_rc: bool,
      },
    /// Affiche les informations de détection (OS, shell, terminal, privilège).
    Detect,
    /// Émet le template de prompt pour un shell donné.
    Template
      {
        /// Shell ciblé (bash, zsh, sh, powershell, cmd).
        shell: String,
      },
    /// Émet le code shell qui repose le prompt épinglé et les marqueurs OSC
    /// (utilisé en shell imbriqué, ex. `sudo -i`) ; à `eval`er.
    Prompt
      {
        /// Shell ciblé (bash, zsh, powershell, cmd).
        #[arg(long, default_value = "bash")]
        shell: String,
      },
    /// Active, désactive ou bascule le mode capture (persisté dans `~/.opshellrc`).
    Capture
      {
        /// État demandé : on, off ou toggle.
        #[arg(default_value = "toggle")]
        state: String,
      },
    /// (interne) Garde de nidification : sort 0 si le cadre peut être lancé.
    #[command(name = "_guard", hide = true)]
    Guard,
    /// (interne) Sort 0 si un ancêtre Bashop est déjà présent (cadre actif).
    #[command(name = "_nested", hide = true)]
    Nested,
  }
