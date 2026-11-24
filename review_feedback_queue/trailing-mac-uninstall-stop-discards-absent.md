# Mac uninstall stop step discards `absent` output and never reports `Skipped` on retries

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When retrying a remote Mac uninstall after the supervisor job and its terminal hosts are already gone (for example after
a later directory-removal step failed), the stop step is reported as completed instead of unchanged/skipped, unlike
Linux uninstall.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/backend.rs:3239`.

`SPEC_impl.md`, "Provisioning" (lines 8187-8188) states that for uninstall "each host command treats work already done
as a skip, so a failed run continues from wherever it stopped". `macos::stop_script` in
`crates/farhelm-helm/src/provisioning/macos.rs:170,232` carefully tracks `changed=no` and prints `absent` to stdout when
neither the launchd job nor the `hosts` directory existed, and `macos.rs:686,1007` unit-test that stdout contract.
However, `SystemBackend::stop_launch_agent` in `crates/farhelm-helm/src/provisioning/backend.rs:3239-3246` discards the
`CommandResult` returned by `require_shell_within` and unconditionally returns `Ok(ActionOutcome::Completed)`. By
contrast, Linux `SystemBackend::stop` at `crates/farhelm-helm/src/provisioning/backend.rs:3548-3557` checks
`if output.stdout == b"absent"` and returns
`ActionOutcome::Skipped("the supervisor and its terminal hosts were already gone".to_string())`. Verify by comparing
`backend.rs:3239-3246` against `backend.rs:3548-3557` and `macos.rs:170,232`. Fix: capture
`let output = self.require_shell_within(...).await?;` in `SystemBackend::stop_launch_agent` and return
`ActionOutcome::Skipped("the supervisor and its terminal hosts were already gone".to_string())` when
`output.stdout == b"absent"`.

Reviewer metadata (g01_service_stop_grace/f2): suggested bucket: other — Purely a step-status reporting discrepancy
(green Completed instead of grey Skipped on a partial-uninstall retry); no functional failure, security issue, or data
loss.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g01_service_stop_grace/f2`.
