# Connection incarnations restart at 1 on every helm start

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A create prepared before a helm restart and submitted after it can occasionally launch on a replacement machine instead
of being refused.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F20 / COR-INCARNATION-RESTARTS`, tagged **possible**. Anchors and title: `manager.rs:1337`, `precondition.rs:62` —
Connection incarnation numbers restart at 1 every helm process, so the expected_incarnation guard can pass for a
different install after a restart

A session create may carry `expected_incarnation`: the connection token the UI saw when it prepared the create.
`incarnation_holds` (`precondition.rs:62`) refuses the create with a 409 if the host's current token differs. This turns
"a retarget or adoption replaced what answers on this host" into "ask again" instead of a silent launch on the wrong
machine.

The token is a plain number from one manager-wide counter, seeded at 1 in every helm process (`manager.rs:1337`). It is
bumped on each connect and disconnect across all hosts. After a helm restart, hosts connect again in roughly the same
way and get small numbers again. `incarnation_holds` compares only the number. So a value a client captured before the
restart (for example in an open New dialog) can equal the token of a completely different connection after it. That
includes a connection to a replacement install, if a retarget or adoption also happened.

The guard therefore relies on a numeric coincidence being unlikely. A small fleet and a helm restarted soon after its
previous start make the coincidence more likely.

Suggested fix: seed the counter from a per-process random value, or mix a per-boot nonce into the token the API
publishes.

User-visible consequence: a create prepared before a helm restart and submitted after it can occasionally launch on a
replacement machine instead of being refused.
