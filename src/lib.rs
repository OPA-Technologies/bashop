//! Bashop — cadre terminal permanent et prompt thémé, multi-shell et multi-OS.
//!
//! Le projet est un hôte PTY (mini-multiplexeur) : il prend possession du
//! terminal, lance le shell dans un pseudo-terminal et dessine trois lignes
//! figées (`term_stat`, `term_prompt`, `term_context`) autour d'une zone
//! centrale émulée où vit le shell.
//!
//! La référence de conception complète est `SPEC.md`. Ce module ne pose que
//! l'arborescence et les types fondateurs du jalon M0.

/// Version publiée, figée à la compilation (`CARGO_PKG_VERSION`).
pub const RELEASE: &str = env!("CARGO_PKG_VERSION");

/// Alias historique de [`RELEASE`].
pub const VERSION: &str = RELEASE;

/// Statut du projet (source unique : ce fichier), valeur de la liste connue d'op.
pub const STATUT: &str = "Stable";

/// Date de dernière mise à jour, format `jj-mm-yyyy` (contrat `-V` d'op).
pub const DATE_LAST_MAJ: &str = "16-10-2026";

pub mod bootstrap;
pub mod cli;
pub mod config;
pub mod config_ui;
pub mod detect;
pub mod frame;
pub mod host;
pub mod install;
pub mod keymap;
pub mod lexer;
pub mod observe;
pub mod providers;
pub mod restauration;
pub mod run;
pub mod safety;
pub mod shells;
pub mod term;
pub mod theme;
