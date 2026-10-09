//! Build classification shared by native release decisions and browser wording.
//!
//! A development build has no published release to update from. Keeping this
//! predicate here lets the web UI describe the same build the helm refuses to
//! treat as a release, without depending on the native helm crate.

/// Whether `version` is `0.0.0` with any SemVer prerelease.
///
/// Main reports `0.0.0-unreleased`, but the rule also accepts other prereleases
/// on that version. A dev release such as `1.2.3-dev.1` is a release for this
/// purpose. An invalid version is not evidence of a development build.
pub fn is_development_build(version: &str) -> bool {
    semver::Version::parse(version)
        .is_ok_and(|v| v.major == 0 && v.minor == 0 && v.patch == 0 && !v.pre.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build wording and native update decisions must classify the same stamp.
    /// Pin the general prerelease rule, including metadata, rather than only
    /// main's current literal; real dev releases must keep their release role.
    #[farhelm_testtrace::test]
    fn a_development_build_is_zero_zero_zero_with_a_prerelease() {
        for development in ["0.0.0-unreleased", "0.0.0-dev.1", "0.0.0-unreleased+abc"] {
            assert!(is_development_build(development), "{development}");
        }
        for other in [
            "0.0.0",
            "0.1.1",
            "0.14.0-rc.2",
            "1.2.3-dev.1",
            "1.0.0-unreleased",
            "garbage",
            "",
        ] {
            assert!(!is_development_build(other), "{other}");
        }
    }
}
