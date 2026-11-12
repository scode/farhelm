//! The tmux version floor and the parser for `tmux -V` lines, which remote
//! provisioning still uses to decide whether a Linux host's own tmux is good
//! enough or Farhelm's private build has to be installed beside it.
//!
//! NOTE: Nothing else in an installation looks for tmux any more. The
//! supervisor runs its sessions in terminal hosts, and the one tmux-related
//! thing it still does at startup, ending the private server an earlier
//! release ran sessions in, needs no tmux program at all
//! (`service::core::old_terminals`). Neither `farhelm helm setup` nor the
//! desktop app probes, requires or names a tmux. Provisioning's check
//! (`farhelm_helm`'s `tmux_meets_floor`) is the last user of this module.

use anyhow::{Context, bail};

/// A tmux release as Farhelm orders it: the numeric `major.minor` pair
/// plus tmux's patch-release letter.
///
/// The letter is not decoration, which is why this type exists at all.
/// [`TMUX_FLOOR`] names one specific regression-tested build, and the
/// letter is the only thing distinguishing it from the distinct
/// below-floor releases it shares a `major.minor` with: 3.7, 3.7a and
/// 3.7b are three separate releases, and 3.7b in particular is heavily
/// exercised in this file's own history — below the current pin, not
/// untried. The version check this replaced compared `(major, minor)`
/// alone, so it could not tell them apart and would have accepted any of
/// them as the pinned one.
///
/// Ordering is the derived lexicographic one, and it matches tmux's own
/// release order because `None < Some('a')`: a bare `3.7` shipped before
/// `3.7a`, which shipped before `3.7b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TmuxVersion {
    major: u32,
    minor: u32,
    /// tmux's patch-release letter (`3.7b`'s `b`); `None` for a bare
    /// `3.7`. At most one lowercase letter — see [`parse_tmux_version`]
    /// for why anything else is refused rather than approximated.
    patch: Option<char>,
}

impl std::fmt::Display for TmuxVersion {
    /// Round-trips [`parse_tmux_version`]'s input spelling, because this
    /// is what the release pin is compared against — a version that
    /// printed differently from how tmux spells it would make that
    /// comparison meaningless.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)?;
        if let Some(patch) = self.patch {
            write!(formatter, "{patch}")?;
        }
        Ok(())
    }
}

/// The oldest tmux Farhelm will drive: the version its own regression
/// suites are run against, not the oldest one that happens to work.
///
/// Farhelm drives tmux far harder than interactive use does — control
/// mode, output-client teardown, pane-death timing — and versions are not
/// interchangeable there. Crashes have been observed on older ones (a
/// distro 3.4 server hosting live sessions aborted on 2026-08-19, and
/// BUGS.md records the same abort class reproduced on distro 3.6), while
/// the pinned build is the one the release gate's tests run against. So
/// the floor tracks the pinned build in `.github/release/source-pins.env`
/// rather than tracking what distributions ship.
///
/// It is DESIGNED to exclude current distro packages (Ubuntu 24.04 ships
/// 3.4, 26.04 about 3.6, Debian 13 and Fedora 42 3.5a). A remote host whose
/// own tmux is below it gets Farhelm's private build from provisioning.
///
/// Bumping it is a deliberate act taken TOGETHER with the pinned build and
/// its regression suite, never on its own. The tests'
/// `floor_and_release_pin_cannot_drift` enforces that half mechanically:
/// it fails the moment this constant and `TMUX_VERSION=` in
/// `.github/release/source-pins.env` disagree.
pub const TMUX_FLOOR: TmuxVersion = TmuxVersion {
    major: 3,
    minor: 7,
    patch: Some('c'),
};

/// The exact prefix `tmux -V` puts in front of the version. Nothing else
/// is accepted: a line that does not start this way did not come from a
/// tmux this project knows how to reason about.
const TMUX_VERSION_LINE_PREFIX: &str = "tmux ";

