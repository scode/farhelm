# A narrow outage fallback may lie outside reachable scrolling

Reviewed commit: f000780292bd39fed96423cd97243c1f49c21f3d

## TLDR

A very narrow, short viewport may put part of an outage explanation beyond the left reachable edge. Actual supported
sizing and recovery geometry remain unverified; this differs from accepted overlap that remains scrollable.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-ui/assets/app.css:7225`. Locations and revisions in the assessment below identify the
reviewed source; later main is outside this review's pinned range.

Later-turn synthesis, reviewed commit f000780292bd39fed96423cd97243c1f49c21f3d. Retained other, likely with unrendered
sizing/reachability premise. Root .window-root and html/body/#main rules (:403-405,:461-469) impose height, not a
viewport-width minimum. The 320px minimum belongs to .app-main (:654-663); .app-shell scrolls horizontally (:554-563),
so that child minimum does not make 100vw at least 320px. Searches found no Rust min_inner_size or HTML viewport minimum
in the pinned source. At widths below 320 CSS pixels, :7225 produces a negative viewport-fixed left edge, while right:0
from inset:0 (:7187-7198) creates an outer overlay width of 320px. A sufficiently wide .session-unavailable-card can fit
the overlay's inner width (:7200-7212), extending text left of the physical viewport without creating overflow inside
its own overlay. Ordinary shell scrolling does not move viewport-fixed content, and html suppresses ordinary document
scrolling (:471-496). Exact engine shrink-to-fit/card width and focus-induced programmatic recovery remain unverified;
no rendering or experiment was attempted. The browser case is enough; no claim that every native platform permits that
width. possible_cover SPEC_impl.md:554-562 expressly accepts overlapping the sidebar to retain 320px, distinct from
unreachable left-side text. SPEC.md:1489-1502 requires horizontal/keyboard reachability, and
TRIAGE_OUTCOMES.md:16767-16785,18645-18656 records reachable explanation fixes. These are not blanket acceptance. FILTER
only excludes small-dialog clipping when scrolling still reaches it. proposed_drop null. No established highest
consequence.

Provenance: `g279/f3`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
