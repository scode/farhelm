//! Choosing the tmux the managed supervisor will run, and refusing startup
//! before any window exists when that tmux is missing or older than the
//! supported floor.

use super::*;

/// The macOS Homebrew/MacPorts prefixes `resolve_supervisor_tmux` should
/// probe, or an empty list everywhere else.
///
/// Kept as a thin `cfg!` wrapper (a runtime branch), not a `#[cfg(target_os =
/// "macos")]` item, so the branch itself — and everything it feeds — stays
/// compiled and exercised on every CI host, matching `tmux_probe`'s own
/// platform-agnostic design (see that module's docs).
pub(super) fn macos_tmux_prefixes() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        crate::tmux_probe::MACOS_TMUX_PREFIXES
    } else {
        &[]
    }
}

/// Real executability check behind the macOS tmux probe: a regular file with
/// at least one execute bit set.
///
/// `tmux_probe::find_tmux_in_prefixes` takes this as an injected predicate so
/// its search-ORDER logic can be unit-tested without touching a real
/// filesystem; this function is the one real implementation, used only at
/// this call site.
pub(super) fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Decide the `FARHELM_TMUX` value (if any) the managed supervisor should be
/// launched with.
///
/// Precedence, per SPEC_impl.md's "Terminal substrate: private tmux server"
/// ("the desktop app takes an ambient `FARHELM_TMUX` as given, otherwise
/// probes ..."): an `ambient_override` already present in the app's own
/// environment is passed through UNTOUCHED and the probe is
/// skipped entirely — the user's choice always wins, and probing anyway
/// would risk silently overruling it with a different Homebrew install found
/// first. Only absent an override does `prefixes` get searched (macOS only in
/// practice; every other platform passes an empty list here and leaves
/// resolution to the supervisor's own PATH lookup, unchanged from before this
/// probe existed).
///
/// `prefixes` and `is_executable` are parameters, not globals, purely so this
/// precedence can be unit-tested by passing in fake values directly —
/// without mutating `std::env` (which races every other test reading the
/// same process-wide environment) or touching a real filesystem.
pub(super) fn resolve_supervisor_tmux(
    ambient_override: Option<std::ffi::OsString>,
    prefixes: &[&str],
    is_executable: impl FnMut(&Path) -> bool,
) -> Option<std::ffi::OsString> {
    // An EMPTY value means "no override", exactly as the supervisor reads
    // it (a profile or unit that writes `FARHELM_TMUX=` means that to
    // whoever wrote it); passing it through would skip the probe and then
    // fall back to the bare `tmux` on Finder's PATH, which is the case the
    // probe exists for.
    if let Some(value) = ambient_override.filter(|value| !value.is_empty()) {
        return Some(value);
    }
    crate::tmux_probe::find_tmux_in_prefixes(prefixes, is_executable).map(PathBuf::into_os_string)
}

/// Which OS's install guidance a tmux refusal should give.
///
/// A parameter to [`tmux_refusal_message`] rather than a `cfg!` branch
/// baked into that function, for the same reason `macos_tmux_prefixes`
/// wraps its `cfg!` in a runtime branch: parameterizing the platform lets
/// CI exercise BOTH pure message variants on whatever host it runs on,
/// rather than compiling the macOS wording out wherever CI happens not to
/// be a Mac. The one real call site ([`run_tmux_preflight_or_exit`]) still
/// picks with `cfg!(target_os = "macos")`, once, at the moment it actually
/// matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TmuxRefusalPlatform {
    Mac,
    Linux,
}

/// The two ways this app's tmux preflight refuses startup outright before
/// spawning its managed supervisor, each rendered by
/// [`tmux_refusal_message`] as one plain stderr message followed by exit
/// status 1 — see `SPEC_impl.md`'s "Terminal substrate: private tmux
/// server" section for why the desktop owns this check at all.
///
/// Deliberately narrower than [`farhelm_supervisor::tmux::TmuxSupport`]:
/// `AtFloor` and `AbovePin` both mean "let it proceed" here (an above-pin
/// tmux still earns its own "unaudited" warning, but that is the
/// supervisor's concern once it actually starts a server, not this
/// preflight's), so [`classify_tmux_preflight`] folds both into `Ok` rather
/// than growing a matching third variant of this enum that no caller would
/// ever act on differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TmuxRefusal {
    /// The probe could not run `program` at all: nothing answered to that
    /// name or path (`io::ErrorKind::NotFound`).
    NotFound,
    /// `program` answered `-V`, but with a version below
    /// [`farhelm_supervisor::tmux::TMUX_FLOOR`].
    BelowFloor {
        found: farhelm_supervisor::tmux::TmuxVersion,
    },
}

