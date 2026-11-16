//! The systemd user unit Farhelm writes, and the one renderer that fills
//! it in.
//!
//! Remote provisioning renders the supervisor unit and pushes it to a host
//! over SSH. The template under `crates/farhelm-helm/units/` is the
//! canonical policy: lifecycle behaviour lives in one reviewed file rather
//! than in the code that writes it.
//!
//! The other half of what lives here is reading the systemd user manager's
//! environment ([`ManagerEnvironment`]), which provisioning and podman do.
//!
//! Everything here is pure: no environment reads, no filesystem access,
//! no process spawning. Callers pass the paths they resolved.

use anyhow::{Context as _, bail};
use std::path::Path;

/// The supervisor unit provisioning installs on a host.
pub const SUPERVISOR_UNIT_NAME: &str = "farhelm-supervisor.service";

/// The supervisor unit as reviewed, with `@…@` placeholders for the two
/// paths [`SupervisorUnitInputs`] supplies plus the `@PATH@` this module
/// derives from the program's.
const SUPERVISOR_UNIT_TEMPLATE: &str = include_str!("../units/farhelm-supervisor.service.in");

/// The tail of the supervisor unit's search path, after the directory its
/// own executable lives in.
const BASE_PATH: [&str; 3] = ["/usr/local/bin", "/usr/bin", "/bin"];

/// The paths the supervisor unit pins.
///
/// There is deliberately no tmux here. The supervisor runs every session in
/// a terminal host and starts no tmux, so nothing the unit could name would
/// be read. A unit an earlier release rendered still carries a
/// `FARHELM_TMUX=` line until it is rendered again; the supervisor ignores
/// it, and keeps it out of the sessions it starts.
pub struct SupervisorUnitInputs<'a> {
    pub farhelm: &'a Path,
    pub state_dir: &'a Path,
}

/// Render the supervisor unit.
///
/// `PATH` is composed here: the directory holding the Farhelm program the
/// unit runs, then [`BASE_PATH`]. A systemd user manager does not
/// necessarily inherit the login shell's `PATH`, so this is what the
/// supervisor shells out with and what its sessions' login shells start
/// from before their startup files run. Farhelm's directory comes first so
/// `farhelm` resolves to this installation's copy. A program with no
/// directory part contributes no `PATH` entry rather than an empty one,
/// which POSIX would read as the current directory.
///
/// `KillMode=process` in the template is equally deliberate: the terminals
/// own the durable sessions (each terminal's terminal host), and they live
/// in the unit's control group, so restarting their manager must stop only
/// the supervisor process rather than systemd's default whole control group.
/// A private tmux server an earlier release left there survives a restart
/// the same way, so the new supervisor's startup step can end it and sweep
/// what it held (the supervisor's `service::core::old_terminals`).
///
/// So is `UnsetEnvironment=` for the three session markers
/// (`FARHELM_SESSION_ID`, `FARHELM_AGENT_ID`, `FARHELM_TAB_ID`). The kill
/// sweep claims every process carrying a session's markers, and nothing
/// exempts the supervisor. A tab's startup file that runs
/// a bare `systemctl --user import-environment` (or `dbus-update-activation-
/// environment --systemd --all`) copies that tab's markers into the user
/// manager, and the next supervisor it starts would inherit them: deleting
/// that session, or closing that tab, would then make the supervisor signal
/// itself. A supervisor systemd manages never legitimately belongs to
/// a session, so stripping them costs nothing. This covers only supervisors
/// started through this unit; SPEC_impl.md records the limit.
///
/// Fails when any path cannot cross a text boundary faithfully (see
/// [`path_text`]), the Farhelm program path contains `$`, or a `PATH`
/// component contains `:`, which systemd's `Environment=` grammar cannot
/// represent. Dollars remain supported in the state directory argument; the
/// program position does not decode the `$$` used for arguments.
pub fn render_supervisor_unit(inputs: &SupervisorUnitInputs) -> anyhow::Result<String> {
    let farhelm = systemd_program(inputs.farhelm)?;
    let mut components = Vec::with_capacity(1 + BASE_PATH.len());
    if let Some(dir) = inputs
        .farhelm
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
    {
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
            ("@FARHELM@", &farhelm),
            ("@STATE_DIR@", &systemd_arg(inputs.state_dir)?),
            ("@PATH@", &environment_value(&components.join(":"))),
        ],
    ))
}

