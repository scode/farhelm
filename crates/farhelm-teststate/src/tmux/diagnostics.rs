//! Bounded private evidence from a tmux server that is about to be cleaned up.
//!
//! This module never controls a server or signals a process. It reuses the
//! shutdown module's socket authority only to run a small, independently
//! bounded set of read-only tmux clients. The retained command outcomes are
//! intentionally private evidence. A guard using this API must collect and emit
//! it before shutdown on every Drop. The outer test wrapper discards capture
//! only when the test succeeds; successful cleanup does not imply a passing
//! test. This API only returns evidence and does not decide either outcome.

use super::{
    SocketAuthority, TmuxPeerAcquisition, tmux_command, validate_executable, validate_socket,
};
use crate::process::{CommandRunConfigError, CommandRunLimits, CommandRunOutcome, run_bounded};
use std::path::Path;
use std::time::{Duration, Instant};

const TOTAL_ALLOWANCE: Duration = Duration::from_secs(2);
const CLEANUP_RESERVE: Duration = Duration::from_millis(250);
const STDERR_LIMIT: usize = 8 * 1024;
const MAX_PANE_ID_BYTES: usize = 64;
const MAX_CAPTURED_PANES: usize = 4;

const SESSIONS_STDOUT_LIMIT: usize = 4 * 1024;
const PANES_STDOUT_LIMIT: usize = 8 * 1024;
const CLIENTS_STDOUT_LIMIT: usize = 4 * 1024;
const CAPTURE_STDOUT_LIMIT: usize = 4 * 1024;

const FIELD_SEPARATOR: u8 = 0x1f;
const RECORD_SEPARATOR: u8 = 0x1e;

// Pane processes can write user options, and names are not framing authority.
// Sanitize control bytes inside text values before adding our own separators;
// otherwise a marker could manufacture another apparently complete pane row.
// tmux's substitution modifier replaces every match, including US, RS and LF.
// Use literal ASCII ranges: a POSIX class such as [[:cntrl:]] contains colons
// that tmux parses as the end of its modifier before the regex sees them. NUL
// cannot occur inside tmux's C-string values.
const SESSIONS_FORMAT: &str = concat!(
    "#{s/[\u{1}-\u{1f}\u{7f}]/_/:session_name}\u{1f}",
    "#{session_windows}\u{1f}#{session_attached}\u{1e}",
);
const PANES_FORMAT: &str = concat!(
    "#{pane_id}\u{1f}#{s/[\u{1}-\u{1f}\u{7f}]/_/:session_name}\u{1f}#{window_index}\u{1f}",
    "#{pane_dead}\u{1f}#{pane_dead_status}\u{1f}#{pane_width}\u{1f}#{pane_height}\u{1f}",
    "#{s/[\u{1}-\u{1f}\u{7f}]/_/:pane_current_command}\u{1f}",
    "#{s/[\u{1}-\u{1f}\u{7f}]/_/:@farhelm-agent}\u{1f}#{s/[\u{1}-\u{1f}\u{7f}]/_/:@farhelm-tab}\u{1e}",
);
const CLIENTS_FORMAT: &str = concat!(
    "#{s/[\u{1}-\u{1f}\u{7f}]/_/:client_session}\u{1f}",
    "#{s/[\u{1}-\u{1f}\u{7f}]/_/:client_flags}\u{1f}#{client_pid}\u{1e}",
);

/// The three metadata queries are fixed so callers cannot turn diagnostics
/// into an arbitrary tmux command runner.
#[derive(Clone, Copy)]
enum MetadataQuery {
    Sessions,
    Panes,
    Clients,
}

impl MetadataQuery {
    fn label(self) -> TmuxDiagnosticLabel {
        match self {
            Self::Sessions => TmuxDiagnosticLabel::Sessions,
            Self::Panes => TmuxDiagnosticLabel::Panes,
            Self::Clients => TmuxDiagnosticLabel::Clients,
        }
    }

