# Approval polling can outlive its deadline

Reviewed commit: ac595ee0bcff499805f34e3b00a0dfa3ffd8ba41

## TLDR

A stalled helm response can keep an approval test waiting past its own deadline until the outer test runner kills it,
losing the intended timely failure and fixture diagnostics.

## Details

Suggested bucket: **other**, for maintainer test infrastructure. Confidence: **confirmed missing deadline**, conditional
on an HTTP response stalling; no reproduction or frequency measurement was run.

`crates/farhelm/tests/e2e/agent_listing_real_stack.rs:211–225` awaits `get_json` before checking the deadline or CLI
liveness. The new keyed-create/clone caller at `1462` adds no enclosing timeout.
`crates/farhelm/tests/e2e/harness.rs:832–843` builds the authenticated client without a timeout; `855–862` awaits both
response headers and body without one. `crates/farhelm-testtrace/src/lib.rs:995–1025` does not supply a body deadline.

The extracted browser-link loop already had this omission. The reviewed change shares it and adds a caller; it does not
introduce the older caller's underlying behavior. `.config/nextest.toml:18–20` bounds a worker to roughly 300 seconds
plus grace, so this is not a claim that the supported nextest run waits forever.

Wrap the complete HTTP operation, including body consumption, in the helper's absolute deadline. A focused test can hold
a response open and distinguish that deadline from the runner's worker termination.

Possible cover: none found in the specification, Planned bucket, known bugs, queue or triage decisions. No filter
matches the whole stuck test-wait consequence. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g005/f1`.
