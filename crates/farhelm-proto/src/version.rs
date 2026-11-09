//! Build classification and the shared boundary for supported update mechanics.
//!
//! A development build has no published release to update from. Keeping this
//! predicate here lets the web UI describe the same build the helm refuses to
//! treat as a release, without depending on the native helm crate.

/// Oldest release whose update mechanics this release supports.
///
/// Raise this only in a change that relies on upgrade logic an earlier release
/// shipped, and surface the raise in its PR and changelog. The release procedure
/// asks the maintainer to accept that boundary; this says nothing about which
/// feature data survives a reinstall. Keep the brick-test baseline in agreement.
pub const UPGRADE_FLOOR: &str = "0.23.0";

/// Whether a known release is too old for the supplied update floor.
///
/// Unknown versions and development builds are not evidence of an unsupported
/// source. Published dev releases and RCs still follow ordinary SemVer precedence,
/// while build metadata cannot make an otherwise equal version older. Callers
/// supply the floor so tests need not depend on today's production boundary.
pub fn below_upgrade_floor(version: &str, floor: &str) -> bool {
    if is_development_build(version) {
        return false;
    }
    match (
        semver::Version::parse(version),
        semver::Version::parse(floor),
    ) {
        (Ok(version), Ok(floor)) => version.cmp_precedence(&floor).is_lt(),
        _ => false,
    }
}

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

    /// A floor excludes old release mechanics, not development or unknown builds.
    /// Literal inputs keep this contract stable when a later release raises it.
    #[farhelm_testtrace::test]
    fn the_upgrade_floor_uses_release_precedence_and_exempts_unknown_builds() {
        for (version, below) in [
            ("1.1.9", true),
            ("1.2.0-rc.1", true),
            ("1.1.9-dev.1", true),
            ("1.2.0", false),
            ("1.2.0+different-build", false),
            ("1.2.1", false),
            ("0.0.0", true),
            ("0.0.0-unreleased", false),
            ("0.0.0-other+build", false),
            ("unknown", false),
            ("", false),
        ] {
            assert_eq!(below_upgrade_floor(version, "1.2.0"), below, "{version}");
        }
        assert!(!below_upgrade_floor("1.0.0", "unknown"));
        assert!(!below_upgrade_floor("1.2.0+aaa", "1.2.0+zzz"));
    }

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