    fn command(self) -> (&'static str, bool, &'static str, usize) {
        match self {
            Self::Sessions => (
                "list-sessions",
                false,
                SESSIONS_FORMAT,
                SESSIONS_STDOUT_LIMIT,
            ),
            Self::Panes => ("list-panes", true, PANES_FORMAT, PANES_STDOUT_LIMIT),
            Self::Clients => ("list-clients", false, CLIENTS_FORMAT, CLIENTS_STDOUT_LIMIT),
        }
    }
}

/// One bounded private tmux snapshot, including every attempted child outcome.
///
/// Metadata commands use fixed formats: sessions name their window count and
/// attachment state; panes include identity, liveness, geometry, command and
/// Farhelm markers; clients include session, flags and PID. Capture commands
/// request only tmux's default visible area, never history, environment, or
/// an argv dump. Metadata text replaces control characters with underscores
/// so a writable marker cannot forge another record. Visible text is unchanged.
/// Kernel-stuck I/O, prompt spawn, and exclusive direct-child
/// reaping remain the practical limitations documented by [`run_bounded`].
#[derive(Debug)]
pub struct TmuxDiagnosticsOutcome {
    /// Whether private socket and executable validation authorized any client.
    pub authorization: TmuxDiagnosticsAuthorization,
    /// The fixed metadata commands, in the requested execution order.
    pub metadata: [TmuxDiagnosticCommand; 3],
    /// Visible captures selected from a validated pane-list prefix.
    pub captures: Vec<TmuxDiagnosticCommand>,
    /// Distinct valid panes beyond the four captures, or unknown when output was
    /// truncated, incomplete, or malformed.
    pub omitted_valid_panes: Option<usize>,
}

/// The authorization decision preceding all diagnostic client commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxDiagnosticsAuthorization {
    /// The retained socket directory and resolved executable passed validation.
    Authorized,
    /// The socket was absent or did not meet private namespace requirements.
    Socket(TmuxPeerAcquisition),
    /// The caller did not provide one resolved executable file.
    ExecutableRefused,
    /// The monotonic clock could not represent the aggregate deadline.
    DeadlineUnavailable,
}

/// A fixed metadata label or a bounded pane ID that passed target validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TmuxDiagnosticLabel {
    Sessions,
    Panes,
    Clients,
    VisiblePane(String),
}

/// One diagnostic command's label and either its runner evidence or why it did
/// not start. A nonzero child status stays inside [`CommandRunOutcome`].
#[derive(Debug)]
pub struct TmuxDiagnosticCommand {
    pub label: TmuxDiagnosticLabel,
    pub result: TmuxDiagnosticAttempt,
}

/// Bounded evidence from one command attempt, or an explicit non-attempt.
#[derive(Debug)]
pub enum TmuxDiagnosticAttempt {
    Attempted(CommandRunOutcome),
    NotAttempted(TmuxDiagnosticUnavailable),
}

/// Why a command was not started under the shared diagnostic deadline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxDiagnosticUnavailable {
    NotAuthorized,
    DeadlineExpired,
    InvalidLimits(CommandRunConfigError),
}

/// Collect bounded metadata and visible panes from one private tmux socket.
///
/// The two-second deadline starts before validation and is never refreshed.
/// Each command receives only its remaining slice, including a direct-child
/// cleanup reserve. A failure in one metadata query does not suppress later
/// queries, but captures require a successful pane listing with complete,
/// well-formed retained records. No numeric PID or descendant process is ever
/// used for cleanup.
pub fn snapshot_tmux_diagnostics(socket: &Path, tmux_executable: &Path) -> TmuxDiagnosticsOutcome {
    let Some(deadline) = Instant::now().checked_add(TOTAL_ALLOWANCE) else {
        return unauthorized(TmuxDiagnosticsAuthorization::DeadlineUnavailable);
    };
    snapshot_before(socket, tmux_executable, deadline)
}