/// Whether the tmux this app is about to hand its managed supervisor clears
/// Farhelm's version floor — the classification half of the preflight,
/// deliberately separated from running the probe itself.
///
/// Takes the ALREADY-COMPLETED [`farhelm_supervisor::tmux::probe_tmux`]
/// result rather than a program to probe or a callback that would run one:
/// classification is then pure data-in-data-out, so tests construct a
/// canned `TmuxProbe` or `TmuxProbeError` directly and never spawn a real
/// process — this preflight is exercised the same way regardless of
/// whether the host running the tests has a below-floor or missing tmux
/// anywhere on it. The real call site, [`run_tmux_preflight_or_exit`], is
/// the only thing that actually calls `probe_tmux`, and it does so through
/// the supervisor's own BOUNDED probe (a time limit, a captured-output
/// cap, and its own process group) rather than a raw `Command::output()` —
/// this preflight runs before any of Farhelm's other startup machinery
/// exists, so an unbounded probe against a hostile or merely broken
/// program named by `--tmux`/`FARHELM_TMUX` could otherwise hang or
/// exhaust memory in an already-invisible GUI process.
///
/// Any failure this cannot make sense of (a spawn failure that is not
/// `ENOENT`, a nonzero `-V` exit, unparseable `-V` output) is reported
/// through the ordinary `anyhow::Error` path instead of inventing a third
/// `TmuxRefusal` variant: those are genuinely unexpected, and this
/// preflight's specialized wording is specifically for the two predictable
/// ways a real Mac without an acceptable tmux fails, not for surprises it
/// has no tailored message for.
fn classify_tmux_preflight(
    program: &Path,
    probed: Result<farhelm_supervisor::tmux::TmuxProbe, farhelm_supervisor::tmux::TmuxProbeError>,
) -> anyhow::Result<Option<TmuxRefusal>> {
    use farhelm_supervisor::tmux::{TmuxProbeError, TmuxSupport};

    match probed {
        Ok(probe) => Ok(match probe.support {
            TmuxSupport::BelowFloor => Some(TmuxRefusal::BelowFloor {
                found: probe.version,
            }),
            TmuxSupport::AtFloor | TmuxSupport::AbovePin => None,
        }),
        // `ENOENT`: the same shape `std::process::Command` produces when
        // the OS could not find `program` at all, whether that means
        // nothing answered to that name/path or something answered but
        // could not be run (a missing shebang interpreter or dynamic
        // loader looks identical to a genuinely absent target on Unix —
        // see `tmux_refusal_message`'s `NotFound` wording).
        Err(TmuxProbeError::NotRunnable(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(Some(TmuxRefusal::NotFound))
        }
        Err(TmuxProbeError::NotRunnable(error)) => Err(error).with_context(|| {
            format!(
                "running {} -V during the desktop tmux preflight",
                program.display()
            )
        }),
        Err(error @ TmuxProbeError::Unparseable(_)) => {
            bail!(
                "{} -V did not produce a usable version: {error}",
                program.display()
            )
        }
        Err(error @ TmuxProbeError::Overran(_)) => {
            bail!("{} -V {error}", program.display())
        }
    }
}

/// Human labels for the places `resolve_supervisor_tmux` actually looked
/// this run, in the order it looked, for the "looked at: ..." clause of a
/// [`TmuxRefusal::NotFound`] message.
///
/// Mirrors `resolve_supervisor_tmux`'s own precedence exactly — an ambient
/// override skips every other probe there, so it must be the ONLY entry
/// here too, or the message would claim to have searched somewhere the
/// real resolution never looked. `MacPorts` is a friendlier label for
/// `/opt/local/bin` than the bare path would be; the other two prefixes are
/// self-explanatory Homebrew locations and are shown verbatim.
fn tmux_probe_targets(ambient: Option<&std::ffi::OsStr>, prefixes: &[&str]) -> Vec<String> {
    if let Some(value) = ambient.filter(|value| !value.is_empty()) {
        return vec![value.to_string_lossy().into_owned()];
    }
    let mut targets = vec![farhelm_supervisor::tmux::TMUX_PROGRAM_ENV.to_string()];
    targets.extend(prefixes.iter().map(|&prefix| {
        if prefix == "/opt/local/bin" {
            "MacPorts".to_string()
        } else {
            prefix.to_string()
        }
    }));
    targets.push("PATH".to_string());
    targets
}

/// Render `items` as an English list ending in "..., then LAST" — the shape
/// both refusal-message probe clauses use ("FARHELM_TMUX, /opt/homebrew/bin,
/// ..., then PATH"), because these are steps tried IN ORDER, and "then"
/// says that where a plain comma list would not.
fn join_with_then(items: &[String]) -> String {
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{}, then {last}", rest.join(", ")),
    }
}

