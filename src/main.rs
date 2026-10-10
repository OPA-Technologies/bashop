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
            let cible = shell.as_deref().and_then(Shell::par_nom);
            if shell.is_some() && cible.is_none()
              {
                println!("Shell inconnu : {}", shell.as_deref().unwrap_or(""));
              }
            else
              {
                let touches = bashop::install::installer(cible);
                if touches.is_empty()
                  {
                    println!("bashop install : aucun rc modifié (shell non pris en charge ou déjà à jour)");
                  }
                else
                  {
                    for chemin in touches
                      {
                        println!("bootstrap posé : {}", chemin.display());
                      }
                  }
              }
          }
        Command::Uninstall =>
          {
            let touches = bashop::install::desinstaller();
            if touches.is_empty()
              {
                println!("bashop uninstall : aucune trace à retirer");
              }
            else
              {
                for chemin in touches
                  {
                    println!("bootstrap retiré : {}", chemin.display());
                  }
              }
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
            let actif = match state.to_ascii_lowercase().as_str()
              {
                "on" => true,
                "off" => false,
                "toggle" => !config::charger().capture,
                _ => {
                  println!("État attendu : on, off ou toggle (reçu : {state})");
                  return Ok(());
                }
              };
            config::fixer_capture(actif)?;
            println!("Mode capture : {}", if actif { "on" } else { "off" });
          }
        Command::Guard =>
          {
            if !bashop::detect::autoriser_cadre()
              {
                std::process::exit(1);
              }
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