/// Keep the entry deadline injectable so tests can prove that all later phases
/// remain unattempted after expiration without racing the scheduler. Production
/// computes this deadline before either path validation step.
fn snapshot_before(
    socket: &Path,
    tmux_executable: &Path,
    deadline: Instant,
) -> TmuxDiagnosticsOutcome {
    let authority = match validate_socket(socket) {
        Ok(authority) => authority,
        Err(reason) => return unauthorized(TmuxDiagnosticsAuthorization::Socket(reason)),
    };
    if validate_executable(tmux_executable).is_none() {
        return unauthorized(TmuxDiagnosticsAuthorization::ExecutableRefused);
    }

    let mut stderr_remaining = STDERR_LIMIT;
    let sessions = run_metadata(
        &authority,
        tmux_executable,
        deadline,
        &mut stderr_remaining,
        MetadataQuery::Sessions,
    );
    let panes = run_metadata(
        &authority,
        tmux_executable,
        deadline,
        &mut stderr_remaining,
        MetadataQuery::Panes,
    );
    let clients = run_metadata(
        &authority,
        tmux_executable,
        deadline,
        &mut stderr_remaining,
        MetadataQuery::Clients,
    );

    let selection = trusted_panes(&panes);
    let mut captures = Vec::with_capacity(selection.ids.len());
    for pane_id in selection.ids {
        captures.push(run_capture(
            &authority,
            tmux_executable,
            deadline,
            &mut stderr_remaining,
            pane_id,
        ));
    }
    TmuxDiagnosticsOutcome {
        authorization: TmuxDiagnosticsAuthorization::Authorized,
        metadata: [sessions, panes, clients],
        captures,
        omitted_valid_panes: selection.omitted,
    }
}

/// Retain every fixed query label even when authority was never established.
/// An empty capture list then means no targets were learned, not no panes exist.
fn unauthorized(authorization: TmuxDiagnosticsAuthorization) -> TmuxDiagnosticsOutcome {
    TmuxDiagnosticsOutcome {
        authorization,
        metadata: [
            TmuxDiagnosticCommand {
                label: TmuxDiagnosticLabel::Sessions,
                result: TmuxDiagnosticAttempt::NotAttempted(
                    TmuxDiagnosticUnavailable::NotAuthorized,
                ),
            },
            TmuxDiagnosticCommand {
                label: TmuxDiagnosticLabel::Panes,
                result: TmuxDiagnosticAttempt::NotAttempted(
                    TmuxDiagnosticUnavailable::NotAuthorized,
                ),
            },
            TmuxDiagnosticCommand {
                label: TmuxDiagnosticLabel::Clients,
                result: TmuxDiagnosticAttempt::NotAttempted(
                    TmuxDiagnosticUnavailable::NotAuthorized,
                ),
            },
        ],
        captures: Vec::new(),
        omitted_valid_panes: None,
    }
}

/// Build and run only one of the fixed metadata commands under the original
/// aggregate deadline. The descriptor keeps labels, options, formats, and
/// output allocations coupled instead of accepting caller-provided argv.
fn run_metadata(
    authority: &SocketAuthority,
    executable: &Path,
    deadline: Instant,
    stderr_remaining: &mut usize,
    query: MetadataQuery,
) -> TmuxDiagnosticCommand {
    let (subcommand, all_panes, format, stdout_limit) = query.command();
    let mut command = tmux_command(authority, executable);
    // env_clear removes locale hints. Without -u, tmux sanitizes US/RS to
    // underscores for a non-UTF8 client, destroying the record boundaries.
    command.args(["-u", subcommand]);
    if all_panes {
        command.arg("-a");
    }
    command.args(["-F", format]);
    run_command(
        command,
        deadline,
        stderr_remaining,
        query.label(),
        stdout_limit,
    )
}

/// Use only an already validated, bounded pane ID as a single target argument.
/// Default capture-pane output is the visible grid; history is never requested.
fn run_capture(
    authority: &SocketAuthority,
    executable: &Path,
    deadline: Instant,
    stderr_remaining: &mut usize,
    pane_id: String,
) -> TmuxDiagnosticCommand {
    let mut command = tmux_command(authority, executable);
    // Match metadata's explicit UTF-8 client mode instead of letting the
    // cleared environment determine how non-ASCII visible text is printed.
    command.args(["-u", "capture-pane", "-p", "-t", &pane_id]);
    run_command(
        command,
        deadline,
        stderr_remaining,
        TmuxDiagnosticLabel::VisiblePane(pane_id),
        CAPTURE_STDOUT_LIMIT,
    )
}

