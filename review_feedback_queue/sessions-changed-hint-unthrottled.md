# A hostile host can flood the helm and every client with refreshes

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

A compromised remote host can keep the helm and every open browser and desktop window continuously refreshing the whole
session list, which may slow down the UI for all hosts. The size of the impact has not been measured.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F59 / SEC-HINT-FLOOD`, tagged **possible**. Anchor and title: `crates/farhelm-helm/src/manager.rs:3650` — a
remote host can drive unthrottled helm refreshes and fleet-wide client re-reads with "sessions changed" hints.

Besides polling each host's session list every 3 seconds, the helm accepts a "sessions changed" hint from a supervisor
(since protocol 33): a content-free message meaning "something changed, re-read me now". On the helm, each hint just
increments a counter (`crates/farhelm-helm/src/client.rs:1969-1971`). The per-host connection task in
`crates/farhelm-helm/src/manager.rs` reacts to it: when a refresh completes and it answers a pending hint, the helm
always raises a fleet-wide change event (`if cache_changed || answers_hint { self.events.bump(); }`, lines 3637-3639),
even when the session list did not change, by design, because a client might be showing something stale. If another hint
arrived during that refresh, the next refresh starts immediately with no wait (`continue`, lines 3650-3653); otherwise
the hint branch of the wait wakes it at once.

The only rate limit is on the sending side: the supervisor spaces its own hints at least 200 ms apart (`HINT_MIN_GAP` in
`crates/farhelm-supervisor/src/service/hints.rs:62`, whose comment explains each hint costs a full list round trip plus
a live read by every open session view). The helm enforces nothing. A compromised supervisor that sends a hint right
after each list reply can therefore drive back-to-back refreshes limited only by round-trip time. Each one rewrites that
host's cached sessions in the helm's database and raises a fleet-wide event, which wakes every browser and desktop
client to re-read the whole fleet list, and makes each open session view do a live read of its own host, which may be a
different, unrelated host.

SPEC.md says malicious behavior by a remote host must not disrupt unrelated hosts or ordinary helm/GUI controls, and
that supervisor messages are untrusted. Before hints existed, a host could cause at most one such event per 3-second
poll. Two reviewers independently reported this, both as possible. Open premises: the actual degradation has not been
measured; the event feed and the client readers do coalesce (roughly one re-read per client at a time); and it is a
judgment call whether resource exhaustion by a remote host falls under SPEC.md's untrusted-supervisor rule, which also
says to choose proportionate remedies. The related queue item `session-detail-drains-full-list.md` covers the cost of
each session-view read.

The suggested fix is a per-connection minimum gap between hint-driven refreshes on the helm side (reusing the 200 ms
`HINT_MIN_GAP`, or something in the 250 ms to 1 s range), with at most one refresh left pending; optionally, raise the
fleet event for an answered hint only when the cache actually changed, or under the same limit. Test: a peer that hints
after every reply gets at most one refresh per gap.
