//! The systemd user units Farhelm writes, and the one renderer that fills
//! them in.
//!
//! Two callers share this module and they must not drift: remote
//! provisioning, which renders a supervisor unit and pushes it to a host
//! over SSH, and `farhelm helm setup`, which renders the same unit plus a
//! helm unit into the local user's `systemd/user` directory. The templates
//! under `crates/farhelm-helm/units/` are the canonical policy — lifecycle
//! behaviour lives in one reviewed file rather than in two code paths that
//! happen to agree today.
//!
//! Ownership is the other half of what lives here. Setup marks every file
//! it writes with [`MANAGED_MARKER`] as the very first line and will only
//! ever overwrite or delete a file carrying that marker, so a unit written
//! by hand — or by a distribution package — is never silently replaced.
//! Provisioning deliberately does NOT mark the units it installs on remote
//! hosts: those files are owned by the provisioning workflow, not by
//! setup, and marking them would invite setup on a host machine to adopt
//! and rewrite them.
//!
//! Everything here is pure: no environment reads, no filesystem access,
//! no process spawning. Callers pass the paths they resolved.

use anyhow::bail;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The first line of every unit file `farhelm helm setup` writes, and the
/// entire basis of its ownership rule.
///
/// Setup refuses to touch a unit file that does not start with exactly
/// this line, so the marker is a compatibility surface: changing it
/// orphans every unit already installed, which then has to be moved aside
/// by hand before setup will run again.
pub const MANAGED_MARKER: &str = "# managed-by: farhelm helm setup";

/// The supervisor unit `farhelm helm setup` installs, and the ONE unit the
/// hosts panel's local-row protection rule looks for: the panel's question
/// is only ever "is a supervisor on this machine already somebody's?".
pub const SUPERVISOR_UNIT_NAME: &str = "farhelm-supervisor.service";

/// The helm unit `farhelm helm setup` installs. Setup owns it like the
/// supervisor unit, but no provisioning path ever reads it — the panel has
/// no opinion about helms.
pub const HELM_UNIT_NAME: &str = "farhelm-helm.service";

/// The supervisor unit as reviewed, with `@…@` placeholders for the three
/// paths [`SupervisorUnitInputs`] supplies plus the `@PATH@` this module
/// derives from two of them. Shared with remote provisioning.
const SUPERVISOR_UNIT_TEMPLATE: &str = include_str!("../units/farhelm-supervisor.service.in");

/// The helm unit as reviewed. `@HELM_ARGS@` is what makes it a template
/// rather than the static file it used to be: the flags a helm needs
/// depend on how setup was invoked.
const HELM_UNIT_TEMPLATE: &str = include_str!("../units/farhelm-helm.service.in");

/// The tail of the SUPERVISOR unit's search path, after the directories
/// its own two executables live in. The helm unit sets no `PATH` at all —
/// it shells out to nothing.
const BASE_PATH: [&str; 3] = ["/usr/local/bin", "/usr/bin", "/bin"];

/// The paths the supervisor unit pins.
///
/// `tmux` is the executable, not a directory, and that is load-bearing:
/// it is pinned into the unit as `FARHELM_TMUX` so the supervisor drives
/// the exact binary whoever rendered this unit approved, rather than
/// whatever `PATH` resolves at start time.
pub struct SupervisorUnitInputs<'a> {
    pub farhelm: &'a Path,
    pub state_dir: &'a Path,
    pub tmux: &'a Path,
}

/// The paths and flags the helm unit pins.
///
/// `state_dir` is REQUIRED, and deliberately so. Leaving it out would let
/// the helm resolve its own default at start time, from the environment
/// the systemd user manager was started with — which is not the
/// environment setup ran in. A one-shot `XDG_STATE_HOME=/srv/state farhelm
/// helm setup` would then pin `/srv/state/farhelm` into the supervisor
/// unit (whose template has no way to omit it) while the helm kept using
/// `~/.local/state/farhelm`, and the two would look for each other's
/// socket in different trees. One resolved directory, written into both
/// units, is what makes that impossible.
///
/// `port` stays optional: nothing else derives a path from it, so an
/// omitted `--port` can safely mean "whatever the helm defaults to".
pub struct HelmUnitInputs<'a> {
    pub farhelm: &'a Path,
    pub state_dir: &'a Path,
    pub port: Option<u16>,
}

