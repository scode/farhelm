# Discovery misses an install.sh supervisor in ~/.local/bin

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Adding a host where the user already runs Farhelm from the standard installer offers to install a second copy, and
confirming it leaves a crash-looping service and may quietly attach to the old build.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F10 / COR-PROBE-MISSES-LOCAL-BIN`, tagged **definite**. Anchors and title: `provisioning/backend.rs:936-942` —
Discovery reports "absent" when farhelm is not on the non-interactive ssh PATH, missing an install.sh supervisor and
offering to install over it

The probe's remote script (backend.rs:936-942) looks for the binary with `command -v farhelm` and, failing that, only
tries `$HOME/.local/lib/farhelm/farhelm` (the ADD layout). If neither exists it exits 75, which the helm reads as a
positive "no supervisor here". But `install.sh` installs to `~/.local/bin/farhelm`, and on stock Ubuntu `~/.local/bin`
is added to PATH only by the login profile, which a non-interactive `ssh host command` does not read. So a host where
the user installed Farhelm the documented way and runs a supervisor from it probes as absent.

The helm then offers ADD, which installs a second binary at `~/.local/lib/farhelm/farhelm`, writes a unit pointing it at
the same default state directory, and runs `enable --now`. If the running supervisor was started by hand, the new unit's
supervisor cannot take the state-directory lock and crash-loops (the same mechanism as F3), and the attach step either
reconnects to the old supervisor and reports success or times out. SPEC.md Topology says a running supervisor —
explicitly including a hand-started one — is used as-is and never replaced; here the probe misses Farhelm's own install
layout.

Suggested change: also try `$HOME/.local/bin/farhelm`; and before declaring a host empty, look for a live socket in the
default state directory or an existing unit, and ask for the binary path instead of offering an install.

Restater note: the claim covers "by hand or via `farhelm helm setup`", but the two cases end differently. If the running
supervisor is setup's `farhelm-supervisor.service`, ADD's `enable --now` finds that unit already active and does not
start a second copy, so there is no crash loop. Instead the unit file is silently overwritten (F4), losing setup's
marker, the row is repointed at the new binary, attach succeeds through the old running process, and the new binary only
takes over at the next restart or boot. The crash loop applies to hand-started supervisors.

User-visible consequence: adding a host where the user already runs Farhelm from the standard installer offers to
install a second copy, and confirming it leaves a crash-looping service (hand-started case) and may quietly attach to
the old build.
