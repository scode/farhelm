# The sftp transfer's 60 s overall deadline fails slow links deterministically

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Hosts reachable only over slow connections cannot be provisioned at all: the file upload is killed after 60 seconds even
while making progress, every retry fails at the same step, and nothing points at link speed as the cause.

## Details

Source: pre-pr-review-swarm, area provisioning, 2026-09-17. Confidence: possible (correctness-general p1). The mechanism
is certain; the trigger is environmental (link speed) and unreported so far.

`install_source` uploads via `sftp_put`, awaited through `capture_child(child, TRANSFER_TIMEOUT, ...)` with
`TRANSFER_TIMEOUT` a fixed 60 s wall clock (backend.rs:18, 730) — the transfer is killed at 60 s even with bytes
flowing. Retries restart from scratch (`remove_temporary` + fresh `put`), no resume. The download leg for the same tens
of megabytes explicitly rejects this shape: "deliberately NO overall deadline — a release archive is tens of megabytes
and a genuinely slow link must be allowed to finish, since the alternative is an 'add host' that fails at the same point
every time" (payloads.rs:758-760), using an idle read timeout instead. The sftp leg moves those same bytes under the
exact overall deadline the download leg refuses.

Suggested fix: a stall-based deadline (fail only after N seconds with no bytes flowing, mirroring the download path's
idle `read_timeout`), or a substantially raised cap. Either keeps hang protection while letting a slow-but-live transfer
finish.
