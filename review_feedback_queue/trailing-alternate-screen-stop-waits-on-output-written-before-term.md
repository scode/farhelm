# Alternate-screen Stop waits on output written before the TERM handler exists

Reviewed commit: 06bc10e8e3038b241c01e298aa07121adee0564f

## TLDR

The test can fail even when Farhelm correctly keeps the frame of a program killed before it restored the primary screen.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/session_lifecycle.rs:3761`.

session_lifecycle.rs:3754-3765 waits for ALT-SCREEN APP, detaches, and calls Stop. fake_agent.rs:1817-1846 writes that
line and the status row before installing its SIGTERM handler. Its own comment names FAKE-AGENT READY as the
post-registration barrier. The intervening detach does not prove the later marker was written. SPEC.md:3286-3291 permits
retaining the visible alternate frame if the program does not leave it as it exits. Wait for the post-registration
marker before Stop while keeping the live alternate-frame check. Bucket: other because this is a fixture-readiness gap.
Confidence: confirmed by source that the test has no post-registration barrier. No intermittent failure observed.
Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/l6`.
