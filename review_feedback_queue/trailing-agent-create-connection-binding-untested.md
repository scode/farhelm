# No test covers agent create's binding of the launch to the approved host connection

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The fix that stops an agent-requested new session from starting on a host connection other than the one the user
approved has no test. A later change could silently drop it, and approved agent launches could again land on a replaced
connection (for example a host entry that now reaches a different machine) without any test failing.

## Details

Concerns 5554fecaa2. Main reference `crates/farhelm-helm/src/agent_requests.rs:2211` (`create_for_agent` passes
`expected_incarnation: composition_incarnation.or(approval.target_incarnation)`); `approve_launch` returns it at
`:2807-2810`.

Before this commit a no-trust agent create carried `composition_incarnation = None` to `dispatch_agent_create`, so
`incarnation_holds(&claim, None)` (`crates/farhelm-helm/src/precondition.rs:106-109`) accepted whatever connection
served the host at dispatch. `approve_launch` rechecks the target incarnation right after the card
(`agent_requests.rs:2792-2799`) but then awaits `yolo_guard::check_agent_approved` (`:2800-2807`), which awaits the
settings read in `check_agent_wrapper` (`crates/farhelm-helm/src/yolo_guard.rs:173-180`), before returning;
`dispatch_agent_create` re-reads `host_client` afterwards (`agent_requests.rs:3093`). The commit closes that window by
returning `target_before` and using it as `expected_incarnation`.

The only test changes in this commit rename the `composition_incarnation` field in two existing tests
(`agent_requests.rs:9253` uses None; `:11445` passes `Some(claim.incarnation)` directly into `dispatch_agent_create`).
The `:11445` test exercises `dispatch_agent_create` with a caller-supplied incarnation (the pre-existing trusted-path
case) and never goes through `create_for_agent`. A search of `crates/farhelm-helm/src`, `crates/farhelm-helm/tests` and
`crates/farhelm/tests` finds `target_incarnation` only at `agent_requests.rs:2211`, `:2695` and `:2808`, and
`create_for_agent` is called only from the verb handler (`:905`); the reconnect tests at `:7340`, `:8193`, `:9275` and
`:11423` cover prompt, delete, template-host identity and the composition path. So replacing
`.or(approval.target_incarnation)` with plain `composition_incarnation`, or making `approve_launch` return None, would
pass the suite. The Stop half of the same commit did get a test
(`stop_session_happy_path_returns_200_with_empty_object_body` in `sessions_tests.rs` calls `do_stop_session` with a
mismatched connection id).

Fix: add a test that drives `create_for_agent` with a no-trust command launch, replaces the target connection after
`approve_launch`'s recheck but before dispatch (a seam around `check_agent_approved`, since the window is a single
await), and asserts `precondition::is_stale` on the refusal with no CreateSession reaching either connection; or at
minimum a unit test that `create_for_agent` forwards `approval.target_incarnation` when `composition_incarnation` is
None.

Bucket: other (a test gap; the code at the reviewed commit binds correctly, and the consequence is regression risk for a
security-relevant binding). Not highest: no user-visible defect exists at the reviewed commit.

Confidence: confirmed by a source search for tests reaching the new path.

Possible cover: none known. The agent approvals paragraph of SPEC_impl.md, edited by this commit ("The helm keeps that
connection through the final launch checks and dispatch"), states the contract the missing test would pin.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g13 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
