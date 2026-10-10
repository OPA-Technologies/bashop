//! Bashop — cadre terminal permanent et prompt thémé, multi-shell et multi-OS.
//!
//! Le projet est un hôte PTY (mini-multiplexeur) : il prend possession du
//! terminal, lance le shell dans un pseudo-terminal et dessine trois lignes
//! figées (`term_stat`, `term_prompt`, `term_context`) autour d'une zone
//! centrale émulée où vit le shell.
//!
//! La référence de conception complète est `SPEC.md`. Ce module ne pose que
//! l'arborescence et les types fondateurs du jalon M0.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod bootstrap;
pub mod cli;
pub mod config;
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