/// Quote a supported program path without applying argument-only semantics.
///
/// Provisioning must refuse dollars before publishing a unit:
/// systemd preserves dollars in the program position, while [`systemd_arg`]
/// doubles them for argument expansion. Keeping this check separate preserves
/// supported dollar spellings in the state directory and environment values.
fn systemd_program(path: &Path) -> anyhow::Result<String> {
    let text = path_text(path)?;
    if text.contains('$') {
        bail!(
            "farhelm executable path {text:?} contains '$', which is unsupported in a systemd \
             service's program path; install farhelm at a path without '$' and retry"
        );
    }
    systemd_arg(path)
}

/// Quote one path for systemd's `ExecStart=` argument grammar.
///
/// Four characters mean something to systemd inside a command line and all
/// four have to be doubled or escaped, or the unit names something other
/// than the path the caller chose:
///
/// - `\` and `"` are unescaped inside the quoted form.
/// - `%` introduces a unit specifier anywhere in the line.
/// - `$` introduces variable expansion: `${NAME}` expands inside an
///   argument and `$NAME` expands to a WHOLE-WORD split, so an unescaped
///   one can redirect the state directory or turn one argument into several.
///   A literal dollar is spelled `$$`. This rule does not apply to the
///   program position; [`systemd_program`] refuses dollars there.
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
/// Used for the supervisor unit's `PATH`; the escaping rules are
/// [`systemd_arg`]'s, minus the surrounding quotes the template already
/// supplies — and minus the dollar.
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

/// The arguments, after `busctl --user`, that read the systemd user
/// manager's environment as JSON: `{"type":"as","data":["NAME=VALUE",…]}`.
///
/// This is the structured source for where the manager loads units from.
/// `systemctl --user show-environment` prints the same block for people:
/// systemd 255 and later print a value with whitespace or shell-special
/// characters as `NAME=$'…'`, which a line parser misreads, so a config
/// directory with a space in its path used to be refused as "relative" or
/// looked up in the wrong place. JSON strings arrive exactly as stored.
/// `--json` needs systemd 240 or later.
pub const MANAGER_ENVIRONMENT_BUSCTL_ARGS: [&str; 6] = [
    "get-property",
    "org.freedesktop.systemd1",
    "/org/freedesktop/systemd1",
    "org.freedesktop.systemd1.Manager",
    "Environment",
    "--json=short",
];

/// One variable of the systemd user manager's environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerValue {
    /// Not set, or set to the empty string (which every reader here treats
    /// as unset, like the shell's `${VAR:-default}`).
    Absent,
    Value(String),
    /// Present, but only in `show-environment`'s escaped `$'…'` form, which
    /// this reader does not decode. A caller refuses rather than guess.
    Unreadable,
}

/// The systemd user manager's environment, read from `busctl` JSON when the
/// user bus answers and from `systemctl --user show-environment` otherwise.
///
/// Two sources because `busctl --user` needs a D-Bus user bus, which some
/// minimal hosts lack even though `systemctl --user` reaches the manager
/// through its private socket; falling back keeps those hosts working for
/// every ordinary path, and only a value that needed escaping is then
/// [`ManagerValue::Unreadable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagerEnvironment {
    entries: Vec<(String, ManagerValue)>,
}

impl ManagerEnvironment {
    /// Decode systemctl's shell-literal output as data, preserving empty
    /// assignments and ANSI-C escapes. No assignment is evaluated or sourced.
    /// Proxy inheritance needs exact values even when a host lacks busctl.
    pub fn from_show_environment_exact(text: &str) -> anyhow::Result<Self> {
        let mut entries = Vec::new();
        for line in text.lines() {
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            entries.push((
                name.to_string(),
                ManagerValue::Value(decode_manager_value(value)?),
            ));
        }
        Ok(Self { entries })
    }

    /// Parse `busctl --user get-property … Environment --json=short` output.
    pub fn from_busctl_json(text: &str) -> anyhow::Result<Self> {
        #[derive(serde::Deserialize)]
        struct Property {
            #[serde(rename = "type")]
            kind: String,
            data: Vec<String>,
        }
        let property: Property = serde_json::from_str(text.trim()).map_err(|error| {
            anyhow::anyhow!("parsing the manager environment from busctl: {error}")
        })?;
        if property.kind != "as" {
            bail!(
                "busctl reported the manager environment as type {:?}, not a string array",
                property.kind
            );
        }
        let entries = property
            .data
            .iter()
            .filter_map(|entry| entry.split_once('='))
            .map(|(name, value)| (name.to_string(), ManagerValue::Value(value.to_string())))
            .collect();
        Ok(Self { entries })
    }

