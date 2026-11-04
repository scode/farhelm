# Classic mouse-report snapshots keep links blocked after copying

Reviewed commit: d17f0d61ed253ebeb8369fc81e38c281af8d9b4b

## TLDR

After copying from a restored frozen terminal in classic mouse-reporting mode, web links may still hover while ordinary
clicks do nothing. Forced deselection or typing is a workaround. Native behavior and classic-mode prevalence are
unverified.

## Details

`crates/farhelm-ui/assets/terminal.js:1285–1315,1824` restores observation/copying without clearing selection. The live
web-link interception clears it (`5582–5601`), but unmount removes that listener (`5627–5629,5902`). Web/OSC activation
rejects a nonempty selection (`3672–3677,3752–3759`). After forced local drag copying, xterm's disabled selection
service ignores ordinary primary presses. Classic DEFAULT reporting uses `triggerBinaryEvent`, which does not emit the
input notification that clears selection; the retained binary handler (`5665–5668`) does not clear it either.

SGR is excluded: its `triggerDataEvent(t,true)` emits that notification and can clear selection even with the old socket
closed. Bounded vendor excerpts support this encoding distinction. Restore appropriate selection dismissal for snapshot
web-link presses while preserving drag rejection and detached transport. The snapshot regression checks links before
copying (`e2e/tests/terminal-reconnect.spec.ts:790–823` onward), missing this ordering.

Suggested bucket: **other**. Confidence: **likely**, with source/vendor paths established but restored-screen behavior
unexecuted. No security, secret exposure or user-work loss was established. Proposed drop: none.

Possible cover: final `SPEC.md:1815–1846`, `SPEC_impl.md:1286–1310` and ledger `16934–16951` support copying and usable
snapshot links without this exception. The classic-selection outcome at `TRIAGE_OUTCOMES.md:7333–7352` concerns stale
clipboard copying, not blocked links. FILTER may cover rare self-correcting behavior, but ordinary retries keep failing
while selection remains; rarity and complete match are unestablished. No exact Planned, BUGS or queue match was found.
Provenance: `terminal-links-contracts-2/f4`, discovery pass 2, trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `terminal-links-contracts-2/f4`.

## Trailing review through ca9b0a19

After copying a forced local selection from a frozen terminal snapshot whose program enabled mouse reporting, ordinary
file-link clicks can keep doing nothing until the selection is explicitly cleared.

Suggested bucket: **extend**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static state
and callback route, no runtime reproduction or frequency claim.

Source locations:
7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1402-1407,1671-1718,1992-1996.

freeze retains state/selection while setting frozen=true and send=null (R1992-1996; T1998-2003). A subsequent unmodified
primary click in tracking mode reports instead of selecting (R1671-1718; T1677-1724); sendUserInput returns before
clearSelection when frozen/sendless (R1402-1407; T1408-1413). File-link objects are web=false (T terminal.js1271,1326),
bypassing the web-interception-only selection clear at T grid-terminal.js1694-1710. The release still invokes the same
file link (T1590-1618), whose activation handler requires !grid.hasSelection (T terminal.js1286-1287,1333-1334), so it
refuses the click. The same state and same refusal survive ordinary repeated clicks. The grid's own copy handler remains
installed; T copy-on-select.js50-100, especially79-82, leaves its forced local selection in place. T
terminal.js6305-6315 freezes the held grid, and1918-1944 restores it as an interactive tombstone without replacing its
own grid listeners. moveInputUnder/restoreInput at T grid-terminal.js1526-1539 do not clear selection; the caller's
legacy mousedown clear is on separate xterm elements (T terminal.js1421,5926), and the grid.clearSelection caller
reference at5376 is paste interception. Plain web links in default interception mode do clear at1710, so the retained
unconditional scope here is file links; no claim that all links fail.

Possible coverage and caveats: material source/scope extension of
review_feedback_queue/classic-mouse-snapshot-copy-blocks-later-links.md (full1-34), NOT a fresh underlying family count
or automatic requeue. That existing item is already discarded by T TRIAGE_OUTCOMES.md19375-19386, execution pending: its
assessment explicitly limits the allegation to classic(non-SGR) web links in terminal.js/xterm, with unverified
vendor/native behavior. The present route is first-party grid file links, independent of classic/SGR encoding, with a
directly established clearing gate. The ledger's linked filter instruction at19348-19361 rejects routes missing a
shipped premise; this route has no such missing premise. Whether the earlier discard intended the broader frozen-link
convenience family remains possible coverage; do not overwrite that decision. Required coverage searched across SPEC.md,
SPEC_impl.md, TODO Planned, unlanded plans/queue, BUGS.md, queue INDEX and TRIAGE_OUTCOMES.md. Relevant sections read in
full: T SPEC2235-2284,2285-2332,2359-2371; SPEC_impl1645-1670; TODO23-27(Planned empty);
ledger10421-10447,16934-16955,19348-19395. The earlier snapshot copy/link repair requirements10421-10447 and16934-16951
support expected interaction but do not cancel the later discard. No other exact scope acceptance found; no blanket
runtime-proof requirement inferred.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g160/f6`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f (R);
persistence checked at ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This was a source-only review; runtime occurrence
and frequency were not measured.
