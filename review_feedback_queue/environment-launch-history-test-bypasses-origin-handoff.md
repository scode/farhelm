# The environment launch-history test skips the origin handoffs

Reviewed commit: ba8f163fb26383ecab6c9642fbd4299ca1463003

## TLDR

The launch-history regression test can pass if an agent-requested environment starts becoming the desktop’s default host
again.

## Details

`SPEC.md:5657` reserves launch history for explicit desktop choices. The environment acceptance test in
`crates/farhelm-helm/src/sessions_tests.rs:89` supplies the agent origin directly and checks the resulting preferences.
The ordinary environment and fresh-checkout routes pass that origin through separate acceptance calls
(`crates/farhelm-helm/src/sessions.rs:1964` and `:2561`). The current test would still pass if either handoff started
losing the origin. A request-level assertion would cover the changed boundary.

Bucket: other. The shipped handoffs and the coverage gap are confirmed by source inspection at
`66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `tensorlake-setup/f34`.
