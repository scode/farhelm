//! The helm: Farhelm's single control-plane process.
//!
//! Per SPEC.md's Concepts, exactly one helm runs at a time. It holds the
//! host registry, connects directly to each supervisor (over the user's
//! ssh for remote hosts, locally for its own machine), aggregates their
//! sessions, and serves the UI over loopback HTTP/WS (axum, per
//! SPEC_impl.md). It holds no authoritative session state — supervisors
//! are the authority — so it can restart freely. M1 fills this in; the M0
//! stub exists so the workspace shape and CI precede product code.

/// Placeholder for the M0 CI pipeline; replaced by real modules in M1.
pub fn crate_name() -> &'static str {
    "farhelm-helm"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exists so `cargo test` compiles and exercises this crate from the
    /// first CI run; replaced by real tests as M1 lands functionality.
    #[test]
    fn stub_compiles_and_runs() {
        assert_eq!(crate_name(), "farhelm-helm");
    }
}
