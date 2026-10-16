# Repeated honest links acquire false mismatch warnings

Reviewed commit: e0586c292df953311a3460e39521168838aa0e59

## TLDR

Printing the same honest web link twice on consecutive terminal rows can make both links warn that their text names a
different destination.

## Details

Suggested bucket: `other`. Confidence: confirmed by static tracing; no browser reproduction. Source: separate
adversarial trailing reviewer. Proposed drop: none.

`crates/farhelm-ui/assets/terminal-links.js:448–468` joins adjacent rows carrying the same hyperlink tag without
distinguishing continuation from another complete occurrence. Emit an OSC link with `id=repeat`, URI and label
`https://example.com/`, close it, move to the next row, then emit the same id, URI and label. The bundled xterm
`OscLinkService.registerLink` (`assets/vendor/xterm.js:1`) reuses the tag for its existing id/URI key. The helper
returns `https://example.com/https://example.com/`, which `linkTextMismatch` at `322–339` compares with the single URI
and warns. Previously each row matched.

Verify against the bundled terminal, checking shared tags before hovering both complete occurrences. Preserve the
intended joined comparison for actual continuations when distinguishing repeated complete links.

Possible cover: SPEC_impl.md:1088–1095 prescribes adjacent same-tag joining, but does not expressly accept this false
warning. `plans/reports/wrapped-link-warning.report.md:102–106` at `4cc3065446357e84eaacea051905d82ff8766805`
acknowledges the possibility without resolving it. Reports are not coverage sources. No matching Planned item, BUGS
entry, queue item, ledger decision or filter was found; repetition need not be a rare timing event.
