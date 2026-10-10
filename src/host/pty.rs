//! Allocation et pilotage du pseudo-terminal.
//!
//! M1 (Unix) : `forkpty`/`openpty` via la crate `portable-pty`.
//! M3 (Windows) : ConPTY.

/// Stratégie d'allocation retenue.
pub const STRATEGIE: &str = "portable-pty (forkpty Unix, ConPTY Windows)";
