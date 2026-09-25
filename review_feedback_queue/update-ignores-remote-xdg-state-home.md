# UPDATE pins ~/.local/state/farhelm, ignoring XDG_STATE_HOME

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a host whose login environment sets XDG_STATE_HOME, Update can restart the supervisor into an empty state directory
the helm then cannot reach, so the host's sessions appear to vanish.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F5 / COR-UPDATE-IGNORES-XDG-STATE`, tagged **possible**. Anchors and title: `provisioning/plan.rs:297-303`,
`provisioning/plan.rs:471-473`, `provisioning/service.rs:676-680`, `provisioning/service.rs:990-1036` — UPDATE with no
recorded state dir pins ~/.local/state/farhelm, ignoring the XDG_STATE_HOME the dial honors

A host row may have no `remote_state_dir`. The helm then dials it with plain `farhelm internal stdio` (no
`--state-dir`), and the remote side picks its default: `$XDG_STATE_HOME/farhelm` if that variable is set in the ssh
session's environment, otherwise `~/.local/state/farhelm` (`default_state_dir_for` in
`crates/farhelm-supervisor/src/lib.rs`).

The UPDATE plan does not follow that rule. With no recorded state dir, `PlanLayout::plan` hard-codes
`<home>/.local/state/farhelm` (plan.rs:297-303), and the reach check reports `HOME` but never `XDG_STATE_HOME`, so it
cannot know better. The new unit therefore runs `supervisor run --state-dir ~/.local/state/farhelm`. At confirmation
(service.rs:990-1036), an answering supervisor is registered with the probe's coordinates, and since the probe had no
state dir either, the row stays `None`.

On a host whose ssh environment sets `XDG_STATE_HOME` (for example via `/etc/environment`), the restart moves the unit's
supervisor onto `~/.local/state/farhelm`: an empty directory, so it mints a new host identity and knows no sessions. The
helm keeps dialing without `--state-dir`, which lands on the XDG path. The attach step times out, every re-plan repeats
the same mistake, and once the old process is gone (at the restart, if it was this unit, or at reboot if it was
hand-started) the directory the helm dials has nothing listening.

A related shape (data-flow reviewer, df7): a row with no `remote_farhelm` whose supervisor was absent at planning but
answers at confirmation. The plan installs to `~/.local/lib/farhelm/farhelm`, but the row records whatever binary the
probe found on PATH, so the registry and the unit it just wrote point at different binaries.

SPEC_impl.md "Provisioning" says UPDATE starts from the recorded coordinates rather than assuming the standard layout;
here it silently builds a second installation. `farhelm helm setup` already honors XDG
(`crates/farhelm/src/setup.rs:567-570`). Suggested change: have the reach check or probe report the remote's resolved
default state directory and use and record that; or, on UPDATE, register the coordinates the plan pins and refuse when
the observation kind changed between planning and confirmation.

User-visible consequence: on a host whose login environment sets XDG_STATE_HOME, Update can restart the supervisor into
an empty state directory the helm then cannot reach, so the host's sessions appear to vanish.
