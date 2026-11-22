# Cleanup can report failure when its verified container has already disappeared

Reviewed commit: b23bbba44fa7bcf0d111696ceb744c54042e6eff

## TLDR

Delete can report a failure and leave a host or helper record for retry even though the requested container is already
absent.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman.rs:450`.

podman.rs:450–470 verifies ownership and sends rm by canonical ID. require at :223–236 rejects nonzero status before the
trailing absence observation. podman-rm(1), Exit Status, specifies status 1 when a requested container did not exist and
no other failures occurred. A container can legitimately disappear after ownership inspection from a separate removal.
The rowless delete adapter also runs per confirmed request with no shared deletion guard (podman.rs:778–810 and
:986–1035 at tip). On a later sequential retry, the missing target succeeds. The code thus distinguishes absence before
inspection from absence at rm, which conflicts with the stated confirmed-idempotent and missing-resource contract
(SPEC_impl.md:6031–6035 and :6058–6064). Recognize the documented missing status and then independently confirm absence
of the exact ID. Preserve other errors and the ownership guard. Existing podman/tests.rs:392–455 tests a sequential
retry after absence, and :1095–1148 waits for a successful rm before checking absence. The failure fixture models a
refusal with the container still present. None witnesses the documented missing-at-rm result. Bucket other for false
diagnostic and recoverable retained ownership, with no additional resource loss established. Confidence source-supported
given documented Podman status. No overlapping operation was run. Possible coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `podman/l13`.
