# Live-route conformance can pass when a terminal control skips its target branch

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

Keeping the target terminal alive improves setup, but the reader-progress test can still pass without proving that the
target control ran under contention.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:11160`.

handlers.rs:10860-10866 claims the declared paths are exercised; :10895-10990 retains the secondary TestHost, and
:11040-11063 verifies the secondary attachment and its initial pause state. At :11159-11196 the only post-send check for
PauseOutput, ResumeOutput, Detach and Resize is input reaching the separate primary attachment (see :10668-10698). The
Pause done before Resume is checked as a setup premise. The tested Resume itself has no postcondition. A control that
silently takes a no-op path can therefore pass even if the intended target branch was never exercised. The Resize
representative also requests the same 80x24 size as its target attach (service/request_modes.rs:307-312 and
handlers.rs:10951-10958), so checking only the resulting dimensions would not distinguish a successful call. Actual
dispatch and effects are implemented at handlers.rs:3027-3051, :1961-1986 and :2041-2065. SPEC.md:5149-5166 requires
reader progress for terminal control. Keep the independent progress marker and add a post-dispatch target state or
dispatch witness (pause bit, attachment removal or a distinct applied resize). Bucket: other (scope of this conformance
test). Confidence: source-confirmed test gap. No production control failure established. Possible coverage:
lifecycle_e2e for individual control behavior. Equivalent control-under-contention witness unverified.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f13`.
