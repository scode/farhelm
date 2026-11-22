# Alternate-screen Stop diagnostics overstate the replay oracle

Reviewed commit: 06bc10e8e3038b241c01e298aa07121adee0564f

## TLDR

If the Stop replay test fails, it can point a maintainer at a screen capture even when replay selected the wrong
emulator buffer.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/session_lifecycle.rs:3777`.

session_lifecycle.rs:3729-3737 now says the old alternate grid can remain inactive, while :3777-3785 says only a
separate capture could make it visible and that no part of the alternate frame may survive Stop. The assertions only
look for those markers in the replay and check absence of the legacy snapshots directory at :3787-3790.
fake_agent.rs:1811-1816 also says no test asserts on the status row, which session_lifecycle.rs:3782-3785 explicitly
checks for absence. SPEC.md:3286-3291 describes the visible screen left by the program, and SPEC_impl.md:2339-2345 says
replay sends visible rows. Align the diagnostics and comments with the assertions. Bucket: other, because the
discrepancy affects test diagnosis. Confidence: confirmed by source. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/f2`.
