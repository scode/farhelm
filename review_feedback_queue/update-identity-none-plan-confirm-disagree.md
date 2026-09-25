# UPDATE plan and confirm disagree on a no-identity supervisor

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In the rare case that a host's supervisor reports no identity, Update fails every time with "plan again", and planning
again never helps.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F18 / COR-IDENTITY-NONE-DISAGREE`, tagged **definite**. Anchors and title: `provisioning/service.rs:666-675`,
`provisioning/service.rs:997-1004` — UPDATE planning accepts a supervisor reporting no identity, but confirmation always
refuses it

Each supervisor reports a _host identity_ in its hello; the helm records it per row to notice when a destination starts
pointing at a different installation. UPDATE checks it twice, with two different rules. At planning (service.rs:666-675)
it refuses only when both the recorded and the reported identity are present and differ, so a supervisor reporting no
identity (`None`) passes. At confirmation (service.rs:997-1004) it refuses whenever an identity was expected and the
report is not equal to it, so the same `None` against a recorded `Some(x)` is refused with "the supervisor identity
changed after UPDATE planning … plan again". Planning again passes and confirmation refuses again, so the loop never
converges. The fix is to use one predicate in both places — either treat a `None` report as a mismatch at planning, or
accept it at confirmation the way planning does.

Restater note: the rule mismatch is real, but I could not find a way for a current Farhelm supervisor to trigger it. The
protocol allows `host_identity: None` for a "claimless" supervisor (one constructed while another process holds the
state-directory lock), but such a process never serves: `serve()` refuses without the lock
(`crates/farhelm-supervisor/src/service/core.rs`, around lines 5048-5051 and 6383-6392), and a supervisor that holds the
lock always mints or reads a durable identity. The probe only ever reaches the serving supervisor through its socket.
Treat this as a consistency cleanup with no known production trigger rather than a user-facing bug.

User-visible consequence: in the rare (possibly unreachable) case that a host's supervisor reports no identity, Update
fails every time with "plan again", and planning again never helps.
