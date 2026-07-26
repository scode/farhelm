//! Wire types and protocol version shared by the helm and supervisors.
//!
//! This crate is the seam that keeps helm and supervisor honestly
//! decoupled: they meet only over the framing protocol defined here, even
//! when both run in the same process (SPEC_impl.md, "Workspace layout").
//! M1 fills in the framing (length-prefixed channels, JSON control frames,
//! raw data frames) and the version hello; in M0 this is a stub so the
//! workspace shape and CI exist before product code does.

/// Protocol version exchanged in the connection hello.
///
/// Incompatible frame changes bump this; the connecting side refuses a
/// mismatch with a clear error per SPEC.md's version-skew rule. Build
/// versions travel separately and are diagnostic only.
pub const PROTOCOL_VERSION: u32 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    /// The workspace starts at protocol version 0; the first real frame
    /// definition in M1 keeps or bumps this deliberately, never
    /// accidentally. This test exists so `cargo test` exercises the crate
    /// from the first CI run.
    #[test]
    fn protocol_version_is_zero_until_first_real_frames() {
        assert_eq!(PROTOCOL_VERSION, 0);
    }
}
