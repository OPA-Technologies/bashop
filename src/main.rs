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
        Command::Install { shell, system } =>
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
            if system
              {
                match bashop::install::installer_systeme()
                  {
                    Ok(chemin) => println!("bootstrap système posé : {}", chemin.display()),
                    Err(erreur) if erreur.kind() == std::io::ErrorKind::Unsupported => {
                      println!("bashop install --system : non pris en charge sur cet OS (Linux uniquement)");
                    }
                    Err(erreur) => {
                      println!(
                        "bashop install --system : échec ({erreur}) — relancez avec des droits root"
                      );
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
        Command::Config { reload_rc } =>
          {
            let config = config::charger();
            if reload_rc
              {
                println!("{}", bashop::config_ui::resume(&config));
              }
            else
              {
                let editee = bashop::config_ui::editer(config)?;
                let chemin = config::ecrire(&editee)?;
                println!("Configuration enregistrée : {}", chemin.display());
                println!("{}", bashop::config_ui::resume(&editee));
              }
          }
        Command::Detect =>
          {
            println!("{}", bashop::detect::resume());
          }
        Command::Template { shell } =>
          {
            afficher_template(&shell);
          }
        Command::Prompt { shell } =>
          {
            let config = config::charger();
            let theme = theme::resoudre(&config.theme, bashop::detect::privilege());
            let palette = theme::palette(&theme, &config.couleurs);
            let glyphe = theme.suffixe.glyphe;
            let ligne_vide = config.ligne_vide;
            match shell.to_ascii_lowercase().as_str()
              {
                "bash" => print!("{}", bootstrap::script_prompt_bash(&palette, glyphe, config.suffix_width, ligne_vide)),
                "zsh" => print!("{}", bootstrap::script_prompt_zsh(&palette, glyphe, config.suffix_width, ligne_vide)),
                "powershell" | "pwsh" => print!("{}", bootstrap::script_prompt_powershell(&palette, glyphe, config.suffix_width, ligne_vide)),
                "cmd" => print!("{}", bootstrap::prompt_cmd(&palette, glyphe, config.suffix_width, None)),
                _ => eprintln!("Shell non pris en charge pour prompt : {shell} (attendu : bash, zsh, powershell, cmd)"),
              }
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
        Command::Nested =>
          {
            if !bashop::detect::ancetre_bashop()
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
    let palette = theme::palette(&theme, &theme::Surcharge::default());
    let largeur = config::SUFFIX_WIDTH_DEFAUT;
    match shell
      {
        Shell::Bash => println!("{}", bootstrap::prompt_bash(&palette, theme.suffixe.glyphe, largeur)),
        Shell::Zsh => println!("{}", bootstrap::prompt_zsh(&palette, theme.suffixe.glyphe, largeur)),
        Shell::PowerShell => println!("{}", bootstrap::script_prompt_powershell(&palette, theme.suffixe.glyphe, largeur, true)),
        Shell::Cmd => println!("{}", bootstrap::prompt_cmd(&palette, theme.suffixe.glyphe, largeur, None)),
        Shell::Sh => println!("Aucun template de prompt pour « {} ».", shell.nom()),
      }
  }