/// Spend the original deadline remainder, including direct-child cleanup, and
/// subtract retained stderr from the snapshot's shared allowance. Discarded or
/// unread bytes remain visible through the runner's loss and completeness fields.
fn run_command(
    mut command: std::process::Command,
    deadline: Instant,
    stderr_remaining: &mut usize,
    label: TmuxDiagnosticLabel,
    stdout_limit: usize,
) -> TmuxDiagnosticCommand {
    let result = match deadline.checked_duration_since(Instant::now()) {
        None => TmuxDiagnosticAttempt::NotAttempted(TmuxDiagnosticUnavailable::DeadlineExpired),
        Some(total) => {
            let reserve = CLEANUP_RESERVE.min(total / 2);
            if reserve.is_zero() {
                TmuxDiagnosticAttempt::NotAttempted(TmuxDiagnosticUnavailable::DeadlineExpired)
            } else {
                let post_exit_read_budget = stdout_limit.saturating_add(*stderr_remaining);
                match CommandRunLimits::new(
                    total,
                    reserve,
                    stdout_limit,
                    *stderr_remaining,
                    post_exit_read_budget,
                ) {
                    Ok(limits) => match run_bounded(&mut command, &limits) {
                        Ok(outcome) => {
                            *stderr_remaining =
                                stderr_remaining.saturating_sub(outcome.stderr.prefix.len());
                            TmuxDiagnosticAttempt::Attempted(outcome)
                        }
                        Err(error) => TmuxDiagnosticAttempt::NotAttempted(
                            TmuxDiagnosticUnavailable::InvalidLimits(error),
                        ),
                    },
                    Err(error) => TmuxDiagnosticAttempt::NotAttempted(
                        TmuxDiagnosticUnavailable::InvalidLimits(error),
                    ),
                }
            }
        }
    };
    TmuxDiagnosticCommand { label, result }
}

/// Bounded capture targets plus an exact overflow count only when the complete
/// listing was observed and every record had the expected shape.
struct PaneSelection {
    ids: Vec<String>,
    omitted: Option<usize>,
}

/// A failed or unreaped listing is evidence but cannot authorize follow-up
/// targets. Successful capped output may still contain usable complete rows.
fn trusted_panes(command: &TmuxDiagnosticCommand) -> PaneSelection {
    let TmuxDiagnosticAttempt::Attempted(outcome) = &command.result else {
        return PaneSelection {
            ids: Vec::new(),
            omitted: None,
        };
    };
    if !command_succeeded(outcome)
        || (!outcome.stdout.complete && outcome.stdout.omitted_bytes == Some(0))
    {
        return PaneSelection {
            ids: Vec::new(),
            omitted: None,
        };
    }
    parse_pane_listing(
        &outcome.stdout.prefix,
        outcome.stdout.complete,
        outcome.stdout.omitted_bytes,
    )
}

/// Require observed child success and intact runner ownership before treating
/// retained listing bytes as the output of a completed inventory request.
fn command_succeeded(outcome: &CommandRunOutcome) -> bool {
    outcome
        .status
        .as_ref()
        .is_some_and(|status| status.success())
        && !outcome.timed_out
        && outcome.errors.is_empty()
        && outcome.direct_child_reaped
        && !outcome.ownership_lost
}

