# Very short windows may hide the host choices

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

In a sufficiently short window, the fixed footer may cover All and the host avatars. Whether keyboard focus provides
ordinary recovery remains unverified.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. No rendered failure or security or
user-work-loss consequence was established.

`crates/farhelm-ui/assets/app.css:622–635,679–692` positions the rail below the 40px title band (48px with ordinary Mac
window chrome) and reserves 128px underneath it. An independent footer occupies the bottom 128px at z-index 5, while the
sidebar clips overflow. With 160px of available sidebar height, that footer starts at y=32 and the rail at y=40. The
rules remain at pinned tip `9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

The reviewer inspected the CSS constraints, control placement, clipping, native window construction
(`src/desktop.rs:288–331`) and frame restoration (`src/desktop/window_state.rs:335–367`). No configured minimum height
or alternate short-height rail layout was found. These checks do not establish engine hit testing or whether focusing a
covered control scrolls it into reach. A short-viewport pointer and keyboard observation would settle that missing
premise. No browser, native-window or runtime experiment ran.

SPEC.md:1483–1492,1571–1575 requires the host choices and keyboard scrolling. Existing short-window records concern
other surfaces. FILTER.md's scrollable-dialog exception does not cover controls that cannot be reached. No exact
Planned, unlanded-plan, BUGS, queue or ledger coverage was found. Possible cover: none. Proposed drop: none. If the
geometry is reproduced, a shared scrolling fallback is one possible repair.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g003/g003-f002`.

## Additional review at d6645f77

Additional reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

The expanded navigation change grows the footer reservation from 128px to 168px, narrowing the same short-height
reachability budget by another 40px. This is an extension of the existing footer/rail cause, not a new queue item.
Engine hit testing, focus-driven scrolling and keyboard recovery remain unverified after source inspection. The separate
horizontal minimum-width conflict is recorded independently. Suggested bucket remains other.

Source and provenance, run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`:

- `g176/f1`: `crates/farhelm-ui/assets/app.css:633-644,809-825`.

Proposed drop: none for the retained remainder. No runtime reproduction, fix or triage decision was performed.