/// Parse a whole `tmux -V` line — `"tmux 3.7b\n"` — into an orderable
/// version.
///
/// Conservative by contract: anything this cannot read exactly is an
/// error, never a guess. Provisioning turns that error into "install our
/// own build", which is the safe direction — a version
/// Farhelm cannot name is a version nobody has audited it against, and
/// guessing high buys a wedged or crashing substrate.
///
/// THE WHOLE LINE is validated, not just a token lifted out of it. An
/// earlier revision took `split_whitespace().nth(1)`, which happily read
/// `not-tmux 3.7c`, `tmux 3.7c vendor-patch` and `tmux +3.07c` as clean
/// releases. That is not a cosmetic gap: this same parser decides whether
/// provisioning SKIPS installing Farhelm's own audited build, so a
/// decorated or forged line that scans as an audited release is exactly
/// how an unaudited substrate gets adopted silently. So: the `tmux `
/// prefix exactly, one version token and nothing after it, at most one
/// trailing newline, and canonical decimal components (no sign, no
/// leading zeros).
///
/// It takes the whole line rather than a bare number because that is the
/// shape callers have — the provisioning probe forwards a remote host's
/// `tmux -V` output verbatim, and "tmux is not installed" text must fail
/// parsing rather than resemble a version.
///
/// DELIBERATELY NARROW, and this is a product decision rather than a
/// parser limitation: Farhelm supports stable releases and tmux's
/// single-letter patch releases, so tmux's official development
/// (`next-3.8`) and release-candidate (`3.8-rc`, `3.8-rc2`) spellings are
/// refused even though they are numerically above the floor. Ordering
/// those stages against a stable release is guesswork, so a host running
/// one gets Farhelm's own build like any other host it cannot vouch for.
pub fn parse_tmux_version(v_output: &str) -> anyhow::Result<TmuxVersion> {
    // Exactly one optional trailing newline; a second one, a `\r`, or any
    // other trailing whitespace falls through to the token check below
    // and is refused there.
    let line = v_output.strip_suffix('\n').unwrap_or(v_output);
    let version = line
        .strip_prefix(TMUX_VERSION_LINE_PREFIX)
        .with_context(|| {
            format!("tmux -V returned {line:?}, which is not a {TMUX_VERSION_LINE_PREFIX:?} line")
        })?;
    if version.is_empty() || version.contains(char::is_whitespace) {
        bail!("tmux -V returned {line:?}, which is not exactly one version token");
    }
    parse_tmux_version_number(version)
        .with_context(|| format!("tmux -V returned an unrecognized version {version:?}"))
}

/// [`parse_tmux_version`] minus the `tmux ` prefix and the line framing.
///
/// The error context naming the offending token is added by the caller,
/// which knows how the token was framed.
fn parse_tmux_version_number(version: &str) -> anyhow::Result<TmuxVersion> {
    let (major, rest) = version
        .split_once('.')
        .context("expected a major.minor version")?;
    let major = parse_version_component(major, "major")?;
    let suffix_at = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    let (minor, suffix) = rest.split_at(suffix_at);
    let minor = parse_version_component(minor, "minor")?;
    // tmux's own patch spelling is exactly one lowercase letter. Anything
    // longer is a build this project has never seen, and pretending the
    // first letter of it is a patch level would silently admit it.
    let patch = match suffix.as_bytes() {
        [] => None,
        [letter] if letter.is_ascii_lowercase() => Some(char::from(*letter)),
        _ => bail!("unrecognized patch suffix {suffix:?}"),
    };
    Ok(TmuxVersion {
        major,
        minor,
        patch,
    })
}

