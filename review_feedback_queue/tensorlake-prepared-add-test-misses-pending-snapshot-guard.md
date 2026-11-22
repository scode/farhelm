# The prepared Add test misses snapshot retention while the sandbox starts

Reviewed commit: 07c31951ac4341774282b9783f6031a152fda4ca

## TLDR

The prepared Tensorlake Add test can pass if Farhelm releases the chosen snapshot for retirement while the new sandbox
is still starting.

## Details

`SPEC_impl.md:6776` requires holding the source guard until the first Running observation.
`crates/farhelm-helm/src/tensorlake/hosts.rs:485` polls the sandbox and releases the guard before the later SSH wait.
The test in `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:1881` checks that the guard has been released
by the wait. It leaves the earlier retention period unobserved. A Pending fixture would distinguish an early release
from the documented handoff.

Bucket: other. The production lifecycle and the test boundary are confirmed from pinned source.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `tensorlake-setup/f35`.
