# The probe misses a supervisor whose farhelm is in a custom install directory

Reviewed commit: 7cc06814956a1e9b6ec41f29e2e57ec57b5d2178

## TLDR

Adding a host whose running supervisor comes from a farhelm installed in a non-standard directory (not on the
non-interactive ssh PATH) probes as "no supervisor", and the helm offers to install a second copy over it.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Narrowed from the original finding (pre-pr-review-swarm run `20260925-0602-2b597e9-f96c`,
`F10 /
COR-PROBE-MISSES-LOCAL-BIN`). The ssh probe script (`provisioning/backend.rs`, `SystemBackend::probe_script`) now
also tries `$HOME/.local/bin/farhelm`, the install script's default, after `command -v farhelm` and the ADD layout
`$HOME/.local/lib/farhelm/farhelm`. That covers the documented install.

What remains is an install elsewhere, for example `FARHELM_INSTALL_DIR` set to a custom directory, with a supervisor
running from it. The probe still exits with positive absence (75), and the helm offers ADD, which installs a second
binary and a unit on the same default state directory. For a hand-started supervisor the new unit then crash-loops on
the state-directory lock; for setup's unit the unit file is overwritten. SPEC.md Topology says a running supervisor is
used as-is.

The original suggestion for this part: before declaring a host empty, look for a live supervisor socket in the default
state directory or an existing `farhelm-supervisor.service` unit, and ask for the binary path instead of offering an
install. That is a new detection step in the probe, not a narrow path addition.
