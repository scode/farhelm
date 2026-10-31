# Rail tooltips merge separate direction boundaries

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A composite rail tooltip can group host and destination text differently from their separately isolated visible values;
actual right-to-left rendering remains unverified.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The inspected composite text boundary differs from the individual value isolates. Relevant escaping, tooltip ownership
and operation-ID guards were checked. A concrete supported directional-name/font rendering and consequential ambiguity
remain unobserved. The launcher target-hover record concerns a different field combination.

Source locations: `crates/farhelm-ui/src/hosts.rs:666-670,704-742; crates/farhelm-ui/assets/tooltip.js:297-310,332-339`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis at reviewed d6645f771bee8ed03a66486ef8755834937d759d retains f1 as OTHER investigated uncertainty. No new
native rendering evidence was supplied or produced. Primary rechecks confirmed raw name and destination become one
escaped formatted string; expanded names/destinations remain separate dir=ltr peer-value spans (hosts.rs:769-773),
whereas the ordinary tooltip writes one textContent run and CSS isolates only the whole tooltip
(app.css:1778-1800,5199-5215). display_peer at peer.rs:159-181 preserves ordinary RTL letters, and validators at
hosts.rs:3302-3313/store.rs:1597-1615 accept ordinary RTL aliases, so the lead is not limited to rejected input or
unsafe control characters. The same formatted string is an aria-label, but no speech/accessibility consequence was
established and none is claimed. Exact missing premise is an accepted remote host name/destination pair, state text and
supported native tooltip geometry that actually changes the apparent grouping or role of those values relative to
separately isolated full labels. Source confirms boundary removal but cannot supply the unobserved engine layout; this
pass forbids native/browser experiments. A retained native capture or focused rendering observation for that
pair/geometry would settle presentation; a stronger bucket needs an additional established harmful action route.
Captured/forwarded IDs refute the inspected dispatch-key hypothesis (f2), not visual grouping. The newly confirmed
literal-local-alias rewrite (f3, duplicate of g144/f1 and g145/f1) is a different deterministic transformation and does
not verify or refute this composite RTL concern. proposed_drop none. possible_cover remains ambiguous:
SPEC_impl.md:1185-1217 permits escaped textContent tooltips and landed host-rail-list-visible.md:138-143 proposes the
name (destination): state format, but neither explicitly waives per-value grouping under SPEC_impl.md:1081. Related
queue launcher-target-hover-merges-direction-runs.md concerns agent/host/folder on the launcher, not the rail
name/destination/state; read fully and do not deduplicate solely by formatter mechanism. The initial coverage search of
SPECs, empty TODO Planned, queue plans, BUGS, INDEX and TRIAGE_OUTCOMES found no exact coverage, and synthesis re-read
the matching specification/plan/queue sections. Confidence: confirmed source difference; suspected presentation
consequence unverified after investigation. No security or user-work-loss route established. Introduced by
0ea8f3872ccc53b3c1f02e9c4d9e2fb2e9e531ca during adjacent implementation necessary for the assigned tests.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g146/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
