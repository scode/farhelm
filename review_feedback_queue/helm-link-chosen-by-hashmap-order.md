# Agent requests routed to a stale helm link by HashMap order

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a network hiccup, agent commands can time out or report "outcome unknown" while the UI looks connected.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F7 / COR-HELM-LINK-ORDER`, tagged **possible**. Anchors and title:
`farhelm-supervisor/src/service/agent_relay.rs:597-623`, `farhelm-supervisor/src/service/agent_relay.rs:683-693`,
`farhelm-supervisor/src/service/handlers.rs:1810-1822`, `farhelm-supervisor/src/service/terminals.rs:559-582` — Agent
requests can be routed to a stale, half-open helm connection chosen by HashMap order

To send an upcall, the supervisor must pick which helm connection to use. The rule (`helm_link_for_session`,
`agent_relay.rs:597-623`) is "the helm holding this session's attachment". An **attachment** is an open terminal view of
the session, and it remembers which connection it arrived on. The code collects every attachment for the session by
iterating the `attachments` map, which is a `HashMap`, so iteration order is arbitrary. It then returns the first one
whose connection is still in the registered-links list (`registered_link_for_attachments`, `agent_relay.rs:683-693`).

The doc comment says lease takeover guarantees that all of a session's attachments belong to one helm, so "the first
registered match is that helm". The takeover code does not guarantee that. A **lease** identifies a client. An attach
with a different lease evicts all of the session's other attachments. An attach with the **same** lease, which is what a
helm reconnecting after a network hiccup presents, removes only the entry for the same terminal
(`handlers.rs:1810-1822`, `terminals.rs:559-582`). Other terminals of that session that the helm has not re-attached
yet, such as a tab the user navigated away from, stay bound to the old connection. If the old transport is half-open (an
ssh link partitioned while idle), nothing tears it down, so its link stays registered. The session then has attachments
on two registered links, and hash order decides which one gets the request. The docstring's "first registered match" is
also not what the code computes: it returns the first matching attachment in hash order, not the earliest-registered
link.

If the dead link is picked:

- A read-only verb like `sessions` waits the full 30 seconds and returns `Timeout`, even though a healthy link exists.
- A mutation reports "outcome unknown" and keeps its delete fence for up to ten minutes (see F4). That blocks deletion
  of the asking session and queues its later mutations behind it.

The UI looks connected throughout. The open premise is how long a replaced connection's attachments actually survive
after the helm reconnects. The suggested fix is to choose deterministically: among links that own any of the session's
attachments, prefer the most recently registered, which is the highest index in the `helm_links` vector, since links are
appended as they register. Also correct the doc comment.