/// Render the supervisor unit. The result is UNMARKED; setup wraps it in
/// [`managed`], provisioning does not.
///
/// `PATH` is composed here, from the directories holding the two
/// executables the unit names plus [`BASE_PATH`]. Farhelm's own directory
/// comes first so a host where Farhelm supplied tmux finds that payload,
/// and tmux's directory joins for the separate reason that a systemd user
/// manager process does not necessarily inherit the login shell's `PATH` —
/// anything the supervisor or a session shells out to should still find
/// tmux by name. A program with no directory part (the `<tmux>` stand-in
/// `farhelm helm setup --dry-run` renders when it could not find a real
/// one) contributes no `PATH` entry rather than an empty one, which POSIX
/// would read as the current directory.
///
/// `KillMode=process` in the template is equally deliberate: tmux owns the
/// durable sessions, so restarting their manager must stop only the
/// supervisor process rather than systemd's default whole control group.
///
/// Fails when any path cannot cross a text boundary faithfully (see
/// [`path_text`]) or when a `PATH` component contains `:`, which systemd's
/// `Environment=` grammar cannot represent.
pub fn render_supervisor_unit(inputs: &SupervisorUnitInputs) -> anyhow::Result<String> {
    let mut search: Vec<&Path> = Vec::new();
    for program in [inputs.farhelm, inputs.tmux] {
        if let Some(dir) = program.parent().filter(|dir| !dir.as_os_str().is_empty())
            && !search.contains(&dir)
        {
            search.push(dir);
        }
    }
    let mut components = Vec::with_capacity(search.len() + BASE_PATH.len());
    for dir in search {
        let text = path_text(dir)?;
        if text.contains(':') {
            bail!(
                "PATH component {text:?} contains ':', which systemd's Environment= grammar \
                 cannot represent faithfully"
            );
        }
        components.push(text);
    }
    components.extend(BASE_PATH.iter().map(|entry| entry.to_string()));
    Ok(render_template(
        SUPERVISOR_UNIT_TEMPLATE,
        &[
            ("@FARHELM@", &systemd_arg(inputs.farhelm)?),
            ("@STATE_DIR@", &systemd_arg(inputs.state_dir)?),
            ("@PATH@", &environment_value(&components.join(":"))),
            ("@TMUX@", &environment_value(&path_text(inputs.tmux)?)),
        ],
    ))
}

/// Render the helm unit. The result is UNMARKED; setup wraps it in
/// [`managed`].
///
/// The state directory is always written out (see [`HelmUnitInputs`] for
/// why an omitted one would be a bug rather than a default), `--port`
/// only when pinned, and the two always appear in the same order, so an
/// unchanged configuration renders byte-identically and setup's
/// write-if-changed check stays meaningful.
pub fn render_helm_unit(inputs: &HelmUnitInputs) -> anyhow::Result<String> {
    let mut args = String::new();
    args.push_str(" --state-dir ");
    args.push_str(&systemd_arg(inputs.state_dir)?);
    if let Some(port) = inputs.port {
        args.push_str(&format!(" --port {port}"));
    }
    Ok(render_template(
        HELM_UNIT_TEMPLATE,
        &[
            ("@FARHELM@", &systemd_arg(inputs.farhelm)?),
            ("@HELM_ARGS@", &args),
        ],
    ))
}

/// Stamp a rendered unit as owned by `farhelm helm setup`.
pub fn managed(unit: String) -> String {
    format!("{MANAGED_MARKER}\n{unit}")
}

/// Whether setup may overwrite or delete this unit file.
///
/// Exact first-line equality, on purpose. A file whose marker line picked
/// up trailing whitespace, a `\r`, or a second comment above it is a file
/// somebody edited, and the safe answer for an ownership check is "not
/// mine" — the cost is a refusal the operator can resolve by hand, while
/// the cost of guessing generously is clobbering somebody's unit.
pub fn is_managed(unit_text: &str) -> bool {
    unit_text.split('\n').next() == Some(MANAGED_MARKER)
}

