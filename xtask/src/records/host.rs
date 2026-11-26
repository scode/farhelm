//! Which machine a push came from: the `Host: <login>@<tailnet DNS name>` trailer on records commits.
//!
//! Executor slots do not all run on the maintainer's host: some run on VMs on the tailnet, and status tooling has to
//! ssh to wherever a slot runs to read its agent's session transcripts. Rather than keep a separate map of slots to
//! machines, every push names its own machine, so the newest commit by a slot says where that slot runs now.
//!
//! The name is the machine's tailnet DNS name (`Self.DNSName` from `tailscale status --json`, trailing dot dropped),
//! not the OS hostname: Tailscale may add a `-1` suffix on a name clash, the machine may be renamed in the admin
//! console, and the DNS name is what ssh over the tailnet actually reaches. The login is the user running the tool.
//!
//! The trailer is best effort and never fails or delays a push for long. When tailscale is not installed, fails (not
//! logged in, daemon down), takes longer than `LOOKUP_TIMEOUT`, prints something unparsable, or either part is empty or
//! outside the conservative character set below, there is simply no `Host:` trailer, and nothing is printed: a host
//! without tailscale is a normal setup, not a problem to report on every push. Executors push before every queue verb,
//! so a hung tailscale CLI stalling pushes would stall the whole flow; that is what the hard timeout is for.
//!
//! The trailer is information for readers only. Nothing in the tool reads it back: which commits count as a slot's
//! own is decided by the commit author alone (`clone` module), so slots on builds that write no `Host:` and slots
//! that do mix freely.

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The tailscale CLI, found on `PATH` like any other program the tool runs.
pub const TAILSCALE: &str = "tailscale";

/// How long the tailscale lookup may take before it is killed and the trailer omitted. A healthy `tailscale status`
/// answers in tens of milliseconds; this only bounds the damage of a wedged CLI or daemon.
pub const LOOKUP_TIMEOUT: Duration = Duration::from_secs(3);

/// The arguments for the lookup. `--peers=false` leaves out every other machine on the tailnet, which would otherwise
/// make up nearly all of the output; `--self` is the default, spelled out because `Self` is the one field read.
const STATUS_ARGS: &[&str] = &["status", "--self", "--json", "--peers=false"];

/// More output than this is not a status answer for one machine; it is read no further and the trailer omitted.
const MAX_OUTPUT: u64 = 1 << 20;

/// The longest login or DNS name accepted (a DNS name's own limit; no real login comes close).
const MAX_PART_LEN: usize = 253;

/// How often the lookup checks whether tailscale has exited. The lookup is one short-lived child per push.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// A validated `<login>@<dns name>` pair, ready to become the `Host:` trailer.
///
/// Both parts hold only ASCII letters, digits, `.`, `_` and `-`, and neither starts with `-` or `.`. Status tooling
/// passes the value to ssh as its destination, so it must never read as an option (`-oProxyCommand=...`) or carry
/// shell or ssh syntax (`@`, `:`, `/`, spaces, newlines); anything outside that set is dropped rather than escaped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostIdentity {
    login: String,
    dns_name: String,
}

impl HostIdentity {
    /// The pair, or `None` when either part fails the character rules above.
    pub fn new(login: &str, dns_name: &str) -> Option<Self> {
        (is_conservative(login) && is_conservative(dns_name)).then(|| Self {
            login: login.to_string(),
            dns_name: dns_name.to_string(),
        })
    }

    /// The trailer line without its line break: `Host: <login>@<dns name>`.
    pub fn trailer(&self) -> String {
        format!("Host: {}@{}", self.login, self.dns_name)
    }
}

