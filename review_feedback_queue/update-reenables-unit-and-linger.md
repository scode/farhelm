# Every UPDATE re-enables the unit and linger

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After the user turns off start-at-boot for Farhelm or linger on a host, the next Update silently turns them back on.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F11 / COR-UPDATE-REENABLES`, tagged **possible**. Anchors and title: `provisioning/plan.rs:408-427`,
`provisioning/backend.rs:1848-1870`, `provisioning/service.rs:1374-1377` — Every UPDATE re-runs enable --now and
loginctl enable-linger, silently undoing a user's choice

UPDATE plans contain the same `EnableSupervisor` (`systemctl --user enable --now`) and `EnableLinger`
(`loginctl enable-linger`) steps as ADD (plan.rs:408-427; executed at service.rs:1374-1377 and backend.rs:1848-1870).
Linger is the systemd setting that starts a user's services at boot without a login. If the user later ran
`systemctl --user disable farhelm-supervisor` or `loginctl disable-linger`, the next Update — or an "update all" across
the fleet — turns both back on, and UPDATE shows no plan, so nothing tells them. Linger is account-wide: re-enabling it
affects every systemd user unit that account has, not just Farhelm's.

SPEC.md says trust in the helm "does not authorize incidental changes to unrelated host configuration"; an Update click
authorizes replacing the binary, not reverting boot-persistence choices. Suggested change: on UPDATE, leave the existing
enable and linger state alone (restart or try-restart without enabling; skip linger), or only re-assert what
provisioning itself recorded enabling.

User-visible consequence: after the user turns off start-at-boot for Farhelm or linger on a host, the next Update
silently turns them back on.