/// The program the unit's EFFECTIVE `[Service] ExecStart=` runs, or `None`
/// when the file names none this parser is willing to read.
///
/// Used to answer one question: does this unit run THIS farhelm? The
/// caller compares the result against its own executable, so an
/// unrecognized spelling yields `None` — and the caller must treat that as
/// "cannot tell", never as "not ours". Getting this wrong in the generous
/// direction means the hosts panel overwriting a unit somebody wrote by
/// hand, which is exactly the outcome the ownership rule exists to prevent.
///
/// Three pieces of systemd's assignment semantics matter here and are
/// implemented:
///
/// - Section scoping. Only `[Service]` counts. `ExecStart=` under
///   `[Unit]`, `[Install]`, or a `[X-…]` section is not a command.
/// - LAST assignment wins. Systemd overwrites a non-list directive with
///   each later assignment in the same section (only an empty value has
///   list semantics), so an earlier line is not the effective one.
/// - An empty `ExecStart=` RESETS the list. A file that sets a command and
///   then clears it has no effective command, and reporting the cleared
///   one would name a binary that never runs.
///
/// What it does NOT do: expand systemd specifiers. A unit whose
/// `ExecStart=` begins with `%h/...` (as Farhelm's own pre-setup unit file
/// did) comes back with the `%h` still in it; the caller's canonicalisation
/// then fails to resolve it, which is the safe direction — see
/// `local_supervisor_is_not_ours`. Line continuations (`\` at end of line)
/// are not joined either; a continued first line still yields its program,
/// which is all this answers.
pub fn exec_start_program(unit_text: &str) -> Option<PathBuf> {
    let mut in_service = false;
    let mut effective: Option<Option<PathBuf>> = None;
    for line in unit_text.lines() {
        let line = line.trim();
        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            in_service = section.eq_ignore_ascii_case("Service");
            continue;
        }
        if !in_service {
            continue;
        }
        let Some(value) = line.strip_prefix("ExecStart=").map(str::trim) else {
            continue;
        };
        if value.is_empty() {
            // The reset spelling: everything assigned so far is discarded.
            effective = Some(None);
            continue;
        }
        effective = Some(exec_start_value_program(value));
    }
    effective.flatten()
}

/// The program named by one `ExecStart=` value.
fn exec_start_value_program(value: &str) -> Option<PathBuf> {
    // Systemd allows a set of modifier characters ahead of the program:
    // `@` supplies argv[0] separately, `-` tolerates failure, `:` disables
    // variable expansion for the arguments, and `+`/`!`/`!!` change
    // privileges. They say nothing about WHICH binary runs, so they are
    // skipped rather than parsed. `:` was missing here once — a valid
    // spelling that made a matching unit read as unclassifiable.
    let value = value.trim_start_matches(['@', '-', ':', '+', '!']).trim();
    let program = first_systemd_token(value)?;
    (!program.is_empty()).then(|| PathBuf::from(program))
}

/// Read one systemd command-line token, undoing the quoting
/// [`systemd_arg`] applies.
///
/// Deliberately narrow: double and single quotes, backslash escapes, `%%`,
/// and `$$`. Systemd's full C-style escape set (`\n`, `\x41`, …) is not
/// handled, because a unit using it against a path is not a unit this
/// project wrote and the honest answer for the ownership check above is
/// "unrecognized".
///
/// `$$` is decoded because [`systemd_arg`] emits it: without that, reading
/// back our own render of a path containing a dollar would report a
/// doubled one and fail to match the binary it names.
fn first_systemd_token(value: &str) -> Option<String> {
    let mut chars = value.chars().peekable();
    let mut token = String::new();
    let quote = match chars.peek() {
        Some('"') => Some('"'),
        Some('\'') => Some('\''),
        _ => None,
    };
    if quote.is_some() {
        chars.next();
    }
    let mut closed = quote.is_none();
    while let Some(character) = chars.next() {
        match character {
            '\\' => token.push(chars.next()?),
            '%' if chars.peek() == Some(&'%') => {
                chars.next();
                token.push('%');
            }
            '$' if chars.peek() == Some(&'$') => {
                chars.next();
                token.push('$');
            }
            character if Some(character) == quote => {
                closed = true;
                break;
            }
            character if quote.is_none() && character.is_whitespace() => break,
            character => token.push(character),
        }
    }
    closed.then_some(token)
}

/// The directory the calling user's systemd manager searches for unit
/// files: `${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user`.
///
/// This is the same rule provisioning's remote reach check derives on the
/// host, and the two must agree — a unit written to the wrong one of these
/// looks perfectly valid and is never loaded.
///
/// A relative `XDG_CONFIG_HOME` is ignored rather than joined, per the XDG
/// base directory spec, which is also what provisioning's reach check
/// treats as unsupported. An empty value counts as unset, matching the
/// shell's `${VAR:-default}`.
pub fn user_unit_dir(xdg_config_home: Option<&OsStr>, home: &Path) -> PathBuf {
    user_unit_dir_for(xdg_config_home, Some(home))
        .expect("a known home directory always yields a unit directory")
}