/// One numeric component of a version, in tmux's own canonical spelling.
///
/// Stricter than `u32::from_str` on purpose. `parse` accepts `+3` and
/// `0007`, both of which would then round-trip through [`TmuxVersion`]'s
/// `Display` as a DIFFERENT string from the one tmux printed — and the
/// release-pin lockstep test and provisioning's floor comparison both
/// trade in those printed forms. Refusing the
/// non-canonical spellings keeps parse and print exact inverses.
fn parse_version_component(text: &str, which: &str) -> anyhow::Result<u32> {
    if text.is_empty() {
        bail!("the {which} version is empty");
    }
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("the {which} version {text:?} is not a plain decimal number");
    }
    if text.len() > 1 && text.starts_with('0') {
        bail!("the {which} version {text:?} has a leading zero");
    }
    text.parse()
        .with_context(|| format!("parsing the {which} version {text:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Version ordering must separate a release from its own patch
    /// letters, which is the whole reason [`TmuxVersion`] exists: the
    /// check this replaced compared `(major, minor)` and could not tell
    /// the regression-tested 3.7c from a 3.7 nobody has run Farhelm on.
    ///
    /// Pins the two orderings that are easy to get wrong — a bare release
    /// sorts BEFORE its first patch letter (tmux shipped 3.7 then 3.7a),
    /// and the minor number is numeric, not textual, so 3.10 is newer
    /// than 3.9 rather than older.
    #[farhelm_testtrace::test]
    fn version_ordering_separates_patch_letters_and_compares_minors_numerically() {
        let parse = |line: &str| parse_tmux_version(line).expect("a well-formed version");
        assert!(parse("tmux 3.7") < parse("tmux 3.7a"));
        assert!(parse("tmux 3.7a") < parse("tmux 3.7b"));
        assert!(parse("tmux 3.7b") < parse("tmux 3.7c"));
        assert!(parse("tmux 3.7c") < parse("tmux 3.8"));
        assert!(parse("tmux 3.9") < parse("tmux 3.10"));
        assert!(parse("tmux 3.10") < parse("tmux 4.0"));
        assert_eq!(parse("tmux 3.7c"), TMUX_FLOOR);
    }

    /// Parsing is the trust boundary in front of the floor, so what it
    /// REFUSES matters as much as what it reads: every shape here would
    /// otherwise have to be guessed at, and a wrong guess admits an
    /// unaudited substrate. Also pins the round-trip through `Display`,
    /// since the release-pin lockstep test compares printed versions.
    #[farhelm_testtrace::test]
    fn version_parsing_refuses_everything_it_cannot_read_exactly() {
        for line in ["tmux 3.7b", "tmux 3.7", "tmux 4.0", "tmux 3.10"] {
            let parsed = parse_tmux_version(line).expect("a well-formed version");
            assert_eq!(format!("tmux {parsed}"), line, "Display must round-trip");
        }
        // A trailing newline is the shape tmux actually emits, so it must
        // parse identically to the trimmed line.
        assert_eq!(
            parse_tmux_version("tmux 3.7c\n").expect("tmux's own output"),
            TMUX_FLOOR
        );
        // No version token at all, a development build, a decorated
        // distro string, and a suffix that is not tmux's single lowercase
        // patch letter.
        for line in [
            "",
            "tmux",
            "not-a-version",
            "tmux next-3.8",
            "tmux 3.7-rc",
            "tmux 3.7ab",
            "tmux 3.7B",
            "tmux 3",
            "tmux .7",
            "tmux 3.a",
        ] {
            assert!(
                parse_tmux_version(line).is_err(),
                "{line:?} must not parse as a version"
            );
        }
        // Whole-line shapes an earlier token-only parser read as clean
        // releases. Each one is a way an unaudited or forged substrate
        // could have been classified as the pinned build — the wrong
        // program answering, a vendor decoration, or a non-canonical
        // spelling that would then print back differently than it came
        // in. `tmux 3.8-rc` and `tmux next-3.8` above are the same
        // refusal by policy rather than by accident: see
        // [`parse_tmux_version`] on why tmux's official development and
        // release-candidate stages are out of contract.
        for line in [
            "not-tmux 3.7c",
            "TMUX 3.7c",
            "tmux3.7c",
            " tmux 3.7c",
            "tmux 3.7c vendor-patch",
            "tmux 3.7c\ntmux 3.6",
            "tmux 3.7c\n\n",
            "tmux 3.7c\r\n",
            "tmux 3.7c ",
            "tmux +3.7c",
            "tmux 3.+7c",
            "tmux -3.7c",
            "tmux 03.7c",
            "tmux 3.07c",
            "tmux 3.8-rc",
            "tmux 3.8-rc2",
        ] {
            assert!(
                parse_tmux_version(line).is_err(),
                "{line:?} must not parse as a version"
            );
        }
    }

    /// The floor and the release pin must not be able to drift apart.
    ///
    /// [`TMUX_FLOOR`] means "the version Farhelm's regression suites
    /// actually run against", and the build that gets regression-tested is
    /// the one `.github/release/source-pins.env` names. Nothing else
    /// couples the two files, so bumping the pinned tarball without
    /// bumping the floor would silently leave the floor pointing at a
    /// build the project no longer ships — and bumping the floor alone
    /// would refuse the build provisioning installs. Failing here is the
    /// intended way to learn that the other half of the bump is missing.
    ///
    /// EVERY assignment is collected and exactly one is required, rather
    /// than reading the first. The pin file is SOURCED by shell scripts,
    /// where the LAST assignment wins, so an old line left above a new one
    /// would leave this test comparing the floor against a version nothing
    /// actually builds — the precise drift it exists to catch, dressed as
    /// a pass.
    #[farhelm_testtrace::test]
    fn floor_and_release_pin_cannot_drift() {
        // Reaching outside the crate with a relative `include_str!` is
        // fine here: this is a workspace-only repository, never packaged
        // or published as a standalone crate.
        const SOURCE_PINS: &str = include_str!("../../../.github/release/source-pins.env");
        let pinned: Vec<&str> = SOURCE_PINS
            .lines()
            .filter_map(|line| line.trim().strip_prefix("TMUX_VERSION="))
            .collect();
        assert_eq!(
            pinned.len(),
            1,
            "source-pins.env must declare TMUX_VERSION exactly once, found {pinned:?}"
        );
        assert_eq!(
            pinned[0],
            TMUX_FLOOR.to_string(),
            "TMUX_FLOOR and the pinned tmux release must be bumped together"
        );
    }
}
