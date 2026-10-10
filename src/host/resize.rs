//! Redimensionnement : gestion de `SIGWINCH` (Unix) et du resize ConPTY.

/// Rôle de la gestion du redimensionnement.
pub const ROLE: &str = "propager la taille du terminal au pty enfant";
