//! Strict attribution of tmux's retained launch command.
//!
//! `pane_start_command` is untrusted observation data, even on a private tmux
//! server: a pane can be foreign, old, or malformed. This parser recognizes
//! only the exact argv the current launch producer writes and never reads the
//! launch spec, which is normally deleted by the exec shim before inspection.

use farhelm_supervisor::{launch, scope};
use std::{
    fmt,
    os::unix::ffi::OsStrExt as _,
    path::{Path, PathBuf},
};

const MAX_ENCODED_BYTES: usize = 16 * 1024;
const MAX_ARGUMENTS: usize = 32;
const MAX_ARGUMENT_BYTES: usize = 8 * 1024;

/// Facts that tie one retained launch command to an installed executable.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct LaunchOrigin {
    pub(crate) executable: PathBuf,
    pub(crate) state_dir: PathBuf,
    pub(crate) session_id: String,
    pub(crate) generation: i64,
    pub(crate) scope_unit: Option<String>,
}

/// Safe parse categories for untrusted retained command text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OriginError {
    TooLong,
    TooManyArguments,
    ArgumentTooLong,
    InvalidEscape,
    InvalidUtf8,
    OuterShape,
    InnerShape,
    ScopeShape,
    PathShape,
    SpecShape,
}

impl fmt::Display for OriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unrecognized retained launch command: {:?}",
            self
        )
    }
}

impl std::error::Error for OriginError {}

/// Decode tmux `args_escape` output and recognize the current launch argv.
///
/// Tmux vis escapes are data escapes even inside quoted words, unlike POSIX
/// shell quoting. Decoding stays byte-oriented until the producer's UTF-8
/// shell command is required; no expansion, filesystem read, or lossy path
/// conversion occurs on this trust boundary.
pub(crate) fn parse_pane_start_command(command: &[u8]) -> Result<LaunchOrigin, OriginError> {
    let outer = decode_tmux_argv(command)?;
    if outer.len() != 5 || outer[1] != b"-l" || outer[2] != b"-i" || outer[3] != b"-c" {
        return Err(OriginError::OuterShape);
    }
    let inner = std::str::from_utf8(&outer[4]).map_err(|_| OriginError::InvalidUtf8)?;
    let words = shell_words::split(inner).map_err(|_| OriginError::InnerShape)?;
    let reconstructed = words
        .iter()
        .map(|word| shell_words::quote(word))
        .collect::<Vec<_>>()
        .join(" ");
    if reconstructed != inner {
        return Err(OriginError::InnerShape);
    }
    if words.len() < 5
        || words[0] != "exec"
        || words[words.len() - 3..] != ["internal", "launch", words[words.len() - 1].as_str()]
    {
        return Err(OriginError::InnerShape);
    }
    let spec = PathBuf::from(&words[words.len() - 1]);
    let executable_index = match words.len() {
        5 => 1,
        _ => parse_scope_prefix(&words)?,
    };
    let executable = PathBuf::from(&words[executable_index]);
    if !is_absolute_raw(&executable) || !is_absolute_raw(&spec) {
        return Err(OriginError::PathShape);
    }
    let (state_dir, session_id, generation) = parse_spec_path(&spec)?;
    let scope_unit = if executable_index == 1 {
        None
    } else {
        Some(scope::unit_name(&session_id, generation).expect("validated UUID"))
    };
    Ok(LaunchOrigin {
        executable,
        state_dir,
        session_id,
        generation,
        scope_unit,
    })
}