/// Render the exact stderr text [`run_tmux_preflight_or_exit`] prints
/// before exiting — the whole required message: one plain sentence naming
/// what went wrong and where farhelm looked, one plain sentence saying how
/// to fix it, and nothing else — no panic, no backtrace hint, no
/// supervisor-shaped `WARN`/`Caused by` chain ahead of it.
///
/// Pure and parameterized over [`TmuxRefusalPlatform`] specifically so both
/// platforms' wording is exercised by plain `cargo test` on whatever host
/// runs it — the actual macOS wording would otherwise be unverified by
/// anything this project's CI runs. `probed` is threaded through rather
/// than recomputed so the message can never disagree with what
/// [`tmux_probe_targets`] (called once, at the real call site) says was
/// actually tried. `program` is the DISPLAY spelling the caller resolved
/// (see [`farhelm_supervisor::tmux::program_display_path`]) — a below-floor
/// refusal has to name the concrete binary that answered, not the bare
/// spelling `--tmux`/`FARHELM_TMUX` happened to carry, or two installs
/// sharing a bare name would be indistinguishable in the message.
///
/// `override_in_force` says whether an ambient `FARHELM_TMUX` — rather
/// than the fixed-prefix probe or a bare `PATH` lookup — is what selected
/// `program`. It changes the REMEDY, not the diagnosis: telling a user
/// whose override names a bad binary to "restart the app" is useless
/// advice, because restarting with the same unchanged override reproduces
/// the identical refusal without ever considering a freshly installed
/// tmux.
///
/// The floor version is read from [`farhelm_supervisor::tmux::TMUX_FLOOR`]
/// — the ONLY source of the "3.7c" text — so a future floor bump cannot
/// leave this message quoting a stale one.
fn tmux_refusal_message(
    refusal: TmuxRefusal,
    program: &Path,
    platform: TmuxRefusalPlatform,
    probed: &[String],
    override_in_force: bool,
) -> String {
    use farhelm_supervisor::tmux::TMUX_FLOOR;

    let subject = match refusal {
        // Deliberately does not claim outright that nothing EXISTS at any
        // of these locations: on Unix, the same `ENOENT` this preflight
        // classifies as `NotFound` also covers a script whose shebang
        // interpreter disappeared or a binary whose loader is missing, and
        // telling those two apart would need an existence check this
        // preflight does not perform (see `classify_tmux_preflight`).
        TmuxRefusal::NotFound => format!(
            "farhelm-desktop needs tmux {TMUX_FLOOR} or newer, and none could be run (looked \
             at: {}). Each one was either not found, or is missing its interpreter or loader.",
            join_with_then(probed)
        ),
        TmuxRefusal::BelowFloor { found } => format!(
            "found tmux {found} at {}, which is below the {TMUX_FLOOR} farhelm needs.",
            program.display()
        ),
    };
    let body = match (platform, override_in_force) {
        // The indented command block matches how the README and
        // provisioning already point Mac users at Homebrew — a bare
        // sentence mentioning `brew install tmux` reads like trivia next to
        // an exact command a person can paste.
        (TmuxRefusalPlatform::Mac, false) => "On macOS, tmux has to be installed by hand; \
             Homebrew is the recommended way:\n\n    brew install tmux\n\nThen start \
             farhelm-desktop again."
            .to_string(),
        (TmuxRefusalPlatform::Mac, true) => "On macOS, tmux has to be installed by hand; \
             Homebrew is the recommended way:\n\n    brew install tmux\n\nFARHELM_TMUX is set \
             and overrides that search, so update it to point at the new install (or unset it) \
             before starting farhelm-desktop again."
            .to_string(),
        (TmuxRefusalPlatform::Linux, false) => format!(
            "Install tmux {TMUX_FLOOR} or newer with your package manager or Linuxbrew (`brew \
             install tmux`), or point FARHELM_TMUX at one.\nThen start farhelm-desktop again."
        ),
        (TmuxRefusalPlatform::Linux, true) => format!(
            "Install tmux {TMUX_FLOOR} or newer with your package manager or Linuxbrew (`brew \
             install tmux`). FARHELM_TMUX is set and overrides where farhelm-desktop looks, so \
             update it to point at a supported tmux, or unset it, before starting \
             farhelm-desktop again."
        ),
    };
    format!("{subject}\n{body}")
}

