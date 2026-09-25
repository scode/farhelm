# Remote provisioning overwrites an existing supervisor unit unchecked

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Adding or updating a host can silently replace a supervisor unit the user or `farhelm helm setup` wrote there, after
which setup and uninstall on that host refuse to manage it.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F4 / COR-REMOTE-UNIT-OVERWRITE`, tagged **definite**. Anchors and title: `provisioning/plan.rs:408-414`,
`provisioning/service.rs:1357-1372`, `provisioning/backend.rs:580-610`, `provisioning/backend.rs:1785-1812` — Remote
WriteUnit replaces an existing farhelm-supervisor.service with no ownership check

Every plan writes the supervisor unit to `<unit dir>/farhelm-supervisor.service` (plan.rs:408-414), which is the same
file name `farhelm helm setup` uses (`units.rs`, `SUPERVISOR_UNIT_NAME`). The write goes through `install_bytes` →
`install_source` (backend.rs:1785-1812, 580-610): compare hashes, and if they differ, upload a temporary and `mv -f` it
over the destination. At no point does it read the existing file to see whether it carries setup's
`# managed-by: farhelm helm setup` marker, or what its `ExecStart=` runs.

This happens on ADD whenever the probe reported "no supervisor" — which includes a unit that exists but whose supervisor
is stopped or crashed, a supervisor on a different state directory, or an `install.sh` binary that is not on the
non-interactive ssh PATH (F10) — and on every UPDATE, which shows the user no plan at all. On the helm's own machine the
same overwrite is prevented by `local_handoff_reason`, which reads the unit and refuses; remote hosts have no
equivalent.

The damage depends on who wrote the existing file. A unit written by `farhelm helm setup` loses its marker, and setup
and `farhelm uninstall` on that machine then refuse to manage it (units.rs:12-19: setup only touches files that carry
the marker). A hand-written unit loses its own `ExecStart` and state-directory choices. The ADD confirmation says "write
user unit … via temporary file and atomic rename", not "replace your existing unit". This contradicts SPEC.md's "must
not accidentally affect the wrong object", Topology's "a machine has exactly one thing writing its units", and setup's
ownership rule, all of which are honored locally but not remotely.

Suggested change: read the remote unit during inspection, classify it (absent / identical / written by provisioning /
setup-managed / foreign), carry that into the plan, and refuse — or require explicit confirmation — before replacing
anything provisioning did not render.

User-visible consequence: adding or updating a host can silently replace a supervisor unit the user or
`farhelm helm setup` wrote there, after which setup and uninstall on that host refuse to manage it.
