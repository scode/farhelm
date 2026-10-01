# A host retarget can briefly route to the old machine

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

Right after the user changes a host's ssh destination, a narrow race can briefly make the old connection current again,
so an operation sent in that moment might go to (or fail against) the machine the user just stopped pointing at.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F19 / COR-RETARGET-RACE`, tagged **possible**. Anchor and title: `crates/farhelm-helm/src/manager.rs:4135` — a
refresh finishing during a host retarget can re-publish the old, retired connection.

In the helm, each host has a background task, its connection actor, that holds the connection to that host's supervisor
and refreshes the session list. The actor publishes the host's status, including the connection that sessions operations
are routed through and a connection-generation number that pending operations check against. When the user changes a
host's ssh destination (a retarget), the host manager does several things in one step
(`crates/farhelm-helm/src/manager.rs:1469-1496`): it sets the host to "connecting", takes the old connection out of the
published status, bumps the generation number, tells the old connection to shut down, and then nudges the actor to
reconnect. The comment there (lines 1458-1462) says the old connection "must not remain routable for even the interval
before the nudge lands", because an operation sent on it would reach the machine the user just stopped pointing at.

The actor's refresh loop (around lines 3587-3627) checks for a nudge as soon as a refresh finishes. If there is none, it
publishes the refresh result through `publish_refresh` (from line 4135), which writes the status unconditionally. The
race works like this. The actor checks for a nudge and finds none. The manager then withdraws the client and nudges.
Then the actor publishes. The publish sets the status back to "connected" with the old connection. And because the
published connection changed from none to the old one, it also mints a fresh generation number, which operations will
then validate against. The state corrects itself at the actor's next loop iteration, when it sees the nudge.

In the worst case, an operation is routed in that gap to the machine the user just retargeted away from. This is marked
possible because it depends on the two tasks interleaving in exactly that gap on the multi-threaded runtime. The
suggested fix is to make `publish_refresh` conditional: skip the write and the generation bump when the published
connection is no longer this actor's connection, or when the generation differs from the one this connection published.

Restater note: the retarget also calls `retire()` on the old connection, which signals its shutdown
(`crates/farhelm-helm/src/client.rs:1693`). Whether a request sent on a retired-but-republished connection can still
reach the remote supervisor before the shutdown takes effect was not confirmed. If it cannot, the practical consequence
is a failed operation rather than one routed to the wrong machine.
