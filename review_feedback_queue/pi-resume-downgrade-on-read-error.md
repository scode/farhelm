# Pi resume check destroys a valid resume locator on transient read errors

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A user with a healthy Pi session can permanently lose the ability to resume its conversation because of one transient
hiccup — a permission blip, a disk error, a file read mid-write — during a restart. From then on the session only offers
starting over, even though the conversation data on disk is fine.

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite. Four lenses agreed
independently (correctness-state-lifecycle p1, correctness-general p1, correctness-systems p1, correctness-data-flow
p2). Coordinator confirmed the code and the callee contract against the tree.

For Pi sessions the supervisor stores a resume locator naming the conversation and the exact session file proving it.
Before a Pi resume launches, `verify_pi_resume` re-reads that file
(`crates/farhelm-supervisor/src/service/core.rs:5450`):

```rust
let verified = match crate::agent_kind::read_record(Path::new(path), integration).await {
    Ok(Some((record, _))) => record.conversation == locator.session_id,
    Ok(None) | Err(_) => false,
};
```

Any non-verified result durably overwrites the stored locator with a fileless token
(`replace_reported_conversation_if_current`, core.rs:5458-5471) and refuses the restart with `Conflict`. A fileless
token permanently downgrades the offer from `Resume` to `FreshOnly`. The loss is unrecoverable for Pi: it is a
"report-only integration" (core.rs:5449) whose hook never re-fires (core.rs:10339-10341), and reload never re-scans
`Reported` rows (core.rs:5171-5174). Note the check runs before the old agent is stopped (verify at core.rs:7721, stop
at core.rs:7800), so a torn read of a file the live agent is appending to is squarely in play.

This contradicts `read_record`'s own documented contract
(`crates/farhelm-supervisor/src/agent_kind/capture.rs:551-557`): `Ok(None)` means gone-or-unrecognized, while "every
other failure is an `Err` … because the caller's policy (retain the already-captured identity, and say so)". The refusal
message itself admits the file only "could not be verified".

Suggested fix: split the `Err` arm from the verified-absent arms. On `Err`, refuse this restart with `Conflict`
(retryable once the cause clears) without touching the row, and include the read error in the refusal; only `Ok(None)`
and conversation mismatch should invalidate the stored locator. Refusing the restart on an unreadable file is correct
fail-closed behavior and stays.
