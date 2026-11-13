# Cache lock repair assumes Git ends with its preparation job

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

Unverified after investigation: repository-cache repair could interfere with a Git writer if that writer survives the
preparation process holding Farhelm's outer lock. Git does not inherit that lock, but a real Git collision, checkout
failure or cache damage has not been established.

## Details

Recorder bucket: **other**. Confidence: **unverified after investigation**. A fresh independent consultation agreed to
this priority for the established route, preserving the missing real-Git collision and stronger conditional
consequences. The reviewer's original **high** suggestion was conditional on material checkout failures; its earlier
orphan/corruption implication is preserved in the private history, not established here. No user-checkout loss,
credential loss or supervisor-database damage is claimed.

Reviewed change: `30f2768904fdb1bec27188c9b820f17f2a145924`, interacting with scope cleanup in
`0c440c7f992b230eed7e48595c48828bb60aa715`. References below use the reviewed tip.
`crates/farhelm-supervisor/src/launch.rs:2159-2166` takes the cache flock and removes Git locks before running Git;
`repo_cache.rs:33-51` removes root/ref transaction locks without observing their owners. The outer flock is CLOEXEC
(`launch.rs:2377-2403`), and ordinary Git is awaited through synchronous `Command::status` (`2251-2263`). Ending only
the shim would release its lock without itself signalling a child.

The investigated production paths narrow that concern. Visible Git has no timeout. Stop/Restart collect the whole scope
or marked process tree; background timeout and cancellation collect the whole detached group. A supervisor crash alone
leaves the separate shim holding its flock, which startup skips. No production timeout that kills only the shim was
found. A waiter can acquire the flock once the shim exits. The missing premise is a real Git mutator still holding one
of the removed transaction locks when a competing repair begins, before group collection ends, followed by the alleged
failure or damage. No production sequence establishing that overlap was found.

Those checks covered visible clone (`launch.rs:1945-1979`), Stop/Restart (`service/core.rs:13320-13392`;
`sweep.rs:1111-1217,1775-1865,2037-2080`), background group cleanup (`spare_builder.rs:497-594`;
`bounded_command.rs:134-203`; `spare_build.rs:119-180`) and startup's nonblocking flock probe (`repo_cache.rs:98-140`).
The shim-kill hook fixture (`launch.rs:6968-7055`) demonstrates the process model, not a production real-Git collision.

SPEC_impl.md:4258-4260 asserts that children end with their preparation job, so the locks have no live owner. That is
the ownership assertion under examination. SPEC.md:2069-2082 accepts particular supervisor-crash/coincident-fault
consequences, not every interruption or lasting repository damage. No exact Planned, unlanded-plan, BUGS, queue, ledger
or filter cover was established. A qualifying crash route would need that acceptance applied to its actual consequence.

Available source checks established neither Git's signal/transaction schedule nor a production isolated-shim fault.
Evidence that would settle the concern is a primary-source Git implementation proof for the same route, or an authorized
isolated test recording shim/Git identities, flock release, transaction-lock ownership, competing repair and repository
outcome. No runtime experiment ran. Do not change locking solely on an assumed corruption route.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g004/l1`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Original wording, investigations and consequence revisions remain in the
private findings history. Proposed drop: none for the retained uncertainty.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): The later repo_cache.rs change only
replaces terminal wording in its comment at line 86. SPEC_impl.md:3797-3798 still makes both lock descriptors CLOEXEC,
and :3819-3821 still assumes Git children end with preparation. A real Git mutator surviving outer-lock release,
overlapping repair and consequential damage remain unestablished; removing the old terminal backend does not settle that
missing sequence. This remains unverified after investigation.
