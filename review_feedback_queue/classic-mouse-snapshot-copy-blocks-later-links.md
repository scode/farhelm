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