/// Reverse the bounded subset of tmux's vis-style argv encoding.
fn decode_tmux_argv(command: &[u8]) -> Result<Vec<Vec<u8>>, OriginError> {
    if command.len() > MAX_ENCODED_BYTES {
        return Err(OriginError::TooLong);
    }
    let mut words = Vec::new();
    let mut index = 0;
    while index < command.len() {
        while index < command.len() && command[index] == b' ' {
            index += 1;
        }
        if index == command.len() {
            break;
        }
        if words.len() == MAX_ARGUMENTS {
            return Err(OriginError::TooManyArguments);
        }
        let mut quote = None;
        let mut word = Vec::new();
        while index < command.len() && !(command[index] == b' ' && quote.is_none()) {
            let byte = command[index];
            if byte == b'\'' || byte == b'\"' {
                if quote.is_none() {
                    quote = Some(byte);
                    index += 1;
                    continue;
                }
                if quote == Some(byte) {
                    quote = None;
                    index += 1;
                    continue;
                }
            }
            if byte == 0 {
                return Err(OriginError::InvalidEscape);
            }
            if byte == b'\\' {
                index += 1;
                let escaped = *command.get(index).ok_or(OriginError::InvalidEscape)?;
                let decoded = match escaped {
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'b' => 8,
                    b'a' => 7,
                    b'v' => 11,
                    b'f' => 12,
                    b's' => b' ',
                    b'\\' => b'\\',
                    b'\'' => b'\'',
                    b'\"' => b'\"',
                    // tmux protects shell-variable-shaped dollars in its
                    // display encoding, even inside the inner quoted command.
                    b'$' => b'$',
                    b'0'..=b'7' => {
                        let digits = command
                            .get(index..index + 3)
                            .ok_or(OriginError::InvalidEscape)?;
                        if !digits.iter().all(|digit| matches!(digit, b'0'..=b'7')) {
                            return Err(OriginError::InvalidEscape);
                        }
                        let value = u16::from(digits[0] - b'0') * 64
                            + u16::from(digits[1] - b'0') * 8
                            + u16::from(digits[2] - b'0');
                        if value == 0 || value > 255 {
                            return Err(OriginError::InvalidEscape);
                        }
                        index += 2;
                        value as u8
                    }
                    _ => return Err(OriginError::InvalidEscape),
                };
                word.push(decoded);
            } else {
                word.push(byte);
            }
            if word.len() > MAX_ARGUMENT_BYTES {
                return Err(OriginError::ArgumentTooLong);
            }
            index += 1;
        }
        if quote.is_some() {
            return Err(OriginError::InvalidEscape);
        }
        words.push(word);
    }
    Ok(words)
}

/// Validate the exact optional systemd-run prefix and return the executable index.
fn parse_scope_prefix(words: &[String]) -> Result<usize, OriginError> {
    let end = words
        .iter()
        .position(|word| word == "--")
        .ok_or(OriginError::ScopeShape)?;
    if end + 5 != words.len() {
        return Err(OriginError::ScopeShape);
    }
    let prefix = &words[1..=end];
    let program = Path::new(&prefix[0]);
    if !is_absolute_raw(program) || program.file_name().is_none_or(|name| name != "systemd-run") {
        return Err(OriginError::ScopeShape);
    }
    let required = ["--user", "--scope", "--collect", "--quiet"];
    if prefix.len() != 7 && prefix.len() != 8 || prefix[1..5] != required {
        return Err(OriginError::ScopeShape);
    }
    let unit_index = if prefix.len() == 8 {
        if prefix[5] != "--expand-environment=no" {
            return Err(OriginError::ScopeShape);
        }
        6
    } else {
        5
    };
    let unit = prefix[unit_index]
        .strip_prefix("--unit=")
        .ok_or(OriginError::ScopeShape)?;
    let spec = Path::new(words.last().expect("inner has spec"));
    let (_, session, generation) = parse_spec_path(spec)?;
    if scope::unit_name(&session, generation).as_deref() != Some(unit) {
        return Err(OriginError::ScopeShape);
    }
    Ok(end + 1)
}

