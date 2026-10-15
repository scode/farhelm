# Trust cleanup can exceed the desktop startup deadline

Reviewed commit: 0ad806334217ba8dde4e7f7dc6ecc7480d27cb9a

## TLDR

Two leftover checkout trust records and one stale Claude lock can make Farhelm refuse to open.

## Details

Suggested bucket: `high`. Confidence: confirmed static interaction; desktop startup was not reproduced. Source: separate
adversarial trailing-review agent, Claude trust scope. Proposed drop: none.

`crates/farhelm-supervisor/src/service/core.rs:5707–5725,5771–5774` retries trust removal serially before serving. Each
contended removal takes the full twelve-second budget in `agent_kind/claude/trust.rs:179–180`; failure retains the
record. Session reload runs during construction at `core.rs:5524` and again before binding at `:6792`. Two eligible
retired records sharing a persistently held vendor lock can therefore spend about 48 seconds waiting across those two
passes. The desktop's readiness budget is 35 seconds (`crates/farhelm-ui/src/desktop.rs:891–892,937–940`).

A crashed vendor process can leave its directory lock behind, and Farhelm deliberately never steals it. The resulting
delay needs neither a slow filesystem nor a hung tmux server. The implementation report acknowledges the serial startup
budget at `plans/reports/claude-trust-prompt.report.md:149–151` but does not resolve its interaction with the desktop
deadline. Keep best-effort cleanup from accumulating its full per-record contention budget before serving, while
preserving ownership and path-reuse fencing.

A focused reproduction can create two retired records with absent original paths and retained trust evidence, hold their
shared vendor lock, and measure readiness through the real desktop startup path. Verify the records remain recoverable
and unrelated sessions remain usable. No such runtime reproduction was performed in this review.

Possible cover: none found. SPEC.md makes trust removal best effort; it does not accept cleanup preventing app startup.
The healthy-filesystem and hung-tmux filters do not cover ordinary vendor-lock contention. No matching Planned item,
BUGS entry, queue item, or ledger decision was found.

The entire 50-file introducing patch was read, with startup and desktop deadline context inspected separately. No
changed scope went unread; unrelated surrounding modules were not exhaustively reviewed.
