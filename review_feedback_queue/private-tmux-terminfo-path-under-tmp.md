# Shipped Linux tmux looks up terminal definitions in the release build's deleted /tmp directory

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

The tmux that Farhelm installs on Linux hosts whose own tmux is too old has the release build machine's throwaway
temporary directory compiled in as the only system place it looks for terminal descriptions. On the user's host that
path sits under the shared, world-writable `/tmp` and normally does not exist, so any other account on a shared host can
create it and supply the terminal descriptions this tmux loads when it runs in a real terminal. Farhelm's own supervisor
never runs it that way, so this matters only if the user (or a program in a session) runs Farhelm's tmux by hand. Even
then, harm beyond a broken or garbled attach depends on a flaw in the terminal-description parser it links. Without any
attacker, such a hand-run attach fails outright because the system's terminal descriptions are never searched.

## Details

Found by the 2026-10-22 prioritized persistent review, group `212-build-tmux-scripts` (finding f1).

Mechanism:

- `scripts/build-private-tmux.sh:18` creates the work directory as `mktemp -d "${TMPDIR:-/tmp}/farhelm-tmux.XXXXXX"`,
  and `:78` sets `prefix="$work/prefix"`. The `EXIT` trap at `:19` deletes the directory after the build.
- For the Linux musl targets (`:89-97`), `ncurses_configure` stays empty, so ncurses is configured (`:129`) with
  `--prefix="$prefix"` and no `--with-default-terminfo-dir` or `--with-terminfo-dirs`. ncurses' configure then defaults
  both to `${datadir}/terminfo`, that is `<work>/prefix/share/terminfo`, and compiles that path into the static archives
  the static tmux links. So the path ends up inside the published `tmux-x86_64-unknown-linux-musl` and
  `tmux-aarch64-unknown-linux-musl` binaries.
- Only the macOS branch (`:110-117`) points ncurses at `/usr/share/terminfo`, with a comment showing the concern was
  understood there. No caller ships a darwin tmux at this commit, per the reviewer.
- Result on a provisioned Linux host: apart from `$TERMINFO`, `~/.terminfo` and `$TERMINFO_DIRS`, the only system
  directory this tmux searches is `/tmp/farhelm-tmux.<6 chars>/prefix/share/terminfo`, assuming `TMPDIR` was unset on
  the release runner (nothing in `.github` or `dist-workspace.toml` sets it, per the reviewer). That path is fixed per
  release and readable from the published binary with `strings`, and any other local account can `mkdir` it in `/tmp`
  and fill it.
- When the read happens: the tmux client reads terminfo for `$TERM` when its stdin is a terminal and it is not a control
  client. The supervisor nulls or pipes stdin on every tmux it spawns
  (`crates/farhelm-supervisor/src/tmux.rs:922, 2573, 2625, 2681, 2765, 2885, 3973, 4466, 6843`; `tmux/stream.rs`,
  `sink.rs` and `input.rs` use pipes). So supported operation never reads these files. The trigger is a person or
  program running the provisioned tmux binary in a real terminal, for example to attach to the private socket and look
  at a session.
- Consequence when triggered: tmux loads capability strings another account controls and uses them to drive the user's
  terminal. If ncurses' compiled-terminfo reader has a memory-safety bug (there have been some, for example
  CVE-2023-29491), that account gets code execution as the user. Without such a bug the impact is a broken or garbled
  attach.

How to verify: on a published release, run `strings tmux-x86_64-unknown-linux-musl | grep -n terminfo` and check that
the only absolute directory is a `/tmp/farhelm-tmux.*/prefix/share/terminfo` path. Or run
`TERM=xterm-256color ./tmux-x86_64-unknown-linux-musl -S /tmp/x.sock new` on a host without `~/.terminfo` and watch it
fail with "missing or unsuitable terminal".

Fix sketch: give the Linux ncurses build an explicit, fixed system search list, such as
`--with-default-terminfo-dir=/usr/share/terminfo --with-terminfo-dirs=/etc/terminfo:/lib/terminfo:/usr/share/terminfo`
(plus `--disable-db-install` if the database is not needed), as the macOS branch already does. Optionally add a release
check that the published binary contains no `/tmp` path.

Suggested bucket: the reviewer said high, possibly highest: a cross-account trust-boundary issue on a shared host, but
reachable only through direct use of the private tmux plus an ncurses parser flaw. Recorded under highest because it is
a trust-boundary consequence; triage settles the bucket.

Confidence: likely. Unverified premises: that ncurses' configure default places the terminfo search list under
`--prefix` (standard ncurses behavior, not checked against a built binary); that `TMPDIR` on the release runner is
`/tmp` or another world-writable directory; and that tmux 3.7c still reads terminfo only when stdin is a terminal and
the client is not a control client.

Reviewer's proposed drop, kept for triage: the triggering use may be covered by SPEC_impl.md "Terminal substrate:
private tmux server" (around 1405): "The private server is an implementation detail. Direct interaction with it, by the
user or by a program running in a session, is unsupported". Recording did not treat that as a cover: the sentence goes
on to say Farhelm "does not track, reap, or restore windows, panes, or configuration created or changed that way", which
is about object tracking, not about letting another account's files reach the user's process. SPEC.md "Client hardening"
declines defense in depth against hypothetical flaws, but only for the browser UI, so it does not clearly apply.

Possible cover: SPEC_impl.md "Terminal substrate: private tmux server" (direct interaction unsupported). SPEC.md "Local
authority and trust between hosts" makes the Unix account the boundary, so other local accounts are not trusted; that
argues against a cover. No FILTER.md filter matches.