/// [`user_unit_dir`] for a caller that may not know `HOME`.
///
/// An absolute `XDG_CONFIG_HOME` answers on its own — it names the
/// directory outright, and systemd's user manager reads it the same way
/// whether or not `HOME` happens to be set. Only when neither value is
/// usable is the directory genuinely indeterminate, and then this fails
/// rather than guessing: the caller that reads unit files for an OWNERSHIP
/// decision must not mistake "I could not tell where to look" for "there
/// is no unit there".
pub fn user_unit_dir_for(
    xdg_config_home: Option<&OsStr>,
    home: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    let config = match xdg_config_home
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        Some(xdg) => xdg,
        None => match home {
            Some(home) => home.join(".config"),
            None => bail!(
                "neither an absolute XDG_CONFIG_HOME nor HOME is set, so the systemd user unit \
                 directory cannot be located"
            ),
        },
    };
    Ok(config.join("systemd").join("user"))
}

/// Quote one path for systemd's `ExecStart=` grammar.
///
/// Four characters mean something to systemd inside a command line and all
/// four have to be doubled or escaped, or the unit names something other
/// than the path the caller chose:
///
/// - `\` and `"` are unescaped inside the quoted form.
/// - `%` introduces a unit specifier anywhere in the line.
/// - `$` introduces variable expansion: `${NAME}` expands inside an
///   argument and `$NAME` expands to a WHOLE-WORD split, so an unescaped
///   one can silently select a different executable, redirect the state
///   directory, or turn one argument into several. A literal dollar is
///   spelled `$$`.
pub(crate) fn systemd_arg(path: &Path) -> anyhow::Result<String> {
    Ok(format!(
        "\"{}\"",
        path_text(path)?
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    ))
}

/// Escape one value for the body of a systemd `Environment="…"` line.
///
/// Shared by `PATH` and `FARHELM_TMUX` so the two cannot drift; the
/// escaping rules are [`systemd_arg`]'s, minus the surrounding quotes the
/// template already supplies — and minus the dollar.
///
/// That asymmetry is deliberate and is systemd's, not ours: `Environment=`
/// assignments are NOT variable-expanded (only `EnvironmentFile=` and the
/// command lines are), so `$$` in one of these would reach the service as
/// two literal dollars. `%` still expands here, which is why it stays.
fn environment_value(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
}

/// Preserve a path exactly at every text-only unit, SSH, and registry
/// boundary. Rejecting a path that cannot survive the trip is safer than
/// displaying one path and later acting on a lossy approximation of it.
///
/// This is the single definition of that rule; provisioning's own
/// `path_text` wraps it to keep its failure type.
pub(crate) fn path_text(path: &Path) -> anyhow::Result<String> {
    let Some(text) = path.to_str() else {
        bail!(
            "path {} is not valid UTF-8, and systemd units and ssh command lines carry paths as \
             text",
            path.to_string_lossy()
        );
    };
    if text.chars().any(char::is_control) {
        bail!("path {text:?} contains a control character");
    }
    Ok(text.to_string())
}

