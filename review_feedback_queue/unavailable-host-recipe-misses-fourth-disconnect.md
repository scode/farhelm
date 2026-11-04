# The unavailable-host reproduction omits the fourth disconnect

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The maintained follow-up asks for three disconnects, but a healthy test host reconnects after all three. A test
following that recipe can fail before reaching the intended unavailable state.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static
documentation mismatch; no runtime failure observed

Source locations: a65147ce29e4bfcc4c20110e5b98a3b632b4f431:TODO.md:75-78,96-99.

Pinned TODO.md:75–78,96–99 prescribes three endings. hosts/terminal.rs:1883 initializes an empty reconnect history;
:3910–3920 checks for three previous reconnects before adding the current one, and :3949–3952 returns the third
connection. test_host.rs:218–222,670–677 closes only the current connection. The existing tests.rs:2648–2680 regression
closes the adoption connection plus three reconnects, four closures. The recipe needs a fourth disconnect, with each
reconnect observed before the next. TRIAGE_OUTCOMES.md:20479–20495 excludes named operational artifacts, but whether it
covers maintained future reproduction guidance is ambiguous; retain that possible coverage.

TODO.md:96-99 says to call drop_connection three times within a minute and check that the session remains unknown. At
the tip, hosts/terminal.rs:1883 starts the reconnect deque empty, and reconnect at 3910-3920 checks len >=
RECONNECT_LIMIT (3) before pushing the current reconnect. That appears to allow three successful reconnects and make the
runtime unavailable on the fourth disconnect. test_host.rs:218-222 only sends DropConnection.

Following the TODO's instructions to disconnect a healthy test host three times does not reproduce the promised
unknown/unavailable session; it reconnects successfully, so a future test written from the entry fails for the wrong
reason. Details/source: TODO.md:75-78 calls the instability trigger three connection endings, and 96-99 explicitly
prescribes three drop_connection calls. hosts/terminal.rs:1883 initializes reconnect history empty; 3910-3920 checks
whether three previous reconnects exist before pushing the current one. Successful resume returns the third new
connection at 3949-3952, so a fourth ending is required to publish Unstable. test_host.rs:218-222 sends one
DropConnection, whose handler at 670-677 closes just that current connection without an extra failure. Existing
regression hosts/terminal/tests.rs:2648-2680 explicitly drops the adoption connection and then each of three reconnects:
four closures total.

Possible coverage and caveats: TRIAGE_OUTCOMES.md:20479-20495 records a decision excluding operational artifacts,
specifically plans, plan reports, landing/delivery records, validation accounting and review/approval bookkeeping. The
same decision also discards an unexecuted plan's contradictory behavior guidance at 20512-20524 and inaccurate
background in a landed plan at 20568-20581. These establish that the exclusion is not limited to historical reports.
However, they do not name the maintained TODO board or future product-bug reproduction guidance, and none has the same
incorrect-recipe trigger/consequence/scope. f2 is future implementation guidance in a wanted product follow-up, not an
accounting complaint about a completed run. Applying the exclusion to it is plausible by analogy but not exact coverage;
under recording.md's ambiguous-coverage rule it remains recorded for the parent's decision. No matching FILTER.md entry
exists at the pinned tip.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g050/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is a65147ce29e4bfcc4c20110e5b98a3b632b4f431;
discovery check at pinned tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime
occurrence and frequency were not measured.
