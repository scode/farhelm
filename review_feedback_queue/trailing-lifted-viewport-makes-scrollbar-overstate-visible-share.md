# The lifted viewport makes the scrollbar overstate the visible share

Reviewed commit: 77b66797b218a8dae046f1d9d3da76386d6b5a60

## TLDR

The scrollback thumb can appear larger than the portion of output actually visible when a status or exit line lifts grid
rows.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/terminal.rs:504`.

terminal.rs 492–511 counts all grid rows as visible and adds lift padding to the navigation total. Yet attached.rs
1812–1826,2116–2138 removes lifted rows from the painted page. For N grid rows, H reachable history/notice rows and L
lifted rows, the supplied fraction is N/(H+N+L), while the visible share is (N-L)/(H+N). The supplied fraction is larger
with any positive lift before minimum size and rounding. components/scrollbar.rs 4–9,43–60,64–89 defines the thumb's
height as the visible share and uses the count directly. The endpoint travel is still correct because total minus
visible remains H+L. Existing attached/tests.rs 1333–1385 asserts nonzero lift but checks wheel/selection; 3047–3059
checks scrollbar reachability without checking thumb height. components/tests/scrollbar.rs 18–46 tests a correctly
supplied fraction in isolation. The prior code already counted all grid rows with a notice, and the new padding changes
range and denominator without using physical visible rows. Report physical visible count and a matching total while
preserving the scrollable travel. Bucket other for misleading control geometry, with no assertion that either endpoint
becomes inaccessible. Confidence confirmed by static arithmetic and existing geometry assertions. No rendered thumb
observed. Possible coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l20`.