/// Substitute template fields without rescanning inserted text as
/// template syntax.
///
/// A path may legally contain the string `@STATE_DIR@`. Walking the
/// template once and appending each replacement, instead of chaining
/// `str::replace`, keeps inserted text literal.
fn render_template(template: &str, values: &[(&str, &str)]) -> String {
    let mut rendered = String::with_capacity(template.len() + 128);
    let mut rest = template;
    while let Some((offset, token, value)) = values
        .iter()
        .filter_map(|(token, value)| rest.find(token).map(|offset| (offset, *token, *value)))
        .min_by_key(|(offset, _, _)| *offset)
    {
        rendered.push_str(&rest[..offset]);
        rendered.push_str(value);
        rest = &rest[offset + token.len()..];
    }
    rendered.push_str(rest);
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The complete text of both units, pinned.
    ///
    /// These templates ARE the lifecycle policy — `KillMode=process` is
    /// what keeps a supervisor restart from killing the tmux sessions it
    /// manages, `WantedBy=default.target` is what makes either unit start
    /// at all — and a substring test would stay green while any of those
    /// directives silently disappeared. The expectations are written out
    /// by hand rather than derived from the templates so that a mistake
    /// in the renderer cannot update the implementation and the
    /// expectation together.
    ///
    /// Both renders are UNMARKED: [`managed`] is setup's own step, and
    /// provisioning must never emit the marker. That property is asserted
    /// separately, below.
    #[test]
    fn both_units_render_to_their_exact_reviewed_text() {
        let supervisor = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/home/u/.local/bin/farhelm"),
            state_dir: Path::new("/home/u/.local/state/farhelm"),
            tmux: Path::new("/home/linuxbrew/.linuxbrew/bin/tmux"),
        })
        .unwrap();
        assert_eq!(
            supervisor,
            concat!(
                "[Unit]\n",
                "Description=Farhelm supervisor\n",
                "\n",
                "[Service]\n",
                "Type=simple\n",
                "ExecStart=\"/home/u/.local/bin/farhelm\" supervisor run --state-dir ",
                "\"/home/u/.local/state/farhelm\"\n",
                "Environment=\"PATH=/home/u/.local/bin:/home/linuxbrew/.linuxbrew/bin:",
                "/usr/local/bin:/usr/bin:/bin\"\n",
                "Environment=\"FARHELM_TMUX=/home/linuxbrew/.linuxbrew/bin/tmux\"\n",
                "KillMode=process\n",
                "Restart=on-failure\n",
                "\n",
                "[Install]\n",
                "WantedBy=default.target\n",
            )
        );

        let helm = render_helm_unit(&HelmUnitInputs {
            farhelm: Path::new("/home/u/.local/bin/farhelm"),
            state_dir: Path::new("/home/u/.local/state/farhelm"),
            port: None,
        })
        .unwrap();
        assert_eq!(
            helm,
            concat!(
                "[Unit]\n",
                "Description=Farhelm helm\n",
                "\n",
                "[Service]\n",
                "ExecStart=\"/home/u/.local/bin/farhelm\" helm run --state-dir ",
                "\"/home/u/.local/state/farhelm\"\n",
                "Restart=on-failure\n",
                "\n",
                "[Install]\n",
                "WantedBy=default.target\n",
            )
        );

        assert!(!is_managed(&supervisor));
        assert!(!is_managed(&helm));
        assert!(is_managed(&managed(supervisor)));
    }

    /// The helm unit's flags are the whole reason it became a template.
    /// The state directory is always there — an omitted one would let the
    /// helm resolve its own from the user manager's environment and drift
    /// away from the supervisor's — while `--port` appears only when
    /// pinned. Order is fixed so that an unchanged configuration
    /// re-renders byte-identically and setup keeps reporting `unchanged`.
    #[test]
    fn the_helm_unit_always_pins_state_and_only_pins_a_given_port() {
        let both = render_helm_unit(&HelmUnitInputs {
            farhelm: Path::new("/bin/farhelm"),
            state_dir: Path::new("/state"),
            port: Some(7433),
        })
        .unwrap();
        assert!(
            both.contains(
                "ExecStart=\"/bin/farhelm\" helm run --state-dir \"/state\" --port 7433\n"
            )
        );
        let no_port = render_helm_unit(&HelmUnitInputs {
            farhelm: Path::new("/bin/farhelm"),
            state_dir: Path::new("/state"),
            port: None,
        })
        .unwrap();
        assert!(no_port.contains("ExecStart=\"/bin/farhelm\" helm run --state-dir \"/state\"\n"));
    }

    /// Every character systemd reads as syntax inside a command line has
    /// to survive rendering as a literal, and the DOLLAR is the dangerous
    /// one: `${NAME}` expands inside an argument and `$NAME` expands to a
    /// whole-word split, so an unescaped one turns a valid directory into
    /// a different executable, a different state tree, or several
    /// arguments. Read-back matters as much as rendering — the ownership
    /// check compares a parsed `ExecStart` against a real path, and a
    /// doubled dollar surviving that round trip would make a unit that IS
    /// ours look like somebody else's.
    #[test]
    fn dollars_are_escaped_in_commands_and_decoded_on_read_back() {
        for path in [
            "/home/u/$NAME/farhelm",
            "/home/u/${NAME}/farhelm",
            "/home/u/$/farhelm",
            "/home/u/$$literal/farhelm",
        ] {
            let unit = render_supervisor_unit(&SupervisorUnitInputs {
                farhelm: Path::new(path),
                state_dir: Path::new("/state/$HOME"),
                tmux: Path::new("/usr/bin/tmux"),
            })
            .unwrap();
            assert!(
                unit.contains(&format!(
                    "ExecStart=\"{}\" supervisor",
                    path.replace('$', "$$")
                )),
                "{unit}"
            );
            assert!(unit.contains("--state-dir \"/state/$$HOME\""), "{unit}");
            assert_eq!(exec_start_program(&unit), Some(PathBuf::from(path)));
        }

        // `Environment=` is NOT expanded by systemd, so a dollar stays
        // single there. Doubling it would hand the supervisor a literal
        // `$$` in its PATH.
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/opt/$NAME/farhelm"),
            state_dir: Path::new("/state"),
            tmux: Path::new("/opt/$NAME/tmux"),
        })
        .unwrap();
        assert!(
            unit.contains("Environment=\"PATH=/opt/$NAME:/usr/local/bin:/usr/bin:/bin\""),
            "{unit}"
        );
        assert!(
            unit.contains("Environment=\"FARHELM_TMUX=/opt/$NAME/tmux\""),
            "{unit}"
        );
    }

    /// `Environment=` has its own escape branch, so the characters that
    /// are only covered for `ExecStart` elsewhere need their own complete
    /// expectations here: a quote or backslash reaching systemd unescaped
    /// ends the assignment early and leaves the supervisor with a PATH or
    /// a pinned tmux that is not the one this render chose.
    #[test]
    fn environment_values_escape_quotes_and_backslashes() {
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/opt/a\"b/farhelm"),
            state_dir: Path::new("/state"),
            tmux: Path::new("/opt/c\\d/tmux"),
        })
        .unwrap();
        assert!(
            unit.contains(
                "Environment=\"PATH=/opt/a\\\"b:/opt/c\\\\d:/usr/local/bin:/usr/bin:/bin\"\n"
            ),
            "{unit}"
        );
        assert!(
            unit.contains("Environment=\"FARHELM_TMUX=/opt/c\\\\d/tmux\"\n"),
            "{unit}"
        );
    }

    /// A path may legally contain the very text the templates use as
    /// placeholders. The renderer walks the template once for that
    /// reason, and the invariant is worth a test because the obvious
    /// simplification — chained `str::replace` — would corrupt such a
    /// path while every other test stayed green.
    #[test]
    fn inserted_text_is_never_rescanned_as_template_syntax() {
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/opt/@STATE_DIR@/farhelm"),
            state_dir: Path::new("/opt/@TMUX@"),
            tmux: Path::new("/opt/@PATH@/tmux"),
        })
        .unwrap();
        assert!(
            unit.contains(
                "ExecStart=\"/opt/@STATE_DIR@/farhelm\" supervisor run --state-dir \
                 \"/opt/@TMUX@\"\n"
            ),
            "{unit}"
        );
        assert!(
            unit.contains("Environment=\"FARHELM_TMUX=/opt/@PATH@/tmux\"\n"),
            "{unit}"
        );
    }

    /// A path that cannot cross the unit file's text boundary must fail
    /// before anything is written, and a `PATH` component holding the
    /// separator systemd uses cannot be represented at all.
    #[test]
    fn unrepresentable_paths_are_refused_rather_than_approximated() {
        let percent = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/tmp/%h/farhelm"),
            state_dir: Path::new("/tmp/state"),
            tmux: Path::new("/tmp/%h/tmux"),
        })
        .unwrap();
        assert!(percent.contains("\"/tmp/%%h/farhelm\""));
        assert!(percent.contains("PATH=/tmp/%%h:"));
        assert!(percent.contains("Environment=\"FARHELM_TMUX=/tmp/%%h/tmux\""));

        assert!(
            render_supervisor_unit(&SupervisorUnitInputs {
                farhelm: Path::new("/tmp/farhelm"),
                state_dir: Path::new("/tmp/state\nother"),
                tmux: Path::new("/tmp/tmux"),
            })
            .is_err()
        );
        assert!(
            render_supervisor_unit(&SupervisorUnitInputs {
                farhelm: Path::new("/tmp/with:colon/farhelm"),
                state_dir: Path::new("/tmp/state"),
                tmux: Path::new("/tmp/tmux"),
            })
            .is_err()
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let non_utf8 = PathBuf::from(std::ffi::OsString::from_vec(vec![
                b'/', b't', b'm', b'p', b'/', 0xff,
            ]));
            assert!(
                render_supervisor_unit(&SupervisorUnitInputs {
                    farhelm: Path::new("/tmp/farhelm"),
                    state_dir: &non_utf8,
                    tmux: Path::new("/tmp/tmux"),
                })
                .is_err()
            );
            assert!(
                render_supervisor_unit(&SupervisorUnitInputs {
                    farhelm: Path::new("/tmp/farhelm"),
                    state_dir: Path::new("/tmp/state"),
                    tmux: &non_utf8,
                })
                .is_err()
            );
        }
    }

    /// Systemd argument escaping handles each special character in its own
    /// grammar rather than borrowing shell quoting rules. A `%h` that
    /// reached systemd unexpanded would name a different binary than the
    /// caller chose.
    #[test]
    fn systemd_argument_rendering_covers_every_supported_escape() {
        for (path, expected) in [
            ("/tmp/a b", "\"/tmp/a b\""),
            ("/tmp/a\"b", "\"/tmp/a\\\"b\""),
            ("/tmp/a\\b", "\"/tmp/a\\\\b\""),
            ("/tmp/%h", "\"/tmp/%%h\""),
        ] {
            assert_eq!(systemd_arg(Path::new(path)).unwrap(), expected);
        }
    }

    /// A program with no directory part must not contribute an empty
    /// `PATH` component: POSIX reads one as the current directory, which
    /// would let a stray `tmux` in whatever directory the user manager
    /// started in win.
    #[test]
    fn a_bare_program_name_contributes_no_path_entry() {
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/opt/farhelm"),
            state_dir: Path::new("/state"),
            tmux: Path::new("<tmux>"),
        })
        .unwrap();
        assert!(
            unit.contains("Environment=\"PATH=/opt:/usr/local/bin:/usr/bin:/bin\""),
            "{unit}"
        );
    }

    /// The ownership rule is exact-match by construction. These are the
    /// near misses that must NOT be adopted, because adopting one means
    /// overwriting a file somebody else wrote.
    #[test]
    fn only_an_exact_first_line_marker_counts_as_managed() {
        assert!(is_managed(&managed("[Unit]\n".to_string())));
        assert!(is_managed(MANAGED_MARKER));
        assert!(!is_managed(""));
        assert!(!is_managed("[Unit]\n# managed-by: farhelm helm setup\n"));
        assert!(!is_managed("# managed-by: farhelm helm setup \n"));
        assert!(!is_managed("# managed-by: farhelm helm setup\r\n"));
        assert!(!is_managed("#managed-by: farhelm helm setup\n"));
    }

    /// The local-row protection rule stands on this parse: a unit whose
    /// `ExecStart=` names the running helm's own binary is a unit the
    /// hosts panel must not touch. It has to survive our own quoting and
    /// reject spellings it cannot resolve.
    #[test]
    fn exec_start_program_reads_back_what_the_renderer_wrote() {
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/home/u/we ird/%farhelm\""),
            state_dir: Path::new("/state"),
            tmux: Path::new("/usr/bin/tmux"),
        })
        .unwrap();
        assert_eq!(
            exec_start_program(&unit),
            Some(PathBuf::from("/home/u/we ird/%farhelm\""))
        );
        assert_eq!(
            exec_start_program("[Service]\nExecStart=/usr/bin/farhelm supervisor run\n"),
            Some(PathBuf::from("/usr/bin/farhelm"))
        );
        assert_eq!(
            exec_start_program("[Service]\nExecStart=-@/usr/bin/farhelm argv0 run\n"),
            Some(PathBuf::from("/usr/bin/farhelm"))
        );
        // `:` (no variable expansion for the arguments) is as valid a
        // modifier as the rest. Missing it made a matching unit read as
        // unclassifiable, which fails closed but reports the wrong thing.
        assert_eq!(
            exec_start_program("[Service]\nExecStart=:+/usr/bin/farhelm supervisor run\n"),
            Some(PathBuf::from("/usr/bin/farhelm"))
        );
        // The parser preserves an unexpanded specifier literally rather
        // than guessing what systemd would substitute. The ownership
        // comparison is what rejects it: canonicalising `%h/...` cannot
        // resolve, so it never matches the running binary.
        assert_eq!(
            exec_start_program("[Service]\nExecStart=%h/.local/lib/farhelm/farhelm helm run\n"),
            Some(PathBuf::from("%h/.local/lib/farhelm/farhelm"))
        );
        assert_eq!(exec_start_program("[Service]\nRestart=always\n"), None);
        assert_eq!(
            exec_start_program("[Service]\nExecStart=\"/unterminated\n"),
            None
        );
        assert_eq!(exec_start_program("[Service]\nExecStart=\n"), None);
    }

    /// Which assignment is the EFFECTIVE one decides whether the panel
    /// believes a unit runs this farhelm. Systemd's own rules — section
    /// scoping, last-assignment-wins, and the empty reset — have to be
    /// the ones this parser follows, because a unit that exercises any of
    /// them is still a unit systemd runs.
    #[test]
    fn only_the_effective_service_assignment_is_reported() {
        // An assignment outside [Service] is not a command at all.
        assert_eq!(
            exec_start_program("[Unit]\nExecStart=/usr/bin/farhelm run\n"),
            None
        );
        assert_eq!(
            exec_start_program(
                "[Unit]\nExecStart=/decoy/farhelm run\n[Service]\nExecStart=/real/farhelm run\n"
            ),
            Some(PathBuf::from("/real/farhelm"))
        );
        // Leaving [Service] ends its scope.
        assert_eq!(
            exec_start_program(
                "[Service]\nExecStart=/real/farhelm run\n[Install]\nExecStart=/decoy/farhelm\n"
            ),
            Some(PathBuf::from("/real/farhelm"))
        );
        // Last one wins: a later assignment overwrites the earlier.
        assert_eq!(
            exec_start_program(
                "[Service]\nExecStart=/first/farhelm run\nExecStart=/second/farhelm run\n"
            ),
            Some(PathBuf::from("/second/farhelm"))
        );
        // The empty spelling resets the list; nothing runs afterwards.
        assert_eq!(
            exec_start_program("[Service]\nExecStart=/first/farhelm run\nExecStart=\n"),
            None
        );
        // ...and a reset can itself be followed by the real command.
        assert_eq!(
            exec_start_program(
                "[Service]\nExecStart=/first/farhelm\nExecStart=\nExecStart=/third/farhelm\n"
            ),
            Some(PathBuf::from("/third/farhelm"))
        );
        // Section names are case-insensitive to systemd.
        assert_eq!(
            exec_start_program("[service]\nExecStart=/real/farhelm run\n"),
            Some(PathBuf::from("/real/farhelm"))
        );
    }

    /// Unit discovery has to land in the directory the user manager
    /// actually searches. Both branches of the `${XDG_CONFIG_HOME:-…}`
    /// rule are pinned here because a wrong answer produces a
    /// valid-looking unit that is never loaded.
    #[test]
    fn user_unit_dir_follows_the_xdg_rule_in_both_directions() {
        let home = Path::new("/home/u");
        assert_eq!(
            user_unit_dir(None, home),
            PathBuf::from("/home/u/.config/systemd/user")
        );
        assert_eq!(
            user_unit_dir(Some(OsStr::new("")), home),
            PathBuf::from("/home/u/.config/systemd/user")
        );
        assert_eq!(
            user_unit_dir(Some(OsStr::new("/xdg")), home),
            PathBuf::from("/xdg/systemd/user")
        );
        // Relative values are ignored by the XDG spec, and treated as
        // unsupported by provisioning's reach check.
        assert_eq!(
            user_unit_dir(Some(OsStr::new("relative")), home),
            PathBuf::from("/home/u/.config/systemd/user")
        );
    }

    /// The ownership lookup runs where `HOME` may be absent, and there
    /// the difference between "no unit" and "I cannot tell where units
    /// live" is a safety property: the first permits the panel to
    /// install, the second must not. An absolute `XDG_CONFIG_HOME`
    /// answers on its own; nothing usable at all is an error.
    #[test]
    fn an_indeterminate_unit_directory_is_an_error_not_an_absence() {
        assert_eq!(
            user_unit_dir_for(Some(OsStr::new("/xdg")), None).unwrap(),
            PathBuf::from("/xdg/systemd/user")
        );
        assert_eq!(
            user_unit_dir_for(None, Some(Path::new("/home/u"))).unwrap(),
            PathBuf::from("/home/u/.config/systemd/user")
        );
        assert!(user_unit_dir_for(None, None).is_err());
        assert!(user_unit_dir_for(Some(OsStr::new("")), None).is_err());
        // A relative XDG value is ignored, which with no HOME leaves
        // nothing to resolve against.
        assert!(user_unit_dir_for(Some(OsStr::new("relative")), None).is_err());
    }
}
