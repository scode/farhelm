# Opening a tab puts the session token on tmux's argv

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

None today; the session token is briefly visible to other local accounts.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F21 / COR-TAB-TOKEN-ARGV`, tagged **possible**. Anchors and title: `service/core.rs:11419-11423`, `tmux.rs:2712-2715` —
Opening a tab puts the session token on the tmux client's command line

Every session has a per-session bearer token, `FARHELM_SESSION_TOKEN`. A process inside the session presents it on the
supervisor's unix socket to prove which session is asking, for example for a restricted spawn. `tab_environment`
(core.rs:11419-11423) adds the token and the socket path to the tab's environment list, and `new_window`
(tmux.rs:2712-2715) turns each entry into a `-e NAME=value` argument on the `tmux new-window` command line. While that
short-lived tmux client runs, the token is in its argv, which any local account can read with `ps` or
`/proc/<pid>/cmdline` unless procfs is mounted with `hidepid`. `redacted_tmux_args` hides it only in error messages. The
agent path handles the same credential differently on purpose: it hands the token over in a 0600 launch spec file.
SPEC_impl.md, and the `InputClient` docs, treat a world-readable command line as a real disclosure channel; that concern
is why input moved off spawned `send-keys` argv.

The open premise is whether another local account could use the token. Today it probably cannot, because the supervisor
socket is 0600 inside a 0700 state directory. So this is a defence-in-depth and consistency gap, not a working exploit:
a credential briefly leaves the Unix-account boundary that SPEC.md draws. The suggested change is to keep secrets off
tmux's argv, either by sending the `new-window` command over an existing control client's stdin (as input already is),
or by launching tabs through a small shim that reads the token from a 0600 file and deletes it.

## Security

(All security-lens findings in this swarm were merged into correctness-lens findings at the same location: F1–F3 and F21
carry the security reviewers' provenance.)
