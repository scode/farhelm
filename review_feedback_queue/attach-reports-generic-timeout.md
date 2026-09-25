# Attach turns version/identity refusals into a generic timeout

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

When a freshly provisioned supervisor is refused for a version or identity problem, the user waits 30 seconds and then
sees only "timed out", with no hint of the real cause.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F17 / COR-ATTACH-GENERIC-TIMEOUT`, tagged **possible**. Anchors and title: `provisioning/service.rs:1400-1421` — The
attach step turns a specific refusal into a generic "timed out"

The attach step (service.rs:1400-1421) polls the connection manager's view of the host every 20 ms and succeeds only
when the host is `Connected` with a new incarnation; it exits early only if the host's actor disappears. The manager has
several states that mean "connected, but refused for a reason the user must act on": `VersionSkew` (the supervisor
speaks another protocol), `IdentityMismatch` (it reports an identity other than the recorded one), `Duplicate` (another
registry row already holds its identity), and `IdentityUnverified`. None of these become `Connected` on their own, but
the loop does not recognise them. It waits out the full 30 s `ATTACH_TIMEOUT` and records only "waiting for the
provisioned supervisor: timed out".

Realistic triggers: a `--payload-dir` holding a different release (the new supervisor is protocol-skewed); re-adding a
known host whose state directory was wiped (new identity); an installed supervisor whose identity another row already
claims. The recorded error hides a precise, actionable cause behind one that suggests a slow or unreachable host, and
the run holds the host lock and a fleet run slot 30 s longer than needed. Suggested change: match those states in the
poll loop and fail immediately with the state and its remedy (for example "the provisioned supervisor reports build X,
which speaks another protocol", or "the host now reports identity Y; adopt it or fix the destination").

User-visible consequence: when a freshly provisioned supervisor is refused for a version or identity problem, the user
waits 30 seconds and then sees only "timed out", with no hint of the real cause.