/// Select only complete `RS LF`-framed pane records. A retained prefix that
/// reached its cap still contributes its complete records, but cannot claim an
/// exact omitted total. Any malformed nonempty record has the same effect.
fn parse_pane_listing(prefix: &[u8], complete: bool, omitted_bytes: Option<u64>) -> PaneSelection {
    let mut ids = Vec::with_capacity(MAX_CAPTURED_PANES);
    // Linked windows make list-panes -a repeat pane identities across sessions.
    // Borrow slices from the bounded 8KiB listing to count distinct panes while
    // retaining only four owned target strings. The input bounds this bookkeeping.
    let mut seen = Vec::new();
    let mut malformed = false;
    let mut remaining = prefix;
    while !remaining.is_empty() {
        let Some(separator) = remaining.iter().position(|byte| *byte == RECORD_SEPARATOR) else {
            malformed = true;
            break;
        };
        let record = &remaining[..separator];
        let after_separator = &remaining[separator + 1..];
        let Some(rest) = after_separator.strip_prefix(b"\n") else {
            malformed = true;
            break;
        };
        remaining = rest;
        if record.is_empty() {
            malformed = true;
            continue;
        }
        let mut fields = record.split(|byte| *byte == FIELD_SEPARATOR);
        let Some(first) = fields.next() else {
            malformed = true;
            continue;
        };
        if fields.count() != 9 || !valid_pane_id(first) {
            malformed = true;
            continue;
        }
        if seen.contains(&first) {
            continue;
        }
        seen.push(first);
        if ids.len() < MAX_CAPTURED_PANES {
            // The validator caps both byte length and character set.
            ids.push(String::from_utf8(first.to_vec()).expect("ASCII pane ID"));
        }
    }
    let incomplete = !complete || omitted_bytes != Some(0);
    let omitted = (!incomplete && !malformed).then_some(seen.len().saturating_sub(ids.len()));
    PaneSelection { ids, omitted }
}

