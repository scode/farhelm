# Host actions disappear at supported sidebar widths

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A narrow sidebar could hide the actions needed for an unreachable host.

## Details

F98 — **possible** — `crates/farhelm-ui/assets/app.css:3937` — Host actions disappear at supported sidebar widths

A long unreachable-host status can consume the row's width and push its trailing actions toggle outside the supported
narrow sidebar. Retry, Settings, and Remove then become hard to discover or use with a pointer. Rendered geometry
remains the material unverified premise; keyboard access may remain and widening is a workaround. Reserve room for the
toggle by allowing status content to shrink or wrap at the supported minimum width.

## Evidence and triage context

- crates/farhelm-ui/src/hosts.rs:2896 renders the locality icon, name and status before the actions button at line 3014.
- crates/farhelm-ui/assets/app.css:3861 establishes a nowrap row; line 3913 gives the name a 4em minimum; line 3943
  prevents status shrinking; line 1805 prevents button shrinking.
- crates/farhelm-ui/assets/app.css:550 fixes sidebar width and clips horizontal overflow.
- SPEC.md:935 supports sidebar widths down to 240px.

Retained confidence: **possible**. Suggested bucket: **high**.

Possible cover:

- No matching Planned item, BUGS entry, open queue item or accepted ledger disposition found. Whole-shell horizontal
  scrolling does not expose content clipped inside the sidebar.

Caveats:

- Original Chromium fixture measurements were not independently reproduced.
- Keyboard access may remain possible; the established concern is visibility and pointer access.
- Widening the sidebar is a workaround.
- The original isolated Chromium measurements were not independently reproduced.
- Do not claim that keyboard access is impossible.
- Widening the sidebar restores access.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_01_cor:p1:F1`.

- `ui_desktop_01_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
