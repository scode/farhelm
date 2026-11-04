# A truncated macOS process listing can declare a live host gone

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

On a Mac with an oversized process listing, test cleanup can erase a stopped host's live socket and state directory,
leaving that host unreachable for future cleanup.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source defect; occurrence/frequency unmeasured, no missing source premise or required runtime measurement.

Source locations: crates/farhelm-teststate/src/hosts.rs:283-315 (1eef3d64e36cd31b56e5b7e815fa3cc183b55637).

hosts.rs:283-315 caps the retained stdout prefix at 8 MiB, checks status success and stdout.complete, then searches only
stdout.prefix before returning Ok(None). process.rs:108-138 defines complete as EOF independently of omitted_bytes.
Later-turn synthesis traced the actual runner at tip:process.rs:305-368,373-415,447-499: normal draining continues after
the retained prefix fills, capture.record discards excess bytes, and a subsequent read of zero sets complete=true
without checking omission. The 64 KiB post-exit drain budget does not refute this: it is separate from reads while ps
remains alive, and a schedule with a small final pipe tail still reaches EOF after discarding more than 8 MiB total.
This is a supported source route, not a runtime timing observation. Apple's official adv_cmds-237 source commit
6bed8737a34dbb54782a18f47dccf933a9967a12, ps/ps.c:453-458,484-487 and ps/print.c:365-368 confirms that -ww/piped output
with command last has no screen-width cap that would rule out the oversized listing. Required trigger remains a
successful EOF-complete listing above 8 MiB with the live host row beyond the retained prefix, combined with a
stopped/full-backlog Mac host. The false negative reaches hosts.rs:215-221 after three seconds of refusals, removes the
live socket, and allows TestDir/sweep removal. Fix: require omitted_bytes == Some(0) before trusting a negative process
result, or use a query that cannot omit candidates silently.

Possible coverage and caveats: none found. Full matching contract at tip:SPEC_impl.md:2504-2515 re-read in synthesis; it
requires retaining unconfirmed hosts. Prior complete coverage assessment remains: Planned empty at TODO.md:23-25,
BUGS.md:1-100 unrelated, TRIAGE_OUTCOMES.md:13987-14014 is the different bounded tmux-probe fix,
podman-test-owner-death-leaves-keeper.md:1-51 is different detached-container ownership, and searches of
specifications/plans/queue/ledger found no exact trigger/consequence/scope match. FILTER does not cover a persistent
resource orphan.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g030/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 1eef3d64e36cd31b56e5b7e815fa3cc183b55637;
explicitly re-read unchanged implementation at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb:hosts.rs:180-320..
This was a source-only review; runtime occurrence and frequency were not measured.