    /// Parse lines of `busctl … Environment --json=pretty` output that each
    /// hold one array element (`"NAME=VALUE",`), such as the ones a remote
    /// host filters out of the full output. Every non-empty line must be one
    /// JSON string, optionally followed by the array's comma.
    pub fn from_busctl_pretty_lines(text: &str) -> anyhow::Result<Self> {
        let mut entries = Vec::new();
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let element: String = serde_json::from_str(line.strip_suffix(',').unwrap_or(line))
                .map_err(|error| anyhow::anyhow!("parsing a busctl environment line: {error}"))?;
            if let Some((name, value)) = element.split_once('=') {
                entries.push((name.to_string(), plain(value)));
            }
        }
        Ok(Self { entries })
    }

    /// Parse `systemctl --user show-environment` output, the fallback when
    /// the user bus is unavailable. A value in plain quotes is unquoted; one
    /// in the `$'…'` form is [`ManagerValue::Unreadable`].
    pub fn from_show_environment(text: &str) -> Self {
        let entries = text
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(name, value)| {
                let value = value.trim();
                let parsed = if value.starts_with("$'") {
                    ManagerValue::Unreadable
                } else {
                    plain(
                        value
                            .strip_prefix('"')
                            .and_then(|value| value.strip_suffix('"'))
                            .or_else(|| {
                                value
                                    .strip_prefix('\'')
                                    .and_then(|value| value.strip_suffix('\''))
                            })
                            .unwrap_or(value),
                    )
                };
                (name.to_string(), parsed)
            })
            .collect();
        Self { entries }
    }

    /// The first assignment of `name`, as the manager applies it.
    pub fn get(&self, name: &str) -> ManagerValue {
        match self.get_exact(name) {
            ManagerValue::Value(value) if value.is_empty() => ManagerValue::Absent,
            other => other,
        }
    }

    /// Read structured environment input without collapsing an explicitly
    /// empty value. Proxy inheritance must preserve that choice exactly.
    /// The show-environment fallback retains its existing unreadable markers.
    pub fn get_exact(&self, name: &str) -> ManagerValue {
        self.entries
            .iter()
            .find(|(entry, _)| entry == name)
            .map_or(ManagerValue::Absent, |(_, value)| value.clone())
    }
}

/// systemctl emits either raw text or a quoted shell literal. Decoding the
/// finite literal grammar avoids executing substitutions in proxy credentials.
fn decode_manager_value(value: &str) -> anyhow::Result<String> {
    let (body, ansi) = if let Some(body) = value.strip_prefix("$'") {
        (
            body.strip_suffix('\'')
                .context("unterminated manager environment quote")?,
            true,
        )
    } else if let Some(body) = value.strip_prefix('\'') {
        return Ok(body
            .strip_suffix('\'')
            .context("unterminated manager environment quote")?
            .to_string());
    } else if let Some(body) = value.strip_prefix('"') {
        (
            body.strip_suffix('"')
                .context("unterminated manager environment quote")?,
            false,
        )
    } else {
        return Ok(value.to_string());
    };
    let mut decoded = Vec::new();
    let mut characters = body.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.extend(character.to_string().bytes());
            continue;
        }
        let escaped = characters
            .next()
            .context("truncated manager environment escape")?;
        if !ansi {
            // POSIX double quotes only consume these backslashes. An old
            // systemctl's literal \n remains two bytes in a quoted value.
            if matches!(escaped, '$' | '\u{60}' | '"' | '\\') {
                decoded.extend(escaped.to_string().bytes());
            } else if escaped != '\n' {
                decoded.push(b'\\');
                decoded.extend(escaped.to_string().bytes());
            }
            continue;
        }
        match escaped {
            'a' => decoded.push(7),
            'b' => decoded.push(8),
            'e' | 'E' => decoded.push(27),
            'f' => decoded.push(12),
            'n' => decoded.push(b'\n'),
            'r' => decoded.push(b'\r'),
            't' => decoded.push(b'\t'),
            'v' => decoded.push(11),
            '\\' | '\'' | '"' | '?' => decoded.push(escaped as u8),
            'x' | 'u' | 'U' => {
                let limit = match escaped {
                    'x' => 2,
                    'u' => 4,
                    'U' => 8,
                    _ => unreachable!(),
                };
                let mut digits = String::new();
                while digits.len() < limit
                    && characters
                        .peek()
                        .is_some_and(|character| character.is_ascii_hexdigit())
                {
                    digits.push(characters.next().unwrap());
                }
                anyhow::ensure!(
                    !digits.is_empty(),
                    "invalid manager environment hexadecimal escape"
                );
                let codepoint = u32::from_str_radix(&digits, 16)?;
                if escaped == 'x' {
                    decoded.push(u8::try_from(codepoint)?);
                } else {
                    decoded.extend(
                        char::from_u32(codepoint)
                            .context("invalid manager environment codepoint")?
                            .to_string()
                            .bytes(),
                    );
                }
            }
            '0'..='7' => {
                let mut digits = escaped.to_string();
                while digits.len() < 3
                    && characters
                        .peek()
                        .is_some_and(|character| matches!(*character, '0'..='7'))
                {
                    digits.push(characters.next().unwrap());
                }
                let codepoint = u32::from_str_radix(&digits, 8)?;
                decoded.push(u8::try_from(codepoint)?);
            }
            _ => bail!("unsupported manager environment escape"),
        }
    }
    anyhow::ensure!(
        !decoded.contains(&0),
        "manager environment contains a NUL value"
    );
    String::from_utf8(decoded).context("manager environment is not UTF-8")
}