/// Split a production-shaped launch path without touching its absent file.
fn parse_spec_path(spec: &Path) -> Result<(PathBuf, String, i64), OriginError> {
    if !is_absolute_raw(spec) {
        return Err(OriginError::PathShape);
    }
    let name = spec
        .file_name()
        .and_then(|part| part.to_str())
        .ok_or(OriginError::SpecShape)?;
    let (session, generation) =
        launch::parse_launch_file_name(name).ok_or(OriginError::SpecShape)?;
    if !scope::is_uuid_shaped(session) || !canonical_generation(generation, name) {
        return Err(OriginError::SpecShape);
    }
    let launch_dir = spec
        .parent()
        .filter(|parent| parent.file_name().is_some_and(|part| part == "launch"))
        .ok_or(OriginError::SpecShape)?;
    let state = launch_dir
        .parent()
        .ok_or(OriginError::SpecShape)?
        .to_path_buf();
    let reconstructed = launch::spec_path_for_launch(&state, session, generation);
    if reconstructed.as_os_str().as_encoded_bytes() != spec.as_os_str().as_encoded_bytes() {
        return Err(OriginError::SpecShape);
    }
    Ok((state, session.to_owned(), generation))
}

/// Require the decimal spelling emitted by the launch-spec producer.
fn canonical_generation(generation: i64, name: &str) -> bool {
    generation >= 0
        && name
            .rsplit_once('.')
            .and_then(|(stem, _)| stem.rsplit_once('.'))
            .is_some_and(|(_, rendered)| rendered == generation.to_string())
}

