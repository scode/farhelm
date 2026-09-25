# Clone reports a missing source when the list was truncated

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a very busy host, cloning an existing session can fail with a misleading "no longer exists" message.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F13 / COR-CLONE-TRUNCATED`, tagged **possible**. Anchors and title:
`farhelm-helm/src/agent_requests.rs:1188-1202` — `agent clone` reports "no longer lists session" when the source host's
list was truncated at 500

To clone, the helm reads the source live. It finds which host owns the source, asks that host's supervisor for its whole
session list (`drain_sessions`), and searches the list for the source id (`agent_requests.rs:1188-1202`). Supervisors
cap that list at 500 rows (`LIST_SESSIONS_CAP`). They keep the newest 500 by creation time and set a `truncated` flag on
the listing (`farhelm-supervisor/src/service/listing.rs::order_and_cut`). `clone_for_agent` never looks at the flag. If
the source is an older session that fell past the cut, the search misses it and the clone fails with `NotFound`: "the
selected source host no longer lists session …, so there is nothing to clone". The agent is told the session is gone
when the read was only partial.

The open premise is whether a single host realistically holds more than 500 sessions. The suggested fix: when the source
is missing from a truncated list, say that the list was incomplete. Better still, read the single session directly
instead of scanning the list.

Restater note: the owner lookup that runs before this read (`route_session` → `resolve_owner`) uses the helm's session
cache, and that cache is fed by the same capped listing. I did not trace whether a truncated drain removes older rows
from the cache. If it does, a source past the cut usually fails earlier, at routing, with a different "no such
session"-style refusal. That refusal is equally misleading, but it comes from another place. The exact message quoted
above would then need a narrower window: the source is still cached but has just fallen out of the newest 500 when the
live read happens. Either way the underlying problem stands: a session past the cap cannot be cloned, and the error does
not say why.
