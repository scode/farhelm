# A completed bulk action sends typing to sorting

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other. Final bulk cleanup still calls
`list/marking.rs:219–223` from `list/view.rs:4576`, targeting `.sort-title` after the toolbar disappears.
`SPEC.md:3260–3267` still makes the selected terminal home. The Templates-close item is a different surviving disabled
target. No exact later repair or cover was found.

## TLDR

When a completed bulk action removes the focused toolbar, keyboard focus can move to the list's sorting heading instead
of the selected session's terminal, even when the user waited without pressing anything.

## Details

Bucket: **other**. Confidence: **confirmed source destination/contract mismatch**; browser scheduling was not exercised.
Original suggestions were other. Reviewed source change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`; its parent already
had the sort-heading destination. The later reviewed-tip focus rule makes the required terminal destination explicit.

At the reviewed tip, `crates/farhelm-ui/src/list/view.rs:4191,4238-4271` captures the marking revision, preserves newer
mark/menu/question gestures and removes only the completed batch's targets. When that removes the effective toolbar it
requests `return_header_focus_after_async` before updating marks. `list/marking.rs:176-195` captures toolbar-owned
focus, waits for the source to disconnect and body to own focus, then explicitly chooses `.sort-title`.

Those ownership guards do not refute the wrong destination: no intervening input is required. `SPEC.md`, “Keyboard
focus”, “Where focus rests”, 3233-3238 requires the selected terminal whenever nothing holds focus on the main screen
without a popup. `SPEC_impl.md:1172-1177` acknowledges the missing terminal-home behavior. Correct the destination while
preserving guards against taking focus from a surviving control or newer interaction.

The bulk-question confirming-close rule at `SPEC.md:2196-2197` returns to the marked count while its toolbar survives;
that is a different transition. The Templates-close/disabled-bulk-button queue item also concerns another owner and
trigger. At recording-time main `cb03def00870e18a98a1fd9046073d2849be3253`, the broad focus TODO has been removed for an
expected GUI rewrite. Four older focus findings were discarded under their specific removed-control, reselection and
terminal-sync scopes; that decision does not match this bulk-removal destination. No exact current queue/ledger, Planned
item, unlanded plan, accepted bug or filter cover was established. This needs a separate repair from `async-body-focus`:
changing moved-on admission alone leaves the explicit sorting destination intact.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `bulk-toolbar-focus`; `g010/l6`, `g006/l2`. Original
source checks and consequence separation remain in `prep/groups/<group>/findings.jsonl`.
