# The terminal-kind specification still excludes tmux tabs from migrated host-agent sessions

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The authoritative implementation specification contradicts itself about preserving an existing session's old terminal
tabs when its agent moves into a terminal host.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed textual
conflict.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:SPEC_impl.md:2078-2084.

original SPEC_impl.md:2023-2025, retained at tip 2078-2080, states a host agent has only host tabs. The very same
paragraph at tip 2083-2084 says a host agent can have old tmux tabs; tip 2247-2249 explicitly preserves those tabs
across the migration; tip SPEC.md:1397-1398 likewise preserves old tabs. The complete Tabs in hosts bullet was rechecked
through tip 2119, including its merging of both kinds at 2103-2104. Correct the blanket clause, for example by
qualifying sessions created in hosts.

p0648.diff lines 138-140 versus 191-199; the corresponding implementation module at the reviewed commit,
crates/farhelm-supervisor/src/service/core/host_restart.rs:115-126, likewise preserves old tmux tabs. The contradiction
remains in SPEC_impl.md:2078-2080 at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.

Possible coverage and caveats: none suppressing; the nearby correct contracts establish the contradiction rather than
accept it. Tip TODO.md:23-25 has no Planned entries; Near term 181-213 concerns hidden tabs/history when the agent
terminal is absent, not this wording. Coverage searches of plans/queue, BUGS.md, queue INDEX and matching queue files,
and TRIAGE_OUTCOMES.md found no matching trigger/consequence/scope. The offline-uninstall queue/ledger hit concerns a
different operation and is not coverage.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g012/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is fba73468a9cb09c217a39da2be7f2c7cea064cb8.. This
was a source-only review; runtime occurrence and frequency were not measured.