/// Check the tmux this app is about to hand its managed supervisor against
/// Farhelm's floor, and if it refuses, print one plain message and exit —
/// called from [`DesktopBootstrap::start`]'s `Absent` branch, immediately
/// before that branch spawns the managed supervisor child, which is the
/// only situation where this process is about to choose and configure a
/// tmux at all (an answering supervisor is reused with whatever tmux it
/// already has, untouched — see `start`'s own doc comment on that
/// ordering).
///
/// `supervisor_tmux` is the SAME value [`DesktopBootstrap::start`] hands
/// its managed supervisor through `FARHELM_TMUX` (`None` meaning "let the
/// supervisor's own bare `tmux` / PATH lookup apply", exactly as it does
/// there) — resolved via [`farhelm_supervisor::tmux::resolve_tmux_program`]
/// so this preflight probes the IDENTICAL program the child would have
/// resolved to. Probing a different one would leave preflight and child
/// free to disagree, which is worse than no preflight: a green preflight
/// followed by the child's own refusal is the exact confusing, panicky
/// experience this whole feature exists to remove.
///
/// The probe itself goes through [`farhelm_supervisor::tmux::probe_tmux`],
/// not a raw `Command::output()`: this call happens before ANY of
/// Farhelm's other startup machinery exists, so the candidate named by
/// `--tmux`/`FARHELM_TMUX` gets the same bounded treatment (a time limit,
/// a captured-output cap, its own process group) the supervisor already
/// gives every operator-supplied tmux candidate, rather than a synchronous
/// call with no deadline that a wedged or chatty wrapper could hang or
/// exhaust memory in.
///
/// Any OTHER bootstrap failure (state directory unreadable, port already
/// bound, ...) is deliberately NOT this function's problem: it returns
/// `Ok(())` and lets `DesktopBootstrap::start` continue, so those failures
/// still surface through the ordinary `anyhow::Error` path that
/// `desktop::run` now prints plainly rather than panics on.
pub(super) fn run_tmux_preflight_or_exit(
    supervisor_tmux: Option<&std::ffi::OsStr>,
    ambient_tmux: Option<&std::ffi::OsStr>,
    prefixes: &[&str],
) -> anyhow::Result<()> {
    let program = farhelm_supervisor::tmux::resolve_tmux_program(None, supervisor_tmux);
    let refusal =
        classify_tmux_preflight(&program, farhelm_supervisor::tmux::probe_tmux(&program))?;
    if let Some(refusal) = refusal {
        let probed = tmux_probe_targets(ambient_tmux, prefixes);
        let platform = if cfg!(target_os = "macos") {
            TmuxRefusalPlatform::Mac
        } else {
            TmuxRefusalPlatform::Linux
        };
        // ADVISORY resolution purely for the message (see
        // `program_display_path`'s own doc comment): a below-floor refusal
        // that printed the bare spelling ("found tmux 3.6 at tmux") would
        // not tell a reader with several tmux installs which one answered.
        // The probe above already ran the ORIGINAL spelling, so this
        // read-only lookup adds no time-of-check/time-of-use gap in what
        // actually executed.
        let display_program = farhelm_supervisor::tmux::program_display_path(
            &program,
            std::env::var_os("PATH").as_deref(),
        );
        let override_in_force = ambient_tmux.filter(|value| !value.is_empty()).is_some();
        let message = tmux_refusal_message(
            refusal,
            &display_program,
            platform,
            &probed,
            override_in_force,
        );
        refuse_and_exit(&message);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `FARHELM_TMUX` already present in the app's own environment is the
    /// user's explicit override and must win outright — probing must not
    /// even run. Asserted here by making the
    /// injected predicate return `true` for everything: if the override were
    /// ignored, the probe would "find" a match and this test could not tell
    /// the two cases apart, so a passing predicate is what makes this a real
    /// test of precedence rather than of the override merely being present.
    #[farhelm_testtrace::test]
    fn ambient_tmux_override_wins_without_probing() {
        let resolved = resolve_supervisor_tmux(
            Some(std::ffi::OsString::from("/wherever/the/user/said/tmux")),
            &["/opt/homebrew/bin"],
            |candidate| {
                panic!("the probe must not run under an override; asked about {candidate:?}")
            },
        );
        assert_eq!(
            resolved,
            Some(std::ffi::OsString::from("/wherever/the/user/said/tmux"))
        );
    }

    /// Absent an override, the first prefix the predicate accepts is what the
    /// supervisor is launched with.
    #[farhelm_testtrace::test]
    fn probe_result_is_used_when_no_override_is_set() {
        let resolved = resolve_supervisor_tmux(
            None,
            &["/opt/homebrew/bin", "/usr/local/bin"],
            |candidate| candidate == Path::new("/usr/local/bin/tmux"),
        );
        assert_eq!(
            resolved,
            Some(std::ffi::OsString::from("/usr/local/bin/tmux"))
        );
    }

    /// No override and no prefix match (or, equivalently, an empty prefix
    /// list — the shape the non-macOS call site always passes) must yield
    /// `None`, leaving PATH resolution to the supervisor exactly as before
    /// this probe existed.
    #[farhelm_testtrace::test]
    fn no_override_and_no_probe_hit_falls_back_to_nothing() {
        assert_eq!(
            resolve_supervisor_tmux(None, &["/opt/homebrew/bin"], |_| false),
            None
        );
        assert_eq!(resolve_supervisor_tmux(None, &[], |_| true), None);
    }

    /// `FARHELM_TMUX=` (present but empty) is how a unit file or profile
    /// spells "no override", and the supervisor reads it that way too; the
    /// app must then probe rather than pass the empty value through, which
    /// would disable the discovery Finder launches depend on.
    #[farhelm_testtrace::test]
    fn an_empty_ambient_override_counts_as_unset_and_probes() {
        let resolved = resolve_supervisor_tmux(
            Some(std::ffi::OsString::new()),
            &["/opt/homebrew/bin"],
            |candidate| candidate == Path::new("/opt/homebrew/bin/tmux"),
        );
        assert_eq!(
            resolved,
            Some(std::ffi::OsString::from("/opt/homebrew/bin/tmux"))
        );
    }

    // ---- the tmux preflight (missing/below-floor startup refusal) ----
    //
    // `classify_tmux_preflight` takes an already-completed `probe_tmux`
    // result rather than a program to run, so these construct a canned
    // `TmuxProbe`/`TmuxProbeError` directly instead of spawning a real
    // tmux — the fixtures deliberately avoid depending on the host's own
    // tmux installation, or lack of one. `tmux_refusal_message` is pure on
    // top of that classification, so both platforms' wording get exercised
    // by plain `cargo test` here even though only one of them is ever seen
    // on a real Mac.

    /// Build a passing [`farhelm_supervisor::tmux::TmuxProbe`] for `stdout`,
    /// classifying it the same way `probe_tmux` itself would.
    fn probe_ok(program: &str, stdout: &str) -> farhelm_supervisor::tmux::TmuxProbe {
        let version = farhelm_supervisor::tmux::parse_tmux_version(stdout)
            .expect("fixture version must parse");
        farhelm_supervisor::tmux::TmuxProbe {
            program: PathBuf::from(program),
            version,
            support: farhelm_supervisor::tmux::classify_tmux_version(version),
        }
    }

    /// `TmuxProbeError::NotRunnable` carrying `ENOENT` — the shape
    /// `std::process::Command` produces when the program cannot be found on
    /// disk or on `PATH` — must classify as [`TmuxRefusal::NotFound`], not
    /// surface as a generic error.
    #[farhelm_testtrace::test]
    fn a_not_found_probe_classifies_as_not_found() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::NotRunnable(
            std::io::Error::from(std::io::ErrorKind::NotFound),
        ));
        let refusal = classify_tmux_preflight(Path::new("/nonexistent/tmux"), probed)
            .expect("ENOENT is a classified refusal, not an error");
        assert_eq!(refusal, Some(TmuxRefusal::NotFound));
    }

    /// A `-V` answer below `TMUX_FLOOR` must classify as `BelowFloor`,
    /// carrying the exact version found so the message can name it.
    #[farhelm_testtrace::test]
    fn a_below_floor_version_classifies_with_the_found_version() {
        let refusal =
            classify_tmux_preflight(Path::new("tmux"), Ok(probe_ok("tmux", "tmux 3.4\n")))
                .expect("a well-formed below-floor version classifies cleanly");
        assert_eq!(
            refusal,
            Some(TmuxRefusal::BelowFloor {
                found: farhelm_supervisor::tmux::parse_tmux_version("tmux 3.4\n").unwrap()
            })
        );
    }

    /// At or above the floor must let startup proceed — `None`, not a
    /// refusal — which is what lets an above-pin tmux through silently here
    /// (its "unaudited" warning is the supervisor's job once it actually
    /// starts, not this preflight's).
    #[farhelm_testtrace::test]
    fn at_or_above_the_floor_is_not_a_refusal() {
        assert_eq!(
            classify_tmux_preflight(Path::new("tmux"), Ok(probe_ok("tmux", "tmux 3.7c\n")))
                .unwrap(),
            None
        );
        assert_eq!(
            classify_tmux_preflight(Path::new("tmux"), Ok(probe_ok("tmux", "tmux 3.9\n"))).unwrap(),
            None
        );
    }

    /// A spawn failure that is NOT `ENOENT` (a permission-denied `--tmux`,
    /// say) must reach the ordinary `anyhow::Error` path rather than being
    /// folded into [`TmuxRefusal::NotFound`] — a bare `EACCES` describes a
    /// wrong permission bit, not an absent tmux, and the two need different
    /// operator repairs. The rendered chain must still name the program so
    /// the failure is actionable.
    #[farhelm_testtrace::test]
    fn a_permission_denied_probe_is_an_ordinary_error_naming_the_program() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::NotRunnable(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        ));
        let error = classify_tmux_preflight(Path::new("/opt/locked/tmux"), probed)
            .expect_err("permission-denied is not a classified refusal");
        let message = format!("{error:#}");
        assert!(message.contains("/opt/locked/tmux"), "{message}");
    }

    /// A nonzero `-V` exit surfaces as [`TmuxProbeError::Unparseable`]
    /// carrying tmux's own stderr (see `probe_tmux`'s doc comment) — the
    /// rendered chain must keep both the program and that stderr text,
    /// since stderr is the only actionable detail a caller ever gets for
    /// "something answered to this name and refused".
    #[farhelm_testtrace::test]
    fn a_nonzero_probe_exit_names_the_program_and_keeps_its_stderr() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::Unparseable(
            "libevent.so.2: cannot open shared object file".to_string(),
        ));
        let error = classify_tmux_preflight(Path::new("/opt/broken/tmux"), probed)
            .expect_err("a nonzero exit cannot be treated as a version");
        let message = format!("{error:#}");
        assert!(message.contains("/opt/broken/tmux"), "{message}");
        assert!(
            message.contains("libevent.so.2: cannot open shared object file"),
            "{message}"
        );
    }

    /// Successful but unparseable `-V` output is the SAME `Unparseable`
    /// variant as a nonzero exit (both come from `probe_tmux` unable to
    /// produce a usable version), so the rendered chain must still carry
    /// the malformed text even though nothing failed at the process level.
    #[farhelm_testtrace::test]
    fn malformed_successful_output_names_the_program_and_keeps_the_malformed_text() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::Unparseable(
            "tmux 9.9zzz-vendor-mangled".to_string(),
        ));
        let error = classify_tmux_preflight(Path::new("/opt/weird/tmux"), probed)
            .expect_err("an unreadable version must refuse");
        let message = format!("{error:#}");
        assert!(message.contains("/opt/weird/tmux"), "{message}");
        assert!(message.contains("tmux 9.9zzz-vendor-mangled"), "{message}");
    }

    /// `TmuxProbeError::Overran` (the bounded probe's time/output-budget
    /// refusal — see `probe_tmux`'s own doc comment) must reach the
    /// ordinary `anyhow::Error` path, exactly like `Unparseable`, rather
    /// than being folded into `Ok(None)` or misread as the specialized
    /// `TmuxRefusal::NotFound`. Those two outcomes look similar only in
    /// that both let a caller move on, but they mean opposite things here:
    /// `Ok(None)` says "this tmux is fine, proceed", while an overrun
    /// candidate answered nothing conclusive at all. Confusing them would
    /// let the desktop hand its managed supervisor a candidate the
    /// preflight never actually cleared, so the supervisor's own
    /// unbounded `-V` call could then hang or flood output with no
    /// preflight left to have caught it first.
    #[farhelm_testtrace::test]
    fn an_overrun_probe_is_an_ordinary_error_naming_the_program_and_detail() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::Overran(
            "it did not answer -V within 5 seconds".to_string(),
        ));
        let error = classify_tmux_preflight(Path::new("/opt/wedged/tmux"), probed)
            .expect_err("an overrun probe cannot be treated as a version");
        let message = format!("{error:#}");
        assert!(message.contains("/opt/wedged/tmux"), "{message}");
        assert!(
            message.contains("it did not answer -V within 5 seconds"),
            "{message}"
        );
    }

    /// The four refusal×platform combinations, pinned as COMPLETE strings
    /// rather than substring checks — a message that silently dropped the
    /// floor version, reworded the install command, added a clause, or let
    /// an `Error:`/backtrace-shaped line back in would be exactly the
    /// regression this preflight exists to prevent, and a looser assertion
    /// could miss any of those.
    #[farhelm_testtrace::test]
    fn refusal_messages_are_pinned_exactly_per_platform() {
        use farhelm_supervisor::tmux::TMUX_FLOOR;

        let probed = vec!["FARHELM_TMUX".to_string(), "PATH".to_string()];
        let below = TmuxRefusal::BelowFloor {
            found: farhelm_supervisor::tmux::parse_tmux_version("tmux 3.4\n").unwrap(),
        };
        let program = Path::new("/usr/bin/tmux");

        let not_found_subject = format!(
            "farhelm-desktop needs tmux {TMUX_FLOOR} or newer, and none could be run (looked \
             at: FARHELM_TMUX, then PATH). Each one was either not found, or is missing its \
             interpreter or loader."
        );
        let below_floor_subject = format!(
            "found tmux 3.4 at /usr/bin/tmux, which is below the {TMUX_FLOOR} farhelm needs."
        );
        let mac_body = "On macOS, tmux has to be installed by hand; Homebrew is the recommended \
             way:\n\n    brew install tmux\n\nThen start farhelm-desktop again.";
        let linux_body = format!(
            "Install tmux {TMUX_FLOOR} or newer with your package manager or Linuxbrew (`brew \
             install tmux`), or point FARHELM_TMUX at one.\nThen start farhelm-desktop again."
        );

        let cases = [
            (
                TmuxRefusal::NotFound,
                TmuxRefusalPlatform::Mac,
                format!("{not_found_subject}\n{mac_body}"),
            ),
            (
                TmuxRefusal::NotFound,
                TmuxRefusalPlatform::Linux,
                format!("{not_found_subject}\n{linux_body}"),
            ),
            (
                below,
                TmuxRefusalPlatform::Mac,
                format!("{below_floor_subject}\n{mac_body}"),
            ),
            (
                below,
                TmuxRefusalPlatform::Linux,
                format!("{below_floor_subject}\n{linux_body}"),
            ),
        ];
        for (refusal, platform, expected) in cases {
            let message = tmux_refusal_message(refusal, program, platform, &probed, false);
            assert_eq!(message, expected);
        }
    }

    /// An ambient `FARHELM_TMUX` in force changes the REMEDY, not the
    /// diagnosis: restarting with the same bad override reproduces the
    /// identical refusal without ever considering a freshly installed
    /// tmux, so both platforms' bodies must name `FARHELM_TMUX` as the
    /// thing to fix. Pinned exactly, and for both platforms, since this is
    /// the one clause that only appears when an override is in force.
    #[farhelm_testtrace::test]
    fn an_override_in_force_is_named_as_the_remedy_on_both_platforms() {
        use farhelm_supervisor::tmux::TMUX_FLOOR;

        let probed = vec!["/custom/tmux".to_string()];
        let mac = tmux_refusal_message(
            TmuxRefusal::NotFound,
            Path::new("/custom/tmux"),
            TmuxRefusalPlatform::Mac,
            &probed,
            true,
        );
        assert_eq!(
            mac,
            format!(
                "farhelm-desktop needs tmux {TMUX_FLOOR} or newer, and none could be run \
                 (looked at: /custom/tmux). Each one was either not found, or is missing its \
                 interpreter or loader.\nOn macOS, tmux has to be installed by hand; Homebrew is \
                 the recommended way:\n\n    brew install tmux\n\nFARHELM_TMUX is set and \
                 overrides that search, so update it to point at the new install (or unset it) \
                 before starting farhelm-desktop again."
            )
        );

        let linux = tmux_refusal_message(
            TmuxRefusal::NotFound,
            Path::new("/custom/tmux"),
            TmuxRefusalPlatform::Linux,
            &probed,
            true,
        );
        assert_eq!(
            linux,
            format!(
                "farhelm-desktop needs tmux {TMUX_FLOOR} or newer, and none could be run \
                 (looked at: /custom/tmux). Each one was either not found, or is missing its \
                 interpreter or loader.\nInstall tmux {TMUX_FLOOR} or newer with your package \
                 manager or Linuxbrew (`brew install tmux`). FARHELM_TMUX is set and overrides \
                 where farhelm-desktop looks, so update it to point at a supported tmux, or \
                 unset it, before starting farhelm-desktop again."
            )
        );
    }

    /// `NotFound`'s "looked at: ..." clause must actually name what
    /// `tmux_probe_targets` reports, in the SAME order — a message that
    /// invented its own list, or reordered the real one, would mislead
    /// exactly the operator trying to act on it.
    #[farhelm_testtrace::test]
    fn the_not_found_message_names_every_probed_location_in_order() {
        let probed = tmux_probe_targets(
            None,
            &["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"],
        );
        let message = tmux_refusal_message(
            TmuxRefusal::NotFound,
            Path::new("tmux"),
            TmuxRefusalPlatform::Mac,
            &probed,
            false,
        );
        assert!(message.contains(
            "looked at: FARHELM_TMUX, /opt/homebrew/bin, /usr/local/bin, MacPorts, then PATH"
        ));
    }

    /// An ambient `FARHELM_TMUX` that failed must be named ALONE — probing
    /// never ran (see `resolve_supervisor_tmux`'s own precedence), so a
    /// message claiming to have also checked Homebrew or PATH would be
    /// describing a search this run never performed.
    ///
    /// The fake prefix list is FIXED and non-empty rather than
    /// `macos_tmux_prefixes()`, which is an empty slice outside macOS: an
    /// implementation that wrongly appended the real prefixes after an
    /// explicit override would still pass against an empty list, because
    /// there would be nothing for it to wrongly append. A fixed, populated
    /// list exercises the precedence regardless of which host runs the
    /// test.
    #[farhelm_testtrace::test]
    fn an_ambient_override_is_probed_alone() {
        assert_eq!(
            tmux_probe_targets(
                Some(std::ffi::OsStr::new("/nonexistent/tmux")),
                &["/fake/homebrew/bin", "/fake/local/bin", "/opt/local/bin"]
            ),
            vec!["/nonexistent/tmux".to_string()]
        );
    }

    /// `FARHELM_TMUX=` (present but empty) must report the SAME probe
    /// targets as no override at all — `resolve_supervisor_tmux` already
    /// treats the two as equivalent (see
    /// `an_empty_ambient_override_counts_as_unset_and_probes`), and this
    /// pins that `tmux_probe_targets` cannot drift from that decision by
    /// treating an empty value as a real override with nothing to report.
    #[farhelm_testtrace::test]
    fn an_empty_override_reports_the_same_targets_as_no_override() {
        let prefixes = ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"];
        assert_eq!(
            tmux_probe_targets(Some(std::ffi::OsStr::new("")), &prefixes),
            tmux_probe_targets(None, &prefixes)
        );
    }
}
