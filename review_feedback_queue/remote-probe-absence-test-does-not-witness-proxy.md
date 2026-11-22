# The probe test cannot establish that its stand-in ran

Reviewed commit: 72dcafbc581259b7411c16f24995dab9f25f0986

## TLDR

The remote probe regression test can pass through the missing-program path both times, without checking how it handles
the stand-in's exit.

## Details

`crates/farhelm-helm/src/provisioning/backend.rs:4282` expects the same absence classification before and after
installing the stand-in. A test that observes the stand-in's own execution would distinguish the two intended cases.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`. Findings: `remote-command-status/r15`.
