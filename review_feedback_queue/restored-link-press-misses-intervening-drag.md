# A retained link press can miss drag motion during recovery

Reviewed commit: d17f0d61ed253ebeb8369fc81e38c281af8d9b4b

## TLDR

A mouse press held through recovery may open a terminal link after the frozen screen returns, despite dragging away and
back while its observer was absent. Opening user-admitted active file types gives a conditional security concern; native
event delivery and any harmful outcome remain unverified.

## Details

`crates/farhelm-ui/assets/terminal.js:1755–1760` retains the terminal and its original gesture object. Unmount invokes
cleanup (`5902`), which removes document observation (`5627–5634`) without retiring the object's press. Restoration
reinstalls that same object (`1807–1824,1285–1313`). `terminal-links.js:151–169` marks dragging only from observed
coordinates, so an away-and-back motion during the gap can receive a click token on release near the original point.

Bounded excerpts from vendored xterm's Linkifier support retention of `_mouseDownLink` across hover clearing and
re-hover with a button held. Its release compares link text/range/position rather than travel. Starting near a cell
boundary and returning to an adjacent cell of the same link within four pixels addresses its unchanged-cell hover cache.
DOM-move, hover/provider and native release ordering remain unexercised; this is a conditional source mechanism.

Ordinary gates remain: selection, scheme and applicable disclosure checks, current ready file hover, release modifier,
opening offer, size/type policy, complete no-clobber save and final extension recheck. No bypass of those gates is
established. Retire pending press evidence when observation loses ownership, preserving the object captured by
providers. The snapshot test begins its gestures after restoration (`e2e/tests/terminal-reconnect.spec.ts:790–833`).

Suggested bucket: provisionally **highest**. Confidence: **confirmed retained state/observer gap; likely activation only
if the unverified event/hover sequence occurs**. Default md/txt/json opening alone establishes no security or work-loss
outcome. User-added HTML/other admitted types give a conditional active-content route described by final
`SPEC.md:3118–3132`; no harmful local-service endpoint, secret exposure, native execution or work-loss chain was shown.

Possible cover: final `SPEC.md:1833–1834` rejects drag activation; `SPEC_impl.md:1293–1310` and ledger
`TRIAGE_OUTCOMES.md:16934–16951` restore observation without accepting held gestures. The earlier click-not-hard-line
decision at `13537–13557` assessed narrower file opening and still ordered drag rejection. The vendor-only exception
does not cover Farhelm's listener lifecycle. No exact Planned, BUGS, queue or filter match was found. Proposed drop:
none for the narrow finding; fresh `audit-snapshot-gesture-authority` finds excluding possible highest **ambiguous**.

Provenance: `terminal-links-contracts-2/f3`, independently `terminal-links-implementation-1/f3`, and the fresh gesture
auditor in the trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was
performed.

Original finding IDs: `terminal-links-contracts-2/f3`, `terminal-links-implementation-1/f3`,
`audit-snapshot-gesture-authority/f1`.
