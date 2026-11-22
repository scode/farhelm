# No existing assertion covers terminal loss between publication and the Restart reply

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

An assertion can pass after this patch's late recovery-status sample is removed, despite a Restart acknowledgement again
being able to hide the recovery card.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:13192`.

core.rs:13181-13200 refreshes terminal_missing after publication and old-host cleanup at host_restart.rs:388-391. The
changed core.rs:20524-20537 fixture shuts down the old host before invoking Restart, :20569-20573 asserts the held
listing, and :20575-20588 only checks that Restart succeeds and the row remains listed once. status.rs:872-881 checks
entry_info, and core.rs:35529-35534 checks create replay. The tip's BeforeOldHostEnded e2e cases inject an interrupted
Restart and check convergence. They do not assert a successful reply's flag after replacement loss. SPEC_impl.md:479-481
and :8379-8383 specify why a positive acknowledgement must describe the terminal that was published. Bucket: other.
Confidence: confirmed for the assertions inspected. No test execution. Possible coverage: recovery_contract,
lifecycle_e2e.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r7`.