/// Looks up this machine's identity for the `Host:` trailer: `login` (the user running the tool) at the tailnet DNS
/// name `program` reports. `None` in every case the module docs list, and quietly.
///
/// `program` and `timeout` are parameters so tests can point the lookup at a stand-in; the tool passes `TAILSCALE`
/// and `LOOKUP_TIMEOUT`. Without a usable login nothing is run at all.
pub fn detect(login: Option<&str>, program: &OsStr, timeout: Duration) -> Option<HostIdentity> {
    let login = login.filter(|l| is_conservative(l))?;
    let mut command = Command::new(program);
    command
        .args(STATUS_ARGS)
        .stdin(Stdio::null())
        // tailscale explains its failures ("Logged out.", "failed to connect to local tailscaled") on stderr. None of
        // that is the push's business, so it goes nowhere rather than into the agent's terminal.
        .stderr(Stdio::null());
    let output = run_bounded(command, timeout)?;
    HostIdentity::new(login, &tailnet_dns_name(&output)?)
}

/// This machine's tailnet DNS name from `tailscale status --json` output, with the trailing dot of the fully
/// qualified form removed. `None` when the output is not JSON, has no string `Self.DNSName`, or the name is empty.
///
/// The JSON format is documented as subject to change between releases; only `Self.DNSName` is read, and its absence
/// is just another way of having no name.
fn tailnet_dns_name(json: &[u8]) -> Option<String> {
    let status: serde_json::Value = serde_json::from_slice(json).ok()?;
    let name = status.get("Self")?.get("DNSName")?.as_str()?;
    let name = name.strip_suffix('.').unwrap_or(name);
    (!name.is_empty()).then(|| name.to_string())
}

/// Runs `command` and returns its stdout if it exits successfully within `timeout`, killing it otherwise.
///
/// Stdout is drained on its own thread while this one polls for the exit, so a chatty child can never block on a
/// full pipe while we wait. A child that has not exited by the deadline is killed and reaped. Output past
/// `MAX_OUTPUT`, or stdout still held open after the exit (by a grandchild, say), also gives `None`; in that last case
/// the reader thread is left to finish on its own.
fn run_bounded(mut command: Command, timeout: Duration) -> Option<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut child = command.stdout(Stdio::piped()).spawn().ok()?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stdout.take(MAX_OUTPUT + 1).read_to_end(&mut bytes);
        let _ = sender.send(read.map(|_| bytes));
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    let bytes = receiver.recv_timeout(remaining).ok()?.ok()?;
    (status.success() && bytes.len() as u64 <= MAX_OUTPUT).then_some(bytes)
}

