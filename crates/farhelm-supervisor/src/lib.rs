//! The supervisor: per-host session management and nothing else.
//!
//! Per SPEC.md's Concepts, the supervisor launches agents, owns their
//! terminals (a private tmux server consumed over control mode, per
//! SPEC_impl.md), receives attachments, and handles spawn requests. It has
//! no UI and no knowledge of other hosts, listens on no network port
//! (reached over a unix socket, remotely via ssh exec), and is the
//! authority on its sessions. M1 fills this in; the M0 stub exists so the
//! workspace shape and CI precede product code.

/// Placeholder for the M0 CI pipeline; replaced by real modules in M1.
pub fn crate_name() -> &'static str {
    "farhelm-supervisor"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exists so `cargo test` compiles and exercises this crate from the
    /// first CI run; replaced by real tests as M1 lands functionality.
    #[test]
    fn stub_compiles_and_runs() {
        assert_eq!(crate_name(), "farhelm-supervisor");
    }
}
