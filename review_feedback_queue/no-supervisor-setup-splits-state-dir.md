# setup --no-supervisor leaves its supervisor on the old state dir

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After switching an existing setup to `farhelm helm setup --no-supervisor` with a different state directory or binary,
the helm restarts but cannot find its supervisor, and setup reported success.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F7 / COR-NO-SUPERVISOR-SPLIT-STATE`, tagged **possible**. Anchors and title: `crates/farhelm/src/setup.rs:573`,
`crates/farhelm/src/setup.rs:602`, `crates/farhelm/src/setup.rs:630` — setup --no-supervisor leaves setup's own
supervisor unit running on a different state directory from the helm it just pinned

`farhelm helm setup --no-supervisor` is documented as "manage only the helm unit, leaving the supervisor to whoever owns
it here" (a hand-written unit, another service manager, a manual start). With the flag, `install()` plans only the helm
unit (L573, L602), and the ownership check, write-if-changed and restart handling all loop over that plan (L630 onward).
So an existing `farhelm-supervisor.service` is never even read, including one that setup itself wrote on an earlier run.

An operator who first ran a full `farhelm helm setup` and later reruns it with `--no-supervisor` and a different
`--state-dir`, `XDG_STATE_HOME` or binary gets the helm rewritten and restarted against the new state directory, while
setup's own supervisor unit stays enabled and running against the old one. Setup reports success, and the helm then
looks for a supervisor socket in a directory nothing listens in. A restart marker that an earlier failed run left for
the supervisor is also silently kept, because marker handling (setup.rs:693–714) visits only planned units. The existing
test covers only a _foreign_ supervisor unit next to `--no-supervisor`.

This undercuts the SPEC_impl.md "## CLI" promise that one state directory is pinned into both units so the services
cannot split. Suggested change: under `--no-supervisor`, still read `farhelm-supervisor.service` without writing it; if
it carries setup's managed marker, refuse and point to `setup --uninstall` or a full setup, or at least refuse or warn
when its pinned state directory or binary differs from the helm unit being written.

Restater note: this requires the operator to switch an existing full setup to `--no-supervisor` while also changing the
state directory or binary, which is partly operator error. The case for fixing it is that setup can see the conflict and
currently says nothing.
