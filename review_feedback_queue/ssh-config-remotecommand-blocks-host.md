# A host whose ssh config sets RemoteCommand can never be reached

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

Some people put a `RemoteCommand` in their ssh config so that `ssh devbox` drops them straight into tmux. The usual form
is `RemoteCommand tmux new -A -s main` with `RequestTTY yes`. If they register that same alias as a Farhelm host, every
connection to it fails immediately. Adding the host, probing it, setting it up, updating it and the ongoing supervisor
connection all fail. The hosts panel shows the generic text: "the ssh channel closed before the handshake completed:
either no supervisor is running there … or the ssh connection itself failed". The real cause, ssh's "Cannot execute
command-line and remote command.", appears only in the helm's log.

## Details

Source: whole-codebase review, 2026-09-30, slice helm-client.

Reviewer's confidence: confirmed (ssh argv traced; OpenSSH 9.6 behavior reproduced locally against an unresolvable
host).

Reviewer's bucket suggestion: other.

Possible cover for triage to check: SPEC_impl.md, ssh transport motivation ("only the real ssh binary honors
~/.ssh/config fully (ProxyJump, Match blocks, agent forwarding, ControlMaster)"). Unsure because that passage is about
honoring the user's connection settings, not about letting a session-level `RemoteCommand` override the command Farhelm
itself has to run. Nothing in SPEC.md's "Supported user environments" or "Supported host setup" names ssh config.
TODO.md, the queue and TRIAGE_OUTCOMES.md have nothing on it.

Every ssh invocation Farhelm makes starts from `ssh_base_args` (`crates/farhelm-helm/src/ssh.rs:63-125`): `BatchMode`,
the ControlMaster options, `--`, then the destination. Two callers add their own remote command after that prefix:

- the steady-state proxy, `ssh_stdio_args` (ssh.rs:139-156), used by `crates/farhelm-helm/src/transport.rs:113-119`;
- provisioning's `ssh_command` (`crates/farhelm-helm/src/provisioning/backend.rs:448-462`), used for probe, reach
  checks, convergence and payload uploads.

Nothing in the prefix cancels a `RemoteCommand` from the user's ssh config. OpenSSH treats a command-line command plus a
configured `RemoteCommand` as fatal before it connects:

```
$ printf 'Host *\n  RemoteCommand tmux new -A -s main\n  RequestTTY yes\n' > cfg
$ ssh -F cfg -o BatchMode=yes -- nonexistent.invalid echo hi
Cannot execute command-line and remote command.
$ ssh -F cfg -o BatchMode=yes -o RemoteCommand=none -- nonexistent.invalid echo hi
Pseudo-terminal will not be allocated because stdin is not a terminal.
ssh: Could not resolve hostname nonexistent.invalid: Name or service not known
```

This was reproduced with OpenSSH_9.6p1. `.invalid` never resolves, so no connection was made. The first run fails before
name resolution. The second gets past the RemoteCommand check and fails at DNS as expected.

Because ssh exits having written nothing, the steady-state connection ends as `ClosedBeforeHello`.
`annotate_ssh_handshake_eof` (ssh.rs:253-278) then adds its "no supervisor is running … start one there with
`farhelm supervisor run`" suggestion, which is wrong for this cause. The actual reason reaches only the relayed stderr
in the helm log. The connection manager keeps retrying and fails the same way every time.

The same run shows `RequestTTY yes` is harmless, because the helm's ssh stdin is a pipe. A config with
`RequestTTY force` would make ssh put a pty around the binary framing protocol. The following was NOT verified against a
live host: that likely corrupts frames (newline translation and echo). A forced tty also turns on ssh's `~.` escape
character, so terminal input containing a newline followed by `~.` could drop the whole host connection.

Fix:

- Add `-o RemoteCommand=none` to the `ssh_base_args` prefix. Command-line options take precedence over the user's
  config, as the second run shows.
- Consider also adding `-T` (or `-o RequestTTY=no`) so a forced tty cannot wrap the protocol stream.
- Both belong in the single security-boundary prefix, so the stdio proxy and every provisioning command get them.
- Update the argument-pinning tests in ssh.rs to match.

Neither option touches the settings the transport promises to honor: keys, agent, ProxyJump, Match and ControlMaster.

Related: `ssh-forwarding-inherited.md` (from a separate review) fixes the same ssh argument prefix by adding further
`-o` options and updates the same argv-pinning tests in `ssh.rs`; the two fixes are best landed together.
