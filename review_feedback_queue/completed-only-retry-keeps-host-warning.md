# A retry first seen completed can leave the old warning

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

Another client's successful retry may leave an earlier host-failure warning showing if this GUI misses every Running
observation for that retry. Opening Hosts acknowledges the badge. The end-to-end schedule remains unverified.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**, with confirmed conditional state-machine
behavior. No action, durable mutation, security or user-work-loss consequence was established.

`crates/farhelm-ui/src/hosts/attention.rs:164–219,226–233` clears a warning through its eligible cause run or a recovery
ticket. For another client's retry B, Running creates that ticket. If this client observes Failed A and then only
Completed B, neither comparison clears A's warning; repeated Completed B snapshots do not repair it. A local acceptance
receipt supplies another recovery route, so the concern specifically needs another client or no local receipt.

The reviewer traced the current-run-only server snapshot (`farhelm-helm/src/provisioning/service.rs:54–61,2441–2457`),
completion publication (`2094–2129`), watch-based feed (`events.rs:206–249`) and coalesced GUI reads
(`farhelm-ui/src/feed.rs:289–315`; `reader.rs:644–678`). There is no transition replay or source guarantee that this
client receives Running before Completed. The missing premise is a concrete supported two-client schedule skipping all
Running observations while this GUI remains mounted. No runtime trace was taken. A controlled two-client delayed-read
trace would settle it. The mechanism remains at pinned tip `9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

SPEC.md:133–137 and the draft release fragment `releasing/changelog.d/steel-sidebar.md:11–12` say a later success of the
same action clears the warning. Either qualify the promise to correlated success or establish sufficient run ordering
before broadening acknowledgement; unrelated successes must still leave newer failures alone.

Possible cover: FILTER.md's rare, self-correcting diagnostic rule. Opening Hosts clears this mark
(`hosts.rs:5072–5078`), but rarity and whole-finding coverage were not established. No exact specification exception,
Planned item, unlanded cause-removing plan, BUGS entry, queue item or ledger decision was found. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, independently identified as `g009/g009-f1` and
`g012/g012-f2`; recorded once after reconciliation.
