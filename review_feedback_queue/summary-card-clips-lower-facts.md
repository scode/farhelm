# A tall session summary hides its lower rows

Reviewed commit: 51fd667d28f3a7ffdec09c6b2f6ae3c2ad220bb6

## TLDR

In a short window or with lengthy session details, the hover summary can cut off its directory and timestamps with no
way to reveal the hidden portion.

## Details

The summary inherits the shared tooltip's viewport-height cap, `overflow:hidden` and `pointer-events:none`
(`crates/farhelm-ui/assets/app.css:1418–1440,1458–1467`). Its header can contain eight title lines and a wrapping agent
line, followed by wrapping host, checkout, directory and timestamp rows (`app.css:1470–1548`;
`crates/farhelm-ui/src/list/summary_card.rs:84–147`). If these exceed the cap, complete lower rows become unreachable;
there is no scrolling. This is separate from deliberately shortening the title. Opening the session offers other access
to its directory, so loss of user work is not established.

To verify, render a managed session with long title, host and directory values in a roughly 360×320 window, then compare
the card's scroll and client heights and the lower rows' rectangles with the clipping rectangle. A sufficiently long
directory can also exceed a normal window's height. The promised facts should remain reachable.

Suggested bucket: **other**. Confidence: **likely**; the CSS mechanism is confirmed, but the rendered threshold has not
been reproduced. Provenance: same-model adversarial trailing review of the stated commit. Proposed drop: none.

Possible cover: `SPEC_impl.md:836` accepts an eight-line tooltip clamp generally, but does not clearly accept
unreachable whole rows in this new card. `SPEC.md:1391–1394` promises the full directory and session facts there. No
exact Planned item, known-bug acceptance, queue item or ledger decision was found. The filter for dialog explanations
reachable by scrolling does not cover this noninteractive tooltip.
