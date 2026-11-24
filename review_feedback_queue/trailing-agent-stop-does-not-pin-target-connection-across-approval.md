# Cross-host agent Stop does not pin the target host connection across the approval wait

Reviewed commit: 720d28d77b0638c2e8ab9e09e4baee5a1e73e92b

## TLDR

When an agent on one host asks to stop a session on another host, Farhelm reads the target session's title and program
type (agent, shell, or main program) for the approval card, but after you click Allow it does not check whether that
target host's connection changed while the card waited. If the target host entry was retargeted to another machine or
adopted a different installation during the wait, approving the card can stop a session and close all its terminal tabs
on the replacement host if it shares the same bare session ID, instead of refusing as Delete, Prompt, and Launch
approvals do.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/agent_requests.rs:619`.

In `crates/farhelm-helm/src/agent_requests.rs:619-641` (at `1b3fe49af86979aebc2719965682a17f8c6366ac`; lines 619-640 at
`720d28d77b0638c2e8ab9e09e4baee5a1e73e92b`), `ParsedAgentVerb::Stop` routes the target via
`crate::sessions::route_active_session(&state, &target).await`, capturing `(target_claim, client)` and passing `&client`
to `describe_stop_target` (whose doc comment at `agent_requests.rs:2853` describes it as "the pinned connection"). After
`approved(&state, origin, session_id, target.host == origin.host, action).await` returns `Ok(())`, however, it calls
`crate::sessions::do_stop_session(&state, &target, false, false)` (`crates/farhelm-helm/src/sessions.rs:3168-3182`),
which calls `route_active_session(&state, &target)` afresh and sends `StopSession` without verifying
`client.connection_id()`.

`approved(...)` only verifies `origin_is_live(state, origin)` (the asking host's connection), and host retargeting
(`hosts::retarget`) and adoption (`hosts::adopt`) keep the same `host_id` and `entry_uuid` (`SPEC.md:4627`) without
cancelling waiting approval cards that target that host (`Approvals::deny_session` only cancels cards _from_ a deleted
session). By contrast, `agent_delete` (`agent_requests.rs:2921`, `sessions.rs:4196`), `agent_prompt`
(`agent_requests.rs:2837`), and cross-host launches (`SPEC_impl.md:9625`) all verify the routed target's
`connection_id()` after approval and refuse with `Conflict`/`Unavailable` if the target host connection changed during
the wait.

Note on provenance: `do_stop_session` did not pin `connection_id` prior to `720d28d7` either; `720d28d7` modified these
exact lines in `ParsedAgentVerb::Stop` to retain `client` and read live target metadata from it for the card's wording
without carrying `client.connection_id()` into dispatch.

How to verify: Mirror the existing `agent_delete` reconnection test
(`crates/farhelm-helm/src/agent_requests.rs:8135-8190`) with `AgentVerb::Stop` from `local` targeting `remote`,
kill/replace `remote`'s connection (or retarget/adopt behind the same host entry) while the Stop card waits, click
Allow, and observe that `StopSession` is dispatched to the new connection instead of returning a `Conflict` refusal.

Fix: Pass `Some(client.connection_id())` into `do_stop_session` (or a checked helper matching `delete_session_teardown`
at `crates/farhelm-helm/src/sessions.rs:4196`), refusing with `ErrorKind::Conflict` when the current routed client's
`connection_id()` no longer matches the connection that was routed and described before approval.

Reviewer metadata (g25_approval_card_program_wording/f1): suggested bucket: highest (stopping a session terminates its
entire process tree and all its terminal tabs on a replacement host connection that was not what the destructive
approval card described, losing user-owned running processes; uncertainty to surface: requires a cross-host Stop where
the target host entry is retargeted or replaced/reconnected while the card waits and the replacement host carries a
session with the same bare session ID, e.g. a cloned VM, restored state, or adopted reinstall; also note `SPEC.md:4627`
says retargeting/adopting preserves the entry UUID and "the existing same-entry rules", whereas `SPEC_impl.md`
explicitly pins target `connection_id` for Launch, Delete, and Prompt).; confidence: confirmed (established directly
from source at `agent_requests.rs:619-641`, `sessions.rs:3168-3182`, `approvals.rs:260-320`, and `hosts.rs:917-972`).;
possible cover: none (could be considered alongside pre-existing target-connection pinning for Rename/Restart, or
whether `SPEC.md:4627` "the existing same-entry rules" intentionally omits Stop; Delete and Prompt both pin
`connection_id`)..

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g25_approval_card_program_wording/f1`.
