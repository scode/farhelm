# Mac fixture cleanup uses the shorter shell-command budget

Reviewed commit: c3bee0e586b954f24a12e665a961eb9e96bf50ca

## TLDR

The native Mac test's cleanup can time out while a terminal-host stop is still within the time production allows it.
Cleanup then remains unconfirmed and can retain owned fixture files. No actual timeout or harm to an operator's
installation was observed.

## Details

`crates/farhelm-helm/src/provisioning/tests/macos_real_tests.rs:318` constructs the production Mac stop script followed
by removal of the owned plist and library. At line 334 it runs that combined script through the generic `require_shell`
operation. `crates/farhelm-helm/src/provisioning/backend.rs:18` gives that operation 30 seconds, applied to the entire
SSH command at line 909.

Production deliberately gives the same stop script more time: `stop_launch_agent` at `backend.rs:2971` uses
`STOP_SUPERVISOR_TIMEOUT`, defined at line 26 as the generic command budget plus the terminal-host command's own budget
and ten seconds. The surrounding comment explains why startup, host stopping and surrounding commands need separate
allowance. `SPEC_impl.md`, "Terminal host", likewise gives the host-stop command its own bounded execution/reporting
period. The fixture's shorter outer deadline can cut off that permitted work.

Both lifecycle and reboot tests use this cleanup, at `macos_real_tests.rs:780` and line 1164. Use the production
timeout-aware stop operation after the fixture's ownership check, then remove its files, or give the combined cleanup
script the same extended allowance. A local SSH timeout does not establish whether remote termination finished.

Priority: `other`. Confidence: confirmed timeout mismatch from source inspection; no runtime occurrence established.
Proposed drop: none. Possible cover: no exact existing coverage found. The rare safe-retry filter is not clearly
applicable: this is failure cleanup with retained owned state, without an established automatic retry that completes the
cleanup. This is separate from the test's missing live-host premise and does not establish a production stop bug.

Provenance: trailing review `9e3ab60b86025ca74948ededd09b99cd81a2477c..25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`, originating record `mac-tests/mac-tests-006`. Other Mac reviewers corroborated
the source distinction; their interface records preserve the same limited consequence.