/// Whether `part` is non-empty, at most `MAX_PART_LEN` bytes, only ASCII letters, digits, `.`, `_` and `-`, and does
/// not start with `-` or `.` (see `HostIdentity`).
fn is_conservative(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= MAX_PART_LEN
        && !part.starts_with(['-', '.'])
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The DNS name comes from `Self.DNSName` with the trailing dot of its fully qualified form stripped, since status
    /// tooling hands it to ssh as written; a name already without the dot is taken as is. Other fields, including
    /// peers' names, are ignored.
    #[test]
    fn the_dns_name_is_self_dnsname_without_its_trailing_dot() {
        let json = br#"{"BackendState":"Running","Self":{"HostName":"host-a","DNSName":"host-a.example.ts.net."},
            "Peer":{"k":{"DNSName":"other.example.ts.net."}}}"#;
        assert_eq!(
            tailnet_dns_name(json).as_deref(),
            Some("host-a.example.ts.net")
        );
        assert_eq!(
            tailnet_dns_name(br#"{"Self":{"DNSName":"host-a.example.ts.net"}}"#).as_deref(),
            Some("host-a.example.ts.net")
        );
    }

    /// Every shape that holds no usable name gives `None` rather than an error, because a missing trailer is the whole
    /// failure mode: no `Self`, no `DNSName` (MagicDNS off, or a release that renamed it), a non-string value, an empty
    /// name (logged out), and output that is not JSON at all or is cut off.
    #[test]
    fn output_without_a_usable_name_gives_none() {
        for json in [
            &br#"{"BackendState":"NeedsLogin"}"#[..],
            br#"{"Self":{"HostName":"host-a"}}"#,
            br#"{"Self":{"DNSName":42}}"#,
            br#"{"Self":null}"#,
            br#"{"Self":{"DNSName":""}}"#,
            br#"{"Self":{"DNSName":"."}}"#,
            b"Logged out.\n",
            b"",
            br#"{"Self":{"DNSName":"host-a.exam"#,
            b"\xff\xfe",
        ] {
            assert_eq!(
                tailnet_dns_name(json),
                None,
                "{}",
                String::from_utf8_lossy(json)
            );
        }
    }

    /// The trailer is `Host: <login>@<dns name>`, the form status tooling parses and passes to ssh.
    #[test]
    fn the_trailer_is_login_at_dns_name() {
        let host = HostIdentity::new("agent", "host-a.example.ts.net").unwrap();
        assert_eq!(host.trailer(), "Host: agent@host-a.example.ts.net");
        assert!(HostIdentity::new("a_b-c.d", "x-1.example.ts.net").is_some());
    }

    /// Either part outside the conservative set drops the trailer. The value ends up as an ssh destination, so the
    /// cases that matter most are option-like (`-oProxyCommand=...`), ones carrying ssh or shell syntax (`@`, `:`,
    /// `/`, spaces, `;`), and line breaks, which would also forge further trailer lines in the commit message.
    #[test]
    fn parts_outside_the_conservative_set_are_refused() {
        let good = "host-a.example.ts.net";
        for bad in [
            "",
            "-oProxyCommand=x",
            ".hidden",
            "a@b",
            "a:b",
            "a/b",
            "a b",
            "a;b",
            "a\nHost: forged",
            "a\tb",
            "agént",
            &"a".repeat(MAX_PART_LEN + 1),
        ] {
            assert_eq!(HostIdentity::new(bad, good), None, "login {bad:?}");
            assert_eq!(HostIdentity::new("agent", bad), None, "dns name {bad:?}");
        }
        assert!(HostIdentity::new(&"a".repeat(MAX_PART_LEN), good).is_some());
    }

    /// A missing tailscale is the normal case on a host without it: no identity, and no error to report. Without a
    /// usable login, the lookup gives `None` the same way.
    #[test]
    fn a_missing_program_or_login_gives_none() {
        let missing = OsStr::new("/nonexistent/xtask-test/tailscale");
        assert_eq!(detect(Some("agent"), missing, LOOKUP_TIMEOUT), None);
        assert_eq!(detect(None, OsStr::new("true"), LOOKUP_TIMEOUT), None);
        assert_eq!(
            detect(Some("-bad"), OsStr::new("true"), LOOKUP_TIMEOUT),
            None
        );
    }

    /// A child that exits successfully has its whole stdout returned; one that exits non-zero has its output ignored,
    /// since a failing `tailscale status` may still print something JSON-shaped.
    #[test]
    fn bounded_runs_return_stdout_only_on_success() {
        let mut ok = Command::new("printf");
        ok.arg("hello");
        assert_eq!(
            run_bounded(ok, LOOKUP_TIMEOUT).as_deref(),
            Some(&b"hello"[..])
        );
        let mut failing = Command::new("sh");
        failing.args(["-c", "printf '{}'; exit 1"]);
        assert_eq!(run_bounded(failing, LOOKUP_TIMEOUT), None);
    }

    /// A child that outlives the timeout is killed and gives `None` once the timeout passes, not when the child would
    /// have finished: a wedged tailscale must not hold up a push. The child here would run for a minute; the check
    /// allows generous slack over the 200 ms timeout for a loaded machine while staying far below that minute.
    #[test]
    fn bounded_runs_kill_a_child_past_its_timeout() {
        let mut slow = Command::new("sleep");
        slow.arg("60");
        let started = Instant::now();
        assert_eq!(run_bounded(slow, Duration::from_millis(200)), None);
        let elapsed = started.elapsed();
        assert!(
            elapsed >= Duration::from_millis(200) && elapsed < Duration::from_secs(20),
            "{elapsed:?}"
        );
    }

    /// Output past the cap is not a status answer: it is refused rather than parsed, and the child, which may still be
    /// writing, does not keep the call waiting past its timeout.
    #[test]
    fn bounded_runs_refuse_oversized_output() {
        let mut flood = Command::new("head");
        flood.args(["-c", &(MAX_OUTPUT + 10).to_string(), "/dev/zero"]);
        assert_eq!(run_bounded(flood, LOOKUP_TIMEOUT), None);
    }
}
