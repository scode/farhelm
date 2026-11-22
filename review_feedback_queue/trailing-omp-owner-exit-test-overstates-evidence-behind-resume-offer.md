# OMP owner-exit test overstates the evidence behind the Resume offer

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

The strengthened test establishes that a saved Resume offer survives process exit and supervisor reload. Its docstring
additionally claims the offer stands on a verified file, which the test does not check.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:24312`.

core.rs:23402-23416 writes a fixture conversation file; :24333-24380 admits a report, confirms the two identities ended,
then asserts the offer and persisted locator/version. The offer helper (:23520-23529) calls session_snapshot
(:7498-7529), which explicitly uses the row and reads no vendor files. service/core/vendor/omp.rs:66-76,139-163
distinguishes the reporter asset checked at admission from the conversation's file and ID checked before actual Resume.
SPEC.md:4138-4155 sets that latter check at Restart, so the absence of a file read here is intended. Describe the
durable offer and process-exit witness in the docstring. This is an assertion/documentation mismatch, with no suggestion
that offer projection should verify the file or that shipped Restart accepts an unverified file. Bucket: other.
Confidence: source-confirmed. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f14`.