/// Refuse every Unix spelling that path APIs would silently normalize.
///
/// Root is normalized on its own. Every other accepted path has exactly one
/// leading slash and nonempty ordinary components, so later `Path` traversal
/// cannot turn foreign retained bytes into a producer-shaped identity.
fn is_absolute_raw(path: &Path) -> bool {
    let bytes = path.as_os_str().as_bytes();
    if bytes == b"/" {
        return true;
    }
    bytes.first() == Some(&b'/')
        && bytes.last() != Some(&b'/')
        && bytes[1..]
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..")
        && !bytes.contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uninstall::probe::{self, ProbeLimits};
    use std::{
        fs, os::unix::fs::PermissionsExt as _, path::PathBuf, process::Stdio, time::Duration,
    };
    use tokio::process::Command;

    const ID: &str = "123e4567-e89b-12d3-a456-426614174000";

    /// Join hand-authored outer words whose fixtures need no tmux escaping.
    fn encoded(words: &[&str]) -> Vec<u8> {
        words.join(" ").into_bytes()
    }

    /// Encode argv as vis-style octets so unit fixtures do not borrow shell semantics.
    fn encoded_octets(words: &[String]) -> Vec<u8> {
        words
            .iter()
            .map(|word| {
                word.as_bytes()
                    .iter()
                    .map(|byte| format!("\\{byte:03o}"))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
            .into_bytes()
    }
    /// Produce the ordinary canonical inner launch command used by shape tests.
    fn inner() -> String {
        format!("exec /opt/farhelm internal launch /state/launch/{ID}.0.json")
    }

    /// Bound native tmux queries without making startup timing the assertion.
    fn probe_limits() -> ProbeLimits {
        ProbeLimits {
            elapsed: Duration::from_secs(2),
            stdout_bytes: 32 * 1024,
            stderr_bytes: 32 * 1024,
            cleanup_elapsed: Duration::from_secs(1),
        }
    }

    /// Query only the fixture's socket, never an ambient tmux server.
    async fn tmux(socket: &Path, arguments: &[String]) -> probe::ProbeOutput {
        let mut command = Command::new(tmux_program());
        command
            .env_clear()
            .env("HOME", socket.parent().expect("fixture socket parent"))
            .env("PATH", "/usr/bin:/bin")
            .env("TERM", "xterm-256color")
            .arg("-u")
            .arg("-S")
            .arg(socket)
            .arg("-N")
            .arg("-f")
            .arg("/dev/null");
        command.args(arguments);
        probe::execute(&mut command, probe_limits())
            .await
            .expect("fixture tmux query completes")
    }

    /// Own the foreground tmux process across every assertion and panic path.
    struct ServerGuard(std::process::Child);

    impl Drop for ServerGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Resolve the test runner's selected tmux before clearing fixture env.
    ///
    /// The spawned server must not inherit test-process configuration, but the
    /// pinned test substrate reaches tmux through PATH. Resolving it once lets
    /// the fixture pass an absolute program while giving its child a minimal,
    /// explicit environment.
    fn tmux_program() -> PathBuf {
        std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|directory| directory.join("tmux"))
            .find(|candidate| {
                fs::metadata(candidate)
                    .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
            })
            .expect("pinned tmux on test PATH")
    }

    /// Wait for an observable fixture condition while its owned server lives.
    async fn wait_for(mut ready: impl FnMut() -> bool, server: &mut ServerGuard) {
        for _ in 0..100 {
            assert!(
                server
                    .0
                    .try_wait()
                    .expect("inspect fixture server")
                    .is_none(),
                "tmux server exited"
            );
            if ready() {
                return;
            }
            // sleep-ok: bounded polling observes the private fixture's socket or shim action.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("fixture readiness condition was not reached");
    }

    /// Wait until tmux independently reports the shim's final exec target.
    async fn wait_for_final_exec(socket: &Path, session: &str, server: &mut ServerGuard) {
        for _ in 0..100 {
            assert!(
                server
                    .0
                    .try_wait()
                    .expect("inspect fixture server")
                    .is_none(),
                "tmux server exited"
            );
            let output = tmux(
                socket,
                &[
                    "display-message".to_string(),
                    "-p".to_string(),
                    "-t".to_string(),
                    session.to_string(),
                    "#{pane_current_command}".to_string(),
                ],
            )
            .await;
            if output.status.success() && output.stdout == b"cat\n" {
                return;
            }
            // sleep-ok: bounded polling observes tmux's process identity after exec.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("fixture pane never execed the final executable");
    }

    /// The parser accepts only the producer's unscoped launch grammar.
    #[test]
    fn parses_unscoped_origin() {
        let inner = inner();
        let origin = parse_pane_start_command(&encoded(&[
            "/bin/sh",
            "-l",
            "-i",
            "-c",
            &shell_words::quote(&inner),
        ]))
        .expect("origin");
        assert_eq!(origin.executable, PathBuf::from("/opt/farhelm"));
        assert_eq!(origin.state_dir, PathBuf::from("/state"));
        assert_eq!(origin.session_id, ID);
        assert_eq!(origin.generation, 0);
        assert_eq!(origin.scope_unit, None);
    }

    /// The parser accepts exactly the quoting emitted by the current producer.
    #[test]
    fn accepts_producer_quoting_and_refuses_shell_active_equivalents() {
        let spec =
            Path::new("/state with space/launch/123e4567-e89b-12d3-a456-426614174000.0.json");
        let produced =
            launch::window_command("/bin/sh", Path::new("/opt/$HOME-farhelm"), spec, Vec::new());
        let origin =
            parse_pane_start_command(&encoded_octets(&produced)).expect("producer command");
        assert_eq!(origin.executable, PathBuf::from("/opt/$HOME-farhelm"));

        let mut active = produced;
        active[4] = active[4].replace("'/opt/$HOME-farhelm'", "/opt/$HOME-farhelm");
        assert_eq!(
            parse_pane_start_command(&encoded_octets(&active))
                .expect_err("active spelling rejected"),
            OriginError::InnerShape
        );
    }

    /// Malformed retained data stays uncertainty and never echoes the command.
    #[test]
    fn refuses_malformed_escapes_without_leaking_input() {
        for command in [
            b"'secret\\q'".as_slice(),
            b"'secret\\000'",
            b"'secret\\400'",
            b"'secret\\12'",
            b"'secret\\'",
            b"'secret",
            b"secret\0suffix",
        ] {
            let error = parse_pane_start_command(command).expect_err("escape rejected");
            assert_eq!(error, OriginError::InvalidEscape);
            assert!(!error.to_string().contains("secret"));
        }
    }

    /// Decoder bounds are inclusive and reject the first excess byte or word.
    #[test]
    fn enforces_decoder_limits_at_the_boundary() {
        assert_eq!(
            decode_tmux_argv(&vec![b' '; MAX_ENCODED_BYTES]),
            Ok(Vec::new())
        );
        assert_eq!(
            decode_tmux_argv(&vec![b' '; MAX_ENCODED_BYTES + 1]),
            Err(OriginError::TooLong)
        );
        assert_eq!(
            decode_tmux_argv(&vec![b'a'; MAX_ARGUMENT_BYTES]).expect("argument at limit")[0].len(),
            MAX_ARGUMENT_BYTES
        );
        assert_eq!(
            decode_tmux_argv(&vec![b'a'; MAX_ARGUMENT_BYTES + 1]),
            Err(OriginError::ArgumentTooLong)
        );
        let at_limit = vec!["x"; MAX_ARGUMENTS].join(" ");
        assert_eq!(
            decode_tmux_argv(at_limit.as_bytes())
                .expect("argument count at limit")
                .len(),
            MAX_ARGUMENTS
        );
        let over_limit = vec!["x"; MAX_ARGUMENTS + 1].join(" ");
        assert_eq!(
            decode_tmux_argv(over_limit.as_bytes()),
            Err(OriginError::TooManyArguments)
        );
    }

    /// Filename aliases cannot claim a launch that the producer did not create.
    #[test]
    fn refuses_noncanonical_generation_and_path_aliases() {
        for (executable, path) in [
            ("/opt/farhelm", format!("/state/launch/{ID}.00.json")),
            ("/opt/farhelm", format!("/state/launch/{ID}.-1.json")),
            ("/opt/farhelm", format!("/state/launch/{ID}.+1.json")),
            (
                "/opt/farhelm",
                format!("/state/launch/{ID}.9223372036854775808.json"),
            ),
            (
                "/opt/farhelm",
                format!("/state/launch/../launch/{ID}.0.json"),
            ),
            ("/opt/farhelm", format!("/state//launch/{ID}.0.json")),
            ("/opt/farhelm", format!("/state/./launch/{ID}.0.json")),
            ("/opt//farhelm", format!("/state/launch/{ID}.0.json")),
            ("/opt/./farhelm", format!("/state/launch/{ID}.0.json")),
            ("/opt/farhelm/", format!("/state/launch/{ID}.0.json")),
        ] {
            let inner = ["exec", executable, "internal", "launch", &path]
                .into_iter()
                .map(shell_words::quote)
                .collect::<Vec<_>>()
                .join(" ");
            let error = parse_pane_start_command(&encoded(&[
                "/bin/sh",
                "-l",
                "-i",
                "-c",
                &shell_words::quote(&inner),
            ]))
            .expect_err("alias rejected");
            assert!(matches!(
                error,
                OriginError::PathShape | OriginError::SpecShape
            ));
        }

        let root_spec = format!("/launch/{ID}.0.json");
        let root_inner = ["exec", "/opt/farhelm", "internal", "launch", &root_spec]
            .into_iter()
            .map(shell_words::quote)
            .collect::<Vec<_>>()
            .join(" ");
        let origin = parse_pane_start_command(&encoded(&[
            "/bin/sh",
            "-l",
            "-i",
            "-c",
            &shell_words::quote(&root_inner),
        ]))
        .expect("root state path is normalized");
        assert_eq!(origin.state_dir, Path::new("/"));
    }

    /// A scope prefix is provenance only when its derived unit still agrees.
    #[test]
    fn accepts_the_producer_scope_shape_and_refuses_a_mismatched_generation() {
        let unit = scope::unit_name(ID, 0).expect("fixture UUID");
        let inner = [
            "exec",
            "/usr/bin/systemd-run",
            "--user",
            "--scope",
            "--collect",
            "--quiet",
            &format!("--unit={unit}"),
            "--",
            "/opt/farhelm",
            "internal",
            "launch",
            &format!("/state/launch/{ID}.0.json"),
        ]
        .into_iter()
        .map(shell_words::quote)
        .collect::<Vec<_>>()
        .join(" ");
        let command = encoded(&["/bin/sh", "-l", "-i", "-c", &shell_words::quote(&inner)]);
        let origin = parse_pane_start_command(&command).expect("producer-shaped scope");
        assert_eq!(origin.scope_unit.as_deref(), Some(unit.as_str()));

        let mismatched = inner.replacen(&unit, &scope::unit_name(ID, 1).expect("unit"), 1);
        let command = encoded(&[
            "/bin/sh",
            "-l",
            "-i",
            "-c",
            &shell_words::quote(&mismatched),
        ]);
        assert_eq!(
            parse_pane_start_command(&command).expect_err("mismatch rejected"),
            OriginError::ScopeShape
        );

        let aliased = inner.replacen("/usr/bin/systemd-run", "/usr//bin/systemd-run", 1);
        let command = encoded(&["/bin/sh", "-l", "-i", "-c", &shell_words::quote(&aliased)]);
        assert_eq!(
            parse_pane_start_command(&command).expect_err("scope alias rejected"),
            OriginError::ScopeShape
        );
    }

    /// Retained tmux argv remains sufficient after the launch shim deletes its spec.
    ///
    /// The path classes pin tmux's vis encoding rather than hand-building a
    /// convenient approximation. Each shim owns its spec deletion, so the
    /// assertion cannot accidentally rely on a file the runtime will not have.
    #[tokio::test]
    async fn real_tmux_retains_and_decodes_deleted_launch_specs() {
        let fixture = tempfile::tempdir().expect("fixture directory");
        let socket = fixture.path().join("tmux.sock");
        let config = fixture.path().join("tmux.conf");
        fs::write(&config, "set -g exit-empty off\nset -g remain-on-exit on\n").expect("config");
        let mut server = ServerGuard(
            std::process::Command::new(tmux_program())
                .arg("-u")
                .args(["-D", "-S"])
                .arg(&socket)
                .arg("-f")
                .arg(&config)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", fixture.path())
                .env("TERM", "xterm-256color")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("start owned tmux server"),
        );
        wait_for(|| socket.exists(), &mut server).await;

        for (index, class) in [
            "ordinary",
            "space ' single \" double",
            "back\\slash\nnewline",
            "tab\tbell\u{7}escape\u{1b}",
            "unicode-å-雪",
            "dollar-$HOME-$_name-${name}",
        ]
        .iter()
        .enumerate()
        {
            let directory = fixture.path().join(class);
            fs::create_dir(&directory).expect("path-class directory");
            let shim = directory.join("farhelm");
            fs::write(
                &shim,
                "#!/bin/sh\n/bin/rm -- \"$3\" || exit 1\nexec /bin/cat\n",
            )
            .expect("shim");
            fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("executable shim");
            let state = fixture.path().join(format!("state-{index}"));
            let spec = launch::spec_path_for_launch(&state, ID, index as i64);
            fs::create_dir_all(spec.parent().expect("launch parent")).expect("launch directory");
            fs::write(&spec, "fixture spec").expect("spec");
            let command = launch::window_command("/bin/sh", &shim, &spec, Vec::new());
            let session = format!("origin-{index}");
            let mut arguments = vec![
                "new-session".to_string(),
                "-d".to_string(),
                "-s".to_string(),
                session.clone(),
                "--".to_string(),
            ];
            arguments.extend(command);
            assert!(
                tmux(&socket, &arguments).await.status.success(),
                "create fixture pane"
            );
            wait_for(|| !spec.exists(), &mut server).await;
            wait_for_final_exec(&socket, &session, &mut server).await;
            let output = tmux(
                &socket,
                &[
                    "display-message".to_string(),
                    "-p".to_string(),
                    "-t".to_string(),
                    session,
                    "#{pane_start_command}".to_string(),
                ],
            )
            .await;
            assert!(output.status.success(), "read retained command");
            let retained = output.stdout.strip_suffix(b"\n").unwrap_or(&output.stdout);
            let origin = parse_pane_start_command(retained).expect("decode real tmux command");
            assert_eq!(origin.executable, shim);
            assert_eq!(origin.state_dir, state);
            assert_eq!(origin.session_id, ID);
            assert_eq!(origin.generation, index as i64);
        }
    }
}