/// A raw value, with the empty string read as unset.
fn plain(value: &str) -> ManagerValue {
    if value.is_empty() {
        ManagerValue::Absent
    } else {
        ManagerValue::Value(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Proxy inputs are copied into another environment, so an explicitly
    /// empty assignment and every escaped byte must survive the fallback.
    #[farhelm_testtrace::test]
    fn exact_manager_environment_preserves_shell_literals_without_evaluation() {
        let environment = ManagerEnvironment::from_show_environment_exact(
            "https_proxy=$'https://u:p a\\'ss@$proxy.example/\\nline'\n\
             HTTP_PROXY=\n\
             no_proxy=\"space and \\\"quote\\\" and \\$dollar\"\n\
             unicode=$'\\xc3\\xa9\\040\\u03bb'\n",
        )
        .unwrap();
        assert_eq!(
            environment.get_exact("https_proxy"),
            ManagerValue::Value("https://u:p a'ss@$proxy.example/\nline".into())
        );
        assert_eq!(
            environment.get_exact("HTTP_PROXY"),
            ManagerValue::Value(String::new())
        );
        assert_eq!(environment.get("HTTP_PROXY"), ManagerValue::Absent);
        assert_eq!(
            environment.get_exact("no_proxy"),
            ManagerValue::Value("space and \"quote\" and $dollar".into())
        );
        assert_eq!(
            environment.get_exact("unicode"),
            ManagerValue::Value("é λ".into())
        );
        assert!(ManagerEnvironment::from_show_environment_exact("x=$'\\x'").is_err());
    }

    /// The manager's environment reads the same from `busctl` JSON and from
    /// the plain `show-environment` lines, and a value `show-environment`
    /// escapes is reported unreadable instead of misread.
    ///
    /// Why it matters: systemd 255 prints a value with a space as
    /// `NAME=$'…'`, and the old line parser read that as a relative path, so
    /// a config directory with a space was refused or looked up in the wrong
    /// place. Specified: JSON yields the exact value, spaces and quotes
    /// included; the fallback unquotes plain quoted values, marks `$'…'`
    /// values unreadable, and treats empty and missing names as absent;
    /// only the first assignment of a name counts.
    #[test]
    fn manager_environment_parses_busctl_json_and_the_show_environment_fallback() {
        let json = r#"{"type":"as","data":["HOME=/home/u","XDG_CONFIG_HOME=/home/u/my config","EMPTY=","XDG_CONFIG_HOME=/second"]}"#;
        let env = ManagerEnvironment::from_busctl_json(json).unwrap();
        assert_eq!(
            env.get("XDG_CONFIG_HOME"),
            ManagerValue::Value("/home/u/my config".to_string())
        );
        assert_eq!(env.get("HOME"), ManagerValue::Value("/home/u".to_string()));
        assert_eq!(env.get("EMPTY"), ManagerValue::Absent);
        assert_eq!(env.get("MISSING"), ManagerValue::Absent);
        assert!(ManagerEnvironment::from_busctl_json(r#"{"type":"s","data":"x"}"#).is_err());
        let pretty = ManagerEnvironment::from_busctl_pretty_lines(
            "\t\t\"XDG_CONFIG_HOME=/home/u/my config\",\n\t\t\"LAST=a\\nb\"\n",
        )
        .unwrap();
        assert_eq!(
            pretty.get("XDG_CONFIG_HOME"),
            ManagerValue::Value("/home/u/my config".to_string())
        );
        assert_eq!(pretty.get("LAST"), ManagerValue::Value("a\nb".to_string()));
        assert!(ManagerEnvironment::from_busctl_pretty_lines("not a string").is_err());
        assert!(ManagerEnvironment::from_busctl_json("not json").is_err());

        let shown = ManagerEnvironment::from_show_environment(
            "HOME=/home/u\nXDG_CONFIG_HOME=$'/home/u/my config'\nQUOTED=\"/a\"\nEMPTY=\n",
        );
        assert_eq!(
            shown.get("HOME"),
            ManagerValue::Value("/home/u".to_string())
        );
        assert_eq!(shown.get("XDG_CONFIG_HOME"), ManagerValue::Unreadable);
        assert_eq!(shown.get("QUOTED"), ManagerValue::Value("/a".to_string()));
        assert_eq!(shown.get("EMPTY"), ManagerValue::Absent);
    }

    /// The supervisor unit strips exactly the markers the kill sweep claims
    /// processes by.
    ///
    /// The unit spells the names as literal text, while the supervisor sets
    /// and reads them through its own constants. If a marker were renamed
    /// there and not here, the unit would keep stripping a name nothing uses,
    /// and a supervisor that inherited the new marker from a polluted user
    /// manager could be claimed by its own sweep again, with every exact-text
    /// test still green.
    #[farhelm_testtrace::test]
    fn the_supervisor_unit_strips_the_sweeps_session_markers() {
        use farhelm_supervisor::launch::{AGENT_ID_ENV_VAR, SESSION_ID_ENV_VAR, TAB_ID_ENV_VAR};
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/usr/local/bin/farhelm"),
            state_dir: Path::new("/srv/farhelm"),
        })
        .unwrap();
        let unset = unit
            .lines()
            .find_map(|line| line.strip_prefix("UnsetEnvironment="))
            .expect("the supervisor unit carries an UnsetEnvironment= line");
        let mut stripped: Vec<&str> = unset.split_whitespace().collect();
        stripped.sort_unstable();
        let mut markers = vec![SESSION_ID_ENV_VAR, AGENT_ID_ENV_VAR, TAB_ID_ENV_VAR];
        markers.sort_unstable();
        assert_eq!(stripped, markers);
    }

    /// The complete text of the supervisor unit, pinned.
    ///
    /// The template IS the lifecycle policy — `KillMode=process` is what
    /// keeps a supervisor restart from killing the terminals it manages,
    /// `WantedBy=default.target` is what makes the unit start at all — and
    /// a substring test would stay green while any of those directives
    /// silently disappeared. The expectation is written out by hand rather
    /// than derived from the template so that a mistake in the renderer
    /// cannot update the implementation and the expectation together.
    #[farhelm_testtrace::test]
    fn the_supervisor_unit_renders_to_its_exact_reviewed_text() {
        let supervisor = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/home/u/.local/bin/farhelm"),
            state_dir: Path::new("/home/u/.local/state/farhelm"),
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
                "Environment=\"PATH=/home/u/.local/bin:/usr/local/bin:/usr/bin:/bin\"\n",
                "UnsetEnvironment=FARHELM_SESSION_ID FARHELM_AGENT_ID FARHELM_TAB_ID\n",
                "KillMode=process\n",
                "Restart=on-failure\n",
                "\n",
                "[Install]\n",
                "WantedBy=default.target\n",
            )
        );
    }

    /// Dollar arguments must still survive the program-path refusal.
    /// Systemd expands arguments, so applying the program restriction to
    /// them would reject supported state directories. Spec: each dollar in
    /// the state directory is written doubled, as `$$`.
    #[farhelm_testtrace::test]
    fn dollars_in_the_state_directory_are_escaped() {
        for path in [
            "/state/$NAME",
            "/state/${NAME}",
            "/state/$",
            "/state/$$literal",
        ] {
            let unit = render_supervisor_unit(&SupervisorUnitInputs {
                farhelm: Path::new("/usr/bin/farhelm"),
                state_dir: Path::new(path),
            })
            .unwrap();
            assert!(
                unit.contains(&format!("--state-dir \"{}\"", path.replace('$', "$$"))),
                "{unit}"
            );
        }
    }

    /// The unit must refuse the path before writing an unusable service.
    /// The shared argument formatter must remain permissive for
    /// dollar-bearing state paths, so this belongs at the program input.
    #[farhelm_testtrace::test]
    fn the_supervisor_unit_refuses_dollars_in_the_program_path() {
        for path in [
            "/opt/$NAME/farhelm",
            "/opt/${NAME}/farhelm",
            "/opt/$$literal/farhelm",
        ] {
            let message = render_supervisor_unit(&SupervisorUnitInputs {
                farhelm: Path::new(path),
                state_dir: Path::new("/state"),
            })
            .unwrap_err()
            .to_string();
            assert!(message.contains(path), "{message}");
            assert!(
                message.contains("contains '$'")
                    && message.contains("install farhelm at a path without '$'"),
                "{message}"
            );
        }
    }

    /// `Environment=` has its own escape branch, so the characters that
    /// are only covered for `ExecStart` elsewhere need their own complete
    /// expectations here: a quote or backslash reaching systemd unescaped
    /// ends the assignment early and leaves the supervisor with a PATH that
    /// is not the one this render chose.
    #[farhelm_testtrace::test]
    fn environment_values_escape_quotes_and_backslashes() {
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/opt/a\"b\\c/farhelm"),
            state_dir: Path::new("/state"),
        })
        .unwrap();
        assert!(
            unit.contains("Environment=\"PATH=/opt/a\\\"b\\\\c:/usr/local/bin:/usr/bin:/bin\"\n"),
            "{unit}"
        );
    }

    /// A path may legally contain the very text the templates use as
    /// placeholders. The renderer walks the template once for that
    /// reason, and the invariant is worth a test because the obvious
    /// simplification — chained `str::replace` — would corrupt such a
    /// path while every other test stayed green.
    #[farhelm_testtrace::test]
    fn inserted_text_is_never_rescanned_as_template_syntax() {
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/opt/@STATE_DIR@/farhelm"),
            state_dir: Path::new("/opt/@PATH@"),
        })
        .unwrap();
        assert!(
            unit.contains(
                "ExecStart=\"/opt/@STATE_DIR@/farhelm\" supervisor run --state-dir \
                 \"/opt/@PATH@\"\n"
            ),
            "{unit}"
        );
        assert!(
            unit.contains("Environment=\"PATH=/opt/@STATE_DIR@:/usr/local/bin:/usr/bin:/bin\"\n"),
            "{unit}"
        );
    }

    /// A path that cannot cross the unit file's text boundary must fail
    /// before anything is written, and a `PATH` component holding the
    /// separator systemd uses cannot be represented at all.
    #[farhelm_testtrace::test]
    fn unrepresentable_paths_are_refused_rather_than_approximated() {
        let percent = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("/tmp/%h/farhelm"),
            state_dir: Path::new("/tmp/state"),
        })
        .unwrap();
        assert!(percent.contains("\"/tmp/%%h/farhelm\""));
        assert!(percent.contains("PATH=/tmp/%%h:"));

        assert!(
            render_supervisor_unit(&SupervisorUnitInputs {
                farhelm: Path::new("/tmp/farhelm"),
                state_dir: Path::new("/tmp/state\nother"),
            })
            .is_err()
        );
        assert!(
            render_supervisor_unit(&SupervisorUnitInputs {
                farhelm: Path::new("/tmp/with:colon/farhelm"),
                state_dir: Path::new("/tmp/state"),
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
                })
                .is_err()
            );
        }
    }

    /// Systemd argument escaping handles each special character in its own
    /// grammar rather than borrowing shell quoting rules. A `%h` that
    /// reached systemd unexpanded would name a different binary than the
    /// caller chose.
    #[farhelm_testtrace::test]
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
    /// would let a stray program in whatever directory the user manager
    /// started in win. Every caller passes an absolute program today; this
    /// pins the guard that keeps a future relative one from opening that
    /// hole.
    #[farhelm_testtrace::test]
    fn a_bare_program_name_contributes_no_path_entry() {
        let unit = render_supervisor_unit(&SupervisorUnitInputs {
            farhelm: Path::new("farhelm"),
            state_dir: Path::new("/state"),
        })
        .unwrap();
        assert!(
            unit.contains("Environment=\"PATH=/usr/local/bin:/usr/bin:/bin\""),
            "{unit}"
        );
    }
}
