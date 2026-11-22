# Podman diagnostic tests bypass the connection stage fixed by the patch

Reviewed commit: 7ceb053a0c34908b8f081ec48d2f31fd1b711a1a

## TLDR

A regression could again hide the cause of an early container connection failure while the route-specific diagnostic
checks pass.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/manager.rs:4974`.

manager.rs:4954–4975 classifies an open stream as an opened socket or process connection; :5030–5039 then waits for the
process marker before advancing. The local and nested diagnostic tests at :6506–6593 provide ConnectingToHost directly
and call ssh_failure_detail, so their outcomes are independent of the changed stage selection. The actor-level marker
fixture at :7130–7153 establishes an ordinary SSH host, whose initial stage selection is unaffected by restoring the
parent predicate from b71b5f73e9e504444356fbaabc084d4c61b2ec3b:4970–4973. SPEC_impl.md:6079–6102 requires the process
marker and actionable nested failure. Add an assertion at the manager boundary for the Podman process route. Bucket
other, regression-test gap. Confidence confirmed for the cited tests. Searches of the Podman e2e and provisioning tests
found command/diagnostic checks but no matching assertion on the actor. Possible coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `podman/l7`.
