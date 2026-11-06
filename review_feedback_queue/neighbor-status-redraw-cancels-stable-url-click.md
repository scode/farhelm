# Neighbor status redraw cancels stable url click

Reviewed commit: ce77ce6c19c6bbd892f199436c4c3acdd79ef36b

## TLDR

A status change elsewhere can cancel a click on an unchanged terminal URL.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/assets/grid-buffer.js:397-398 at ce77ce6c19c6bbd892f199436c4c3acdd79ef36b`.

Source assessment (read with the later corrections below) (review record g075/l8, event 17):

Reviewed commit: ce77ce6c19c6bbd892f199436c4c3acdd79ef36b. TLDR: clicking an unchanged web address can silently do
nothing if a separate neighboring status/progress line redraws while the mouse button is down. Details: the patch adds
both boundary rows to every printed URL's evidence unconditionally (grid-buffer.js:397-398,408). Use a standalone URL
row with wrapped=false, preceding blank unwrapped row, and following progress row. urlWindow returns only the URL row:
the previous row is not wrapped (516-517), and forward scanning stops on the unchanged URL row's !wrapped flag
(529-532). Editing only the following row's progress text therefore cannot alter the recognized URL, its cells or range,
yet its row identity is captured as evidence by grid-terminal.js:1238-1242,1343-1345. Press captures the hovered object
(1693-1703); while held, refreshHover explicitly does nothing because gesture is set (1383-1384), even if the update has
already been painted. Release at 1619 rejects the click because hoverCurrent compares every evidence-row identity
(1247-1250), including that irrelevant neighboring row. Then lookup refreshes only after the lost activation
(1624-1626). This can be reproduced deterministically by hovering such a URL, mouse-down, delivering and flushing a
row-only update to its unwrapped next status row, asserting the URL row unchanged, then mouse-up: no opener call occurs.
It does not require an unpainted-screen timing window or any URL change. Confidence: confirmed from source control/data
flow, no runtime test executed. Suggested bucket: other (lost clicks/retries, not security or data loss; frequency
depends on output and click duration). proposed_drop: false. possible_cover: not searched, parent-owned. This is
distinct from g028:l3/l6 prepaint wrong-target concern, g028:l4/g029:r1 stationary OSC warning lag, and g011:g011-l1
file-token activation refutation. The sub-second before-update acceptance does not refute this sequence because the
progress update can be fully painted during a held press while the clicked URL never changes. Existing
relabel-during-press test (grid-attach.spec.ts:1267-1293) correctly rejects changed link text but does not cover
unrelated boundary updates; the assigned added test covers target extension after repaint, not this false invalidation.
Suggested fix: retain safe invalidation when neighboring content can change the URL, but avoid canceling a press solely
for an irrelevant boundary's content change; track boundary predicates separately or revalidate the recognized
URI/ranges before preserving an unchanged plain-URL gesture. Do not preserve a changed target or waive required
disclosures. Temporal qualification: new boundary evidence at ce77ce6c introduces this neighboring-row cancellation for
plain URLs; prior plain links captured their actual range rows only. No later repair of the relevant files appears
through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b in the already inspected path history.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g075/l8`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
