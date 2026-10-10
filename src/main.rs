use anyhow::Result;
use bashop::bootstrap;
use bashop::cli::{Cli, Command};
use bashop::config;
use bashop::shells::Shell;
use bashop::theme;
use clap::Parser;

fn main() -> Result<()>
  {
    let cli = Cli::parse();
    match cli.command
      {
        Command::Run { shell } =>
          {
            let config = config::charger();
            bashop::run::executer(shell, &config)?;
          }
        Command::Install { shell } =>
          {
            println!("bashop install — shell={:?} (bootstrap à finaliser en M4)", shell);
          }
        Command::Uninstall =>
          {
            println!("bashop uninstall (bootstrap à finaliser en M4)");
          }
        Command::Config =>
          {
            println!("bashop config (interface à finaliser en M4)");
          }
        Command::Detect =>
          {
            println!("{}", bashop::detect::resume());
          }
        Command::Template { shell } =>
          {
            afficher_template(&shell);
          }
        Command::Capture { state } =>
          {
            println!("bashop capture {state} (persistance à finaliser en M4)");
          }
      }
    Ok(())
  }

fn afficher_template(nom: &str)
  {
    let Some(shell) = Shell::par_nom(nom)
      else
        {
          println!("Shell inconnu : {nom} (attendu : bash, zsh, sh, powershell, cmd)");
          return;
        };
    let theme = theme::par_nom("classique");
    let largeur = config::SUFFIX_WIDTH_DEFAUT;
    match shell
      {
        Shell::Bash => println!("{}", bootstrap::prompt_bash(&theme, largeur)),
        Shell::Zsh => println!("{}", bootstrap::prompt_zsh(&theme, largeur)),
        _ => println!("Aucun template de prompt pour « {} ».", shell.nom()),
      }
  }
