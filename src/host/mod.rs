//! Hôte PTY / mini-multiplexeur.
//!
//! Bashop prend possession du terminal, alloue un pseudo-terminal au shell et
//! dessine le cadre permanent autour de la zone centrale émulée.

pub mod pty;
pub mod pump;
pub mod resize;
