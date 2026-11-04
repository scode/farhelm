# Unverified after investigation: pending Replace can recreate its announcement

Reviewed commit: f288c370c16f9a9d7d32185054885cb3fbd61161

## TLDR

A screen-reader user may hear the same slow Replace announced again when its progress moves between visible surfaces
before the request ends. The new populated live region is established by source; repeated speech on a supported
browser/screen-reader pair remains unverified.

## Details

The shared progress component, introduced by `372c7ef7aa193093b9c918b5f478cc52dc111441`, creates a populated
`role=status` in `crates/farhelm-ui/src/launch_progress.rs:111–128`. Its announcement state belongs to the component
rather than the pending request. Two routes can replace the announcing surface while that request continues.

First, select session A and start a sidebar Replace that remains pending beyond two seconds. Another row's Replace B can
run concurrently: `list/view.rs:614–621,644–649` checks the operation's own source ID, and `957–966` checks a page token
without taking it. The list-owned tasks at `2585–2606` survive selected-view changes. B's success calls `on_open`
directly at `2594`, before ending B's row operation at `2606`. That bypasses the manual-navigation guard at `3040–3046`.
A's selected view unmounts while its request/progress remain, and `list/row.rs:2729–2733` changes A's announcement prop
from false to true. The progress component then inserts another populated status region for A. The row comment claiming
stable announcement ownership from navigation locking does not cover this other-row success route.

Second, session-detail reads continue during Replace (`session_view.rs:943–975,1295–1302`). The helm can return a cached
stale session (`crates/farhelm-helm/src/sessions.rs:2616–2639`). The stale-first terminal classifier at
`session_view.rs:3476–3482` changes the interrupted-card condition at `1825–1827`, moving progress between separate
card/header component sites at `2506–2516,2655–2670`. Those visual copies likewise lack request-owned announcement
continuity.

`SPEC.md`, “Creation,” 1005–1010 calls for one polite announcement when the status appears, rather than one for each
elapsed second. `SPEC.md`, “Lifecycle operations,” 1464–1469 shares progress across these surfaces. Whether that
requires one announcement per pending operation or permits one per visible reappearance remains unresolved. Elapsed-time
updates are not alleged to announce every second.

A stable announcer or request-owned announced state could preserve continuity while visual copies move. To settle the
user-visible consequence, hold selected A's sidebar Replace beyond the progress threshold, complete B first, and observe
A's new row announcement with a supported screen reader. An equivalent stale-detail transition tests the other route. No
such runtime or assistive-technology observation is claimed.

## Confidence and priority

Confidence: likely accessibility concern, retained **unverified after investigation**. Source establishes the DOM
transition and no relevant later repair through `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`. Missing premises are whether
supported browser/screen-reader combinations speak the new populated region and the observed live overlap/timing. No
supported screen-reader observation or live overlapping-request trace is available.

Suggested bucket: `other`. No duplicate mutation or lost work is established. `could_be_highest: false`.
`proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

Grouped findings: `g057-l3` is the original finding and `g056-l5` supplies the concurrent-row trigger for the same
announcement-continuity concern. Both triggers and the AT interpretation uncertainty are retained. The g041
test-evidence findings are unrelated.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`: owner `g057`, ID `g057-l3`, latest `g057/findings.jsonl` physical
line15; owner `g056`, ID `g056-l5`, latest `g056/findings.jsonl` physical line9. The latest g057 event independently
checks the added concurrent-row trigger while preserving the speech and interpretation caveats.
