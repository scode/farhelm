# Borrower reload lacks a completed origin-runtime handoff witness

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

The borrower test compares status across reload, but can pass without showing the origin terminal was released and
successfully recovered.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:37221`.

core.rs:37123-37143 starts the origin and creates the borrower; :37203-37208 confirms only the borrower's host ended.
The origin helper (:35282-35289) calls HostTerminal::abort, which requests cancellation but does not await the old task
(hosts/terminal.rs:2415-2427). Then the test immediately reloads and checks only the borrower's Error classification and
its agreement with storage (:37217-37240). SPEC_impl.md:2234-2247 permits the writable scan to take over and lets it
return unavailable handles, so the test can pass if cancellation had not completed at adoption, or if the origin returns
unavailable. It still checks borrower classification. The gap is the claimed restart premise and origin recovery. Await
a completed release and assert a live adopted origin, if the test is to establish that handoff. Bucket: other
(fixture/test scope). Confidence: source-confirmed missing witness. No runtime timing result or production breach
observed. Possible coverage: recovery, recovery_contract.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f7`.
