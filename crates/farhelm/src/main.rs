//! The `farhelm` multi-call binary.
//!
//! One artifact carries every role — `helm run`, `supervisor run`,
//! `spawn`, and the hidden `internal` namespace — because provisioning
//! copies exactly one binary to a host, and the spawn CLI must exist
//! inside every session without separate installation (SPEC_impl.md,
//! "CLI"). The clap subcommand grammar arrives in M1; the M0 stub only
//! anchors the workspace and CI.

fn main() {
    // M0 stub: the real clap subcommand grammar (helm run, supervisor
    // run, spawn, internal ...) lands in M1 per PLAN_M1.md.
    println!("farhelm {} (M0 stub)", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    /// Exists so `cargo test` compiles and exercises this crate from the
    /// first CI run; replaced by real tests as M1 lands functionality.
    #[test]
    fn stub_compiles_and_runs() {
        assert_eq!(env!("CARGO_PKG_NAME"), "farhelm");
    }
}
