//! Cadre permanent : `term_stat` (ligne 1) et `term_context` (dernière ligne).

pub mod context;
pub mod stat;

/// Fond commun des trois lignes du cadre (`term_stat`, prompt, `term_context`).
pub const FOND_CADRE: (u8, u8, u8) = (0x2E, 0x34, 0x40);