/// Accept tmux's numeric pane identity syntax, never session patterns or target
/// expressions. The length ceiling also bounds the owned label allocation.
fn valid_pane_id(value: &[u8]) -> bool {
    value.len() >= 2
        && value.len() <= MAX_PANE_ID_BYTES
        && value[0] == b'%'
        && value[1..].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::super::TmuxPathRefusal;
    use super::*;
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::net::UnixListener;
    use std::process::Command;

    const HELPER_MODE: &str = "FARHELM_DIAGNOSTICS_HELPER_MODE";

    /// Make rows match tmux's format output exactly: it adds LF after the
    /// record separator supplied by the `-F` string.
    fn pane_row(id: &str) -> Vec<u8> {
        let mut row = id.as_bytes().to_vec();
        for _ in 0..9 {
            row.extend([FIELD_SEPARATOR, b'x']);
        }
        row.extend([RECORD_SEPARATOR, b'\n']);
        row
    }

    /// Only whole, strictly shaped records may turn metadata into command
    /// targets; a terminal partial record is evidence, not authority.
    #[test]
    fn pane_prefix_selects_complete_ids_and_marks_partial_total_unknown() {
        let mut bytes = pane_row("%1");
        bytes.extend(pane_row("%2"));
        bytes.extend(b"%3\x1fx");
        let parsed = parse_pane_listing(&bytes, false, Some(3));
        assert_eq!(parsed.ids, vec!["%1".to_owned(), "%2".to_owned()]);
        assert_eq!(parsed.omitted, None);
    }

    /// A complete listing gives an exact omitted count only when every
    /// nonempty record has the shape the capture target contract requires.
    #[test]
    fn malformed_pane_record_prevents_an_exact_omitted_count() {
        let mut bytes = pane_row("%1");
        bytes.extend(b"invalid\x1fx\x1fx\x1fx\x1fx\x1fx\x1fx\x1fx\x1fx\x1fx\x1e\n");
        let parsed = parse_pane_listing(&bytes, true, Some(0));
        assert_eq!(parsed.ids, vec!["%1".to_owned()]);
        assert_eq!(parsed.omitted, None);
    }

    /// Four captures are the fixed work ceiling even if a complete listing
    /// contains more panes than failure evidence can afford to inspect.
    #[test]
    fn complete_pane_listing_reports_the_exact_overflow_count() {
        let bytes = ["%1", "%2", "%3", "%4", "%5"]
            .into_iter()
            .flat_map(pane_row)
            .collect::<Vec<_>>();
        let parsed = parse_pane_listing(&bytes, true, Some(0));
        assert_eq!(
            parsed.ids,
            vec![
                "%1".to_owned(),
                "%2".to_owned(),
                "%3".to_owned(),
                "%4".to_owned(),
            ]
        );
        assert_eq!(parsed.omitted, Some(1));
    }

    /// A window linked into multiple sessions must not consume several capture
    /// slots or inflate the number of panes omitted from the diagnostic snapshot.
    #[test]
    fn linked_panes_are_counted_and_selected_once() {
        let bytes = ["%1", "%1", "%2", "%3", "%2", "%4", "%5", "%5"]
            .into_iter()
            .flat_map(pane_row)
            .collect::<Vec<_>>();
        let parsed = parse_pane_listing(&bytes, true, Some(0));
        assert_eq!(parsed.ids, ["%1", "%2", "%3", "%4"]);
        assert_eq!(parsed.omitted, Some(1));
    }

    /// An empty complete listing is exact, while a prefix ending between RS
    /// and LF cannot authorize that last row. No whitespace is trimmed from IDs.
    #[test]
    fn framing_boundaries_preserve_only_complete_targets() {
        assert_eq!(parse_pane_listing(b"", true, Some(0)).omitted, Some(0));
        let mut bytes = pane_row("%1");
        let second = pane_row("%2");
        bytes.extend_from_slice(&second[..second.len() - 1]);
        let parsed = parse_pane_listing(&bytes, false, Some(1));
        assert_eq!(parsed.ids, ["%1"]);
        assert_eq!(parsed.omitted, None);
        for id in [" %1", "%1 ", "%", "%1:0", "%1;kill-server", "%é"] {
            let parsed = parse_pane_listing(&pane_row(id), true, Some(0));
            assert!(parsed.ids.is_empty(), "unexpected target: {id}");
            assert_eq!(parsed.omitted, None);
        }
    }

    /// A private missing socket is distinct from refused authority and must
    /// not start a client that might create or discover another server.
    #[test]
    fn missing_socket_leaves_every_query_unattempted() {
        let directory = private_directory();
        let result =
            snapshot_tmux_diagnostics(&directory.path().join("missing.sock"), &true_executable());
        assert_eq!(
            result.authorization,
            TmuxDiagnosticsAuthorization::Socket(TmuxPeerAcquisition::SocketAbsent)
        );
        assert!(result.metadata.iter().all(|query| matches!(
            query.result,
            TmuxDiagnosticAttempt::NotAttempted(TmuxDiagnosticUnavailable::NotAuthorized)
        )));
    }

    /// Drive all seven real child invocations through a fake executable that
    /// validates argv and floods each stream. This catches swapped runner limits
    /// and per-command stderr resets that isolated primitive tests cannot see.
    #[test]
    fn snapshot_shares_output_caps_across_all_seven_commands() {
        let directory = private_directory();
        let socket = directory.path().join("tmux.sock");
        let _listener = UnixListener::bind(&socket).expect("private authority socket");
        let executable = directory.path().join("fake-tmux");
        // Builtins only: the runner owns the sole child, with no pipeline or
        // grandchild requiring an independent cleanup fixture. The anchored
        // working directory also gives each command one bounded receipt line.
        std::fs::write(
            &executable,
            r#"#!/bin/sh
test "$1" = -S && test "$2" = tmux.sock && test "$3" = -u || exit 41
shift 3
printf '%s\n' "$1" >> calls || exit 42
case "$1" in
  list-sessions|list-clients) test "$#" = 3 && test "$2" = -F || exit 43 ;;
  list-panes)
    test "$#" = 4 && test "$2" = -a && test "$3" = -F || exit 44
    for id in 1 2 3 4 5; do
      printf '%%%s\037x\037x\037x\037x\037x\037x\037x\037x\037x\036\n' "$id"
    done ;;
  capture-pane)
    test "$#" = 4 && test "$2" = -p && test "$3" = -t || exit 45
    case "$4" in %1|%2|%3|%4) ;; *) exit 46 ;; esac ;;
  *) exit 47 ;;
