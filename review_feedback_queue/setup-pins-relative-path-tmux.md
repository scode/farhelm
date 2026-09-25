# setup can pin a tmux from a relative PATH entry

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Running `farhelm helm setup` from inside an untrusted checkout, with "." in PATH, can make that checkout's tmux file run
every Farhelm session from then on.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F25 / SEC-RELATIVE-PATH-TMUX-PIN`, tagged **possible**. Anchors and title: `crates/farhelm/src/setup.rs:1338`,
`crates/farhelm/src/setup.rs:1339`, `crates/farhelm-supervisor/src/tmux.rs:873-878` — farhelm helm setup can pin a tmux
found through a relative PATH entry such as "." into the boot-time supervisor unit

When no `--tmux` flag or `FARHELM_TMUX` variable names a tmux, setup's `choose_tmux` walks the entries of `PATH` via
`candidates_on_path` (setup.rs:1338) and makes each hit absolute against the directory setup runs in (L1339).
`candidates_on_path` (tmux.rs:873–878) skips only the _empty_ PATH entry; its comment explains that pinning "whatever
directory setup happened to run in" into a systemd unit "is never a defensible thing to pin". But `.`, `./bin` or
`node_modules/.bin` mean the same thing and still pass. If setup is run inside a checkout containing an executable file
named `tmux`, setup runs it once with `-V`. If it prints an acceptable version, setup writes `<cwd>/./tmux` into
`farhelm-supervisor.service` as `FARHELM_TMUX=` and as a `PATH=` entry, then enables the unit.

The one-off execution during setup is something the user's PATH already allowed. The escalation is persistence: that
file now runs as the tmux for every Farhelm agent session, at every boot, which is the outcome the empty-entry filter
was written to prevent. Moving or deleting that checkout later also silently breaks the supervisor unit.

Suggested change: skip every non-absolute PATH entry when searching for tmux, resolve against the working directory only
paths named explicitly with `--tmux`/`FARHELM_TMUX`, and mention a skipped relative candidate in the refusal.

Restater note: requires a relative entry such as `.` in the user's PATH (uncommon) and running setup from inside an
untrusted directory.
