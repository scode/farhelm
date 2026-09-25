# A create replay can fail with a Codex offer Conflict

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Rarely, a create that worked is reported as failed with a confusing message.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F9 / COR-CREATE-REPLAY-CONFLICT`, tagged **possible**. Anchors and title: `service/core.rs:8127-8131` — A create replay
can fail with an unrelated "Codex restart offer changed" Conflict

Creates carry an idempotency key (the "intent key"). If a client resends a create whose session already exists, for
example because the first reply was lost, the supervisor "replays" the original answer from the stored row in
`replay_created_session`. For Codex and Grok sessions it first calls `refresh_reported_capture` (core.rs:6085). That
re-reads the row under the session's capture lock and re-checks whether the conversation the agent reported can still be
resumed.

That refresh returns `false` in three cases: the row's generation changed between the two reads (a restart happened),
the row disappeared, or a compare-and-swap lost to a newly arriving conversation report. On `false`, the create replay
returns `Conflict` with "the Codex restart offer changed; refresh the session" (core.rs:8127-8131), even for Grok. For a
create, `Conflict` otherwise means "this key is used up": the session was deleted, or the key was reused for a different
request. A create that did succeed is therefore reported as a conflict whose message has nothing to do with creating
anything. The race is harmless in itself; only a restart-offer check lost it.

The open premise was whether a client treats this `Conflict` as final and drops the key, so that a later submission
creates a duplicate. In the web UI's create form (`crates/farhelm-ui/src/list/create_form.rs`, around 3505-3520) the key
deliberately survives a failure unless the helm's incarnation marker is present. Resubmitting the unchanged form replays
against the same key and should succeed once the race is over. A duplicate therefore needs the user to edit or reset the
form after the confusing error, which mints a new key. The fix is to retry the refresh, or to build the reply from the
re-read row without the readiness check, when the caller is a create replay. `Conflict` stays for the restart path,
where it means something. What the user sees: rarely, retrying a create that actually worked shows a confusing
Codex-worded conflict.

Restater note: The duplicate-session consequence looks weaker than the merged text implies. The web create form keeps
the intent key across a 409 without the incarnation marker, and the session also appears in the list. A duplicate needs
a user action after the error. I did not check other clients, such as the CLI or agent-issued creates.
