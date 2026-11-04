# The tab-reap polling deadline does not bound an individual listing request

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The tab-exit regression's advertised 15-second failure boundary does not cover the listing it awaits.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed structural
deadline defect; no observed flake, production deadlock or user-work-loss route claimed.

Source locations: 7987d070e91684f4c52df7695136cb87e66bd94f:crates/farhelm/tests/e2e/host_tabs.rs:208-220,836-853;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:208-220,864-881.

at the original commit host_tabs.rs:208-220 awaits condition().await before consulting the deadline, and returns
immediately on Ok even if late. Its caller :836-853 awaits listed_tabs without another timeout. At tip the same
locations are :208-220 and :864-881; terminal_tabs.rs:480-495 forwards to SupervisorClient::list_sessions;
farhelm-helm/src/client.rs:2925-3001,3215-3246 documents and implements a reply wait with no deadline. Static backend
route established at tip: service/listing.rs:174-223 asks tmux for every terminal-bearing entry, including host agents;
tmux.rs:4662-4720 uses run -> run_bytes (:2595-2602,:2881-2891), which passes no deadline to command.output().await
(:2893-2937). Thus no hidden per-request timer repairs the helper contract. This establishes conditional source behavior
without asserting that a real run currently hangs. The 15-second constant is host_terminals.rs:47-48;
.config/nextest.toml:18-21 has a separate five-minute slow-test boundary plus grace, which does not implement the
helper's timeout. Fix: timeout_at the whole condition future using the existing deadline and retain prior observation
diagnostics.

wait_for_condition awaits condition().await before checking its deadline; an_exited_host_tab_is_reaped_on_its_hosts_exit
passes a closure awaiting listed_tabs. terminal_tabs.rs:480-495 forwards directly to SupervisorClient::list_sessions;
farhelm-helm/src/client.rs:2930-2938,2950-2999,3229-3244 explicitly waits without a timeout while the connection remains
open. The test-authoring checklist requires bounded polling; wrapping each condition future in timeout_at(deadline, ...)
preserves the diagnostic boundary.

The tab-exit regression's advertised 15-second failure boundary does not cover the listing it awaits. A non-answering
listing can wait until the outer runner intervenes, and a successful observation arriving after the deadline is
accepted.

Possible coverage and caveats: none with matching trigger, consequence and scope. Repeated targeted searches across
SPEC.md, SPEC_impl.md, empty TODO Planned (:23-24), plans/queue, BUGS.md, review queue INDEX/files and
TRIAGE_OUTCOMES.md found no item for this helper. At tip the full matching ledger section TRIAGE_OUTCOMES.md:17735-17762
scopes its analogous completed repair exclusively to await_cli_approval. SPEC_impl.md:2078-2119 promises event-driven
automatic tab cleanup and :6742-6808 describes test substrate, neither accepts an unbounded test poll.
.agents/test-authoring.md:18-20 requires bounded polling. FILTER.md:44-67 may suppress a product hang whose only cause
is hung private tmux, but does not explicitly settle this separate test-helper timeout contract; the issue also accepts
late successful conditions and is not asserted as a new product hang.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g181/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7987d070e91684f4c52df7695136cb87e66bd94f;
persistence reverified at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review;
runtime occurrence and frequency were not measured.
