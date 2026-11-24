# No test covers answering a shared automatic and manual SSH sign-in while automatic updates remain enabled

Reviewed commit: 8026bea2ba3bd3d7280cd37dc504660b23fe0487

## TLDR

The automatic host-update sign-in test suite never tests what happens when a user clicks Update or Uninstall on a host
that already has an automatic-update sign-in waiting and answers that shared prompt with automatic updates still turned
on (the default setting), allowing a race between the automatic update resuming and the user's manual Update or
Uninstall to go undetected.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/tests/automatic_sign_in.rs:556`.

Commit 8026bea2ba3bd3d7280cd37dc504660b23fe0487 states "Clicking Update or Uninstall can use the question already
waiting for that host" and adds crates/farhelm-helm/src/provisioning/tests/automatic_sign_in.rs (also at tip
1b3fe49af86979aebc2719965682a17f8c6366ac) to test the production listener, setup SSH relay, and reply routes. However,
every shared automatic+manual sign-in test in automatic_sign_in.rs avoids answering the shared prompt while automatic
updates are still enabled:

1. `opt_out_cancels_only_automatic_checks_and_leaves_the_connection_eligible`
   (crates/farhelm-helm/src/provisioning/tests/automatic_sign_in.rs:556-627) joins an automatic Setup prompt with a
   manual `POST /api/hosts/{host}/update` (lines 590-602), then explicitly disables automatic updates
   (`fixture.enabled(false).await`, line 606) and waits for the listener scan to drop `PendingUpdate` (line 607) BEFORE
   replying with `"fixture-code"` (lines 608-613). Its subsequent
   `assert_eq!(fixture.service.view(fixture.host).await.unwrap().run_id, None)` (lines 623-626) only witnesses the
   opt-out path where the automatic `resume` continuation was already aborted.
2. `racing_preparations_return_the_single_winning_setup_attempt` (lines 631-652) calls `setup.start_automatic(...)`
   directly (without `automatic::scan`'s `resume` task), drops `owner` (line 641), and cancels the question (line 645)
   rather than answering `Ready`.
3. `manual_reply_replaces_a_question_released_after_prepare` (lines 687-711) disables automatic updates before
   activating the manual reply and cancels the replacement question.
4. No test in `automatic_sign_in.rs` exercises manual `POST /api/hosts/{host}/uninstall` sharing an automatic Setup
   sign-in at all.

Because none of the tests answer a shared sign-in with `auto_update_remote_hosts` enabled, the test suite does not
witness the cross-group defect identified by `g39_host_auto_update_2fa_impl` (`l1`): answering a shared sign-in wakes
automatic Update's in-process `resume` continuation (`crates/farhelm-helm/src/provisioning/automatic.rs:323-342`)
concurrently with the user's manual Uninstall or Update replay over the UI channel.

How to verify: Inspect `crates/farhelm-helm/src/provisioning/tests/automatic_sign_in.rs:556-711` and note that
`POST /api/hosts/{host}/uninstall` never appears in the file and every `SshSignInOutcome::Ready` assertion with a manual
caller occurs only after `fixture.enabled(false)`.

Fix: Add tests in `crates/farhelm-helm/src/provisioning/tests/automatic_sign_in.rs` where automatic updates stay enabled
while a manual `POST /api/hosts/{host}/uninstall` and a manual `POST /api/hosts/{host}/update` join an automatic Setup
sign-in, the user answers `"fixture-code"`, and the test verifies the intended settlement behavior (after `g39`'s `l1`
is fixed, that the manual action owns the host without an automatic Update racing ahead of Uninstall or colliding with
manual Update).

Reviewer metadata (g40_host_auto_update_2fa_reconnect_tests/f1): suggested bucket: other (test coverage gap in
maintainer tests; the underlying product race is separately tracked in `g39_host_auto_update_2fa_impl` as `l1`).;
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g40_host_auto_update_2fa_reconnect_tests/f1`.