esac
i=0
while test "$i" -lt 256; do
  printf '%s' 012345678901234567890123456789012345678901234567890123456789012345
  printf '%s' 012345678901234567890123456789012345678901234567890123456789012345 >&2
  i=$((i + 1))
done
"#,
        )
        .expect("write bounded fake diagnostic executable");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .expect("make fixture executable");

        // Scheduling seven finite children is not the deadline test. A separate
        // expired-entry test covers the production deadline wiring deterministically.
        let snapshot = snapshot_before(
            &socket,
            &executable,
            Instant::now() + Duration::from_secs(30),
        );
        let caps = [4096, 8192, 4096, 4096, 4096, 4096, 4096];
        assert_eq!(snapshot.captures.len(), 4, "{snapshot:?}");
        let mut stdout = 0;
        let mut stderr = 0;
        for (query, cap) in snapshot.metadata.iter().chain(&snapshot.captures).zip(caps) {
            let TmuxDiagnosticAttempt::Attempted(result) = &query.result else {
                panic!("finite fake query was not attempted: {query:?}");
            };
            assert!(command_succeeded(result), "{query:?}");
            assert!(result.stdout.prefix.len() <= cap, "{query:?}");
            assert!(result.stdout.omitted_bytes.is_some_and(|count| count > 0));
            stdout += result.stdout.prefix.len();
            stderr += result.stderr.prefix.len();
        }
        assert!(stdout <= 32 * 1024);
        assert!(stderr <= STDERR_LIMIT);
        assert_eq!(snapshot.omitted_valid_panes, None);
        let calls =
            std::fs::read_to_string(directory.path().join("calls")).expect("query receipts");
        assert_eq!(calls.lines().count(), 7);

        let expired = snapshot_before(&socket, &executable, Instant::now());
        assert!(expired.metadata.iter().all(|query| matches!(
            query.result,
            TmuxDiagnosticAttempt::NotAttempted(TmuxDiagnosticUnavailable::DeadlineExpired)
        )));
        assert!(expired.captures.is_empty());
        assert_eq!(
            std::fs::read_to_string(directory.path().join("calls")).unwrap(),
            calls
        );
    }

    /// Refused socket authority prevents every metadata child from starting;
    /// diagnostic collection is observational only after that same gate.
    #[test]
    fn invalid_socket_refuses_before_any_diagnostic_command() {
        let outcome = snapshot_tmux_diagnostics(Path::new("tmux.sock"), &true_executable());
        assert_eq!(
            outcome.authorization,
            TmuxDiagnosticsAuthorization::Socket(TmuxPeerAcquisition::Refused(
                TmuxPathRefusal::Relative
            ))
        );
        assert!(outcome.metadata.iter().all(|command| {
            matches!(
                &command.result,
                TmuxDiagnosticAttempt::NotAttempted(TmuxDiagnosticUnavailable::NotAuthorized)
            )
        }));
    }

    /// An already-expired aggregate deadline never reaches spawn, so skipped
    /// work stays explicit without a scheduler-dependent timeout test.
    #[test]
    fn expired_deadline_leaves_work_unattempted() {
        let deadline = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("representable past deadline");
        let mut stderr_remaining = STDERR_LIMIT;
        let command = Command::new(true_executable());
        let outcome = run_command(
            command,
            deadline,
            &mut stderr_remaining,
            TmuxDiagnosticLabel::Sessions,
            SESSIONS_STDOUT_LIMIT,
        );
        assert!(matches!(
            outcome.result,
            TmuxDiagnosticAttempt::NotAttempted(TmuxDiagnosticUnavailable::DeadlineExpired)
        ));
    }

    /// The diagnostic wrapper keeps each failed or flooding direct child
    /// bounded and reaped, and spends stderr retention from one shared pool.
    #[test]
    fn fake_commands_preserve_bounded_failure_flood_and_timeout_evidence() {
        let mut stderr_remaining = 17;
        let failure = run_fake(
            "failure",
            Instant::now() + Duration::from_secs(5),
            &mut stderr_remaining,
            13,
        );
        let stderr = run_fake(
            "stderr",
            Instant::now() + Duration::from_secs(5),
            &mut stderr_remaining,
            13,
        );
        let flood = run_fake(
            "flood",
            Instant::now() + Duration::from_secs(5),
            &mut stderr_remaining,
            13,
        );
        let timeout = run_fake(
            "timeout",
            Instant::now() + Duration::from_millis(100),
            &mut stderr_remaining,
            13,
        );

        let TmuxDiagnosticAttempt::Attempted(failure) = failure.result else {
            panic!("failure helper was not attempted");
        };
        assert!(failure.status.is_some_and(|status| !status.success()));
        assert!(failure.direct_child_reaped);

        let TmuxDiagnosticAttempt::Attempted(stderr) = stderr.result else {
            panic!("stderr helper was not attempted");
        };
        assert!(failure.stderr.prefix.len() + stderr.stderr.prefix.len() <= 17);
        assert!(stderr.stderr.omitted_bytes.is_some_and(|count| count > 0));

        let TmuxDiagnosticAttempt::Attempted(flood) = flood.result else {
            panic!("flood helper was not attempted");
        };
        assert!(flood.stdout.prefix.len() <= 13);
        assert!(flood.stdout.omitted_bytes.is_some_and(|count| count > 0));
        assert!(flood.direct_child_reaped);

        let TmuxDiagnosticAttempt::Attempted(timeout) = timeout.result else {
            panic!("timeout helper was not attempted");
        };
        assert!(timeout.timed_out);
        assert!(timeout.direct_child_reaped);
        assert_eq!(
            stderr_remaining,
            17 - failure.stderr.prefix.len()
                - stderr.stderr.prefix.len()
                - flood.stderr.prefix.len()
                - timeout.stderr.prefix.len()
        );
    }

    /// Re-execute only the selected fixture mode with child-local configuration;
    /// the parent test's environment and tracing runtime stay untouched.
    fn run_fake(
        mode: &str,
        deadline: Instant,
        stderr_remaining: &mut usize,
        stdout_limit: usize,
    ) -> TmuxDiagnosticCommand {
        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command
            .args([
                "--exact",
                "tmux::diagnostics::tests::fake_diagnostic_helper",
                "--nocapture",
            ])
            .env_clear()
            .env(HELPER_MODE, mode);
        run_command(
            command,
            deadline,
            stderr_remaining,
            TmuxDiagnosticLabel::Sessions,
            stdout_limit,
        )
    }

    /// Child-only fixture modes exercise runner cleanup without changing the
    /// test process environment or borrowing an unbounded cleanup mechanism.
    #[test]
    fn fake_diagnostic_helper() {
        let Some(mode) = std::env::var_os(HELPER_MODE) else {
            return;
        };
        match mode.to_string_lossy().as_ref() {
            "failure" => {
                eprintln!("expected fake diagnostic failure");
                std::process::exit(23);
            }
            "flood" => {
                let mut stdout = std::io::stdout().lock();
                stdout
                    .write_all(&[b'x'; 128])
                    .expect("write fake diagnostic flood");
            }
            "stderr" => {
                let mut stderr = std::io::stderr().lock();
                stderr
                    .write_all(&[b'e'; 128])
                    .expect("write fake diagnostic stderr flood");
            }
            "timeout" => loop {
                std::thread::park();
            },
            other => panic!("unknown fake diagnostic mode: {other}"),
        }
    }

    /// Namespace authorization requires a private parent even when no real
    /// server is involved. Keep every synthetic socket inside that owned directory.
    fn private_directory() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("private diagnostic fixture");
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private fixture mode");
        directory
    }

    /// Refusal tests need a valid executable so only the socket decision is
    /// exercised; canonicalization removes distribution-specific symlink spelling.
    fn true_executable() -> std::path::PathBuf {
        for candidate in [Path::new("/usr/bin/true"), Path::new("/bin/true")] {
            if let Ok(resolved) = std::fs::canonicalize(candidate)
                && validate_executable(&resolved).is_some()
            {
                return resolved;
            }
        }
        panic!("no resolved true executable")
    }
}
