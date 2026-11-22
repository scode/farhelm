# List admission test observes reply timing instead of list construction

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

A regression could let a busy supervisor build lists outside its concurrency limit while this test still passes.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:11308`.

handlers.rs:556-567 and :595-619 place the permit around list construction and release it before sending specifically to
bound construction cost. The test holds every permit (:11292-11301), observes later terminal input (:11303-11305), then
checks only that no complete ListSessions reply arrived in one second and that one arrives after releasing permits
(:11306-11319). An implementation that constructs the list before acquiring a permit, while still awaiting the permit
before replying, satisfies every assertion. Even a full bypass is observable only if its reply reaches the fixture
within the window. The input witness establishes connection-read progress, not list-task entry or construction. The
cancellation concern is refuted: farhelm-proto/src/io.rs:142-177 retains partial bytes in FrameReader, and
handlers.rs:10610-10665 files complete frames before its next await. Preserve the input/reply check and add a
deterministic witness around construction or the permit boundary if this test is meant to guard the work budget. Bucket:
other (regression coverage only). Confidence: source-confirmed assertion gap. Current shipped path takes the permit
correctly. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f3`.
