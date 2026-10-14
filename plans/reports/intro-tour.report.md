### What this was about

Farhelm opened directly on the session list without introducing the app or pointing people at its first-session guide
and private feedback. This plan adds the approved welcome card, with dismissal shared across the helm and a welcome item
in the ? menu for getting it back.

### Things you should know

Existing users see the card once after updating too. Escape, the close button, got it and Send feedback now mark it
seen; Get started opens the first-session guide without closing it. Send feedback now opens the ordinary feedback dialog
and sends nothing by itself. A failed preference read suppresses the card for that page load; a failed dismissal write
still closes it and may let it appear on a later load. Other already-open cards stay open until their user closes them.

The welcome's shared setting upgrades the helm database to schema 51. An older helm cannot open that database after the
upgrade. The supervisor database and helm–supervisor protocol are unchanged by this plan.

The first PR prepares shared storage; the second adds the card and keeps existing browser, capture and desktop-click
fixtures free of it through the preferences API. The first-session and Send feedback documentation pages mention it. The
release brick-test procedure names its dismissal during fresh startup. No screenshot publication or website deployment
was performed.

### Open questions and possible follow-ups

None required for this plan. The card uses the approved text and existing modal facilities; a guided tour remains
outside this scope. Native Mac desktop interaction and release brick testing were not run during development.

### The PRs

- [#2014](https://github.com/scode/farhelm/pull/2014/changes): shared welcome dismissal setting, draft, based on main.
- [#2015](https://github.com/scode/farhelm/pull/2015/changes): welcome card, help menu, fixture seeds and documentation,
  draft, based on #2014.

### Checks run, reused and skipped

- Recorded storage/API/preference and migration units: 51/51 passed, run `8a25c0b4-42cb-4f73-bff8-f3201827a935`.
- Recorded final UI preference queue and help-menu units: 18/18 passed, run `13f8e3b8-a714-43e8-be1a-9cfab6c1cfeb`.
- Browser integration selection on Chromium and WebKit: 57/58 passed in run `8808df28-c75b-492c-bb75-e93ffdb91ef4`. The
  WebKit recovery fixture correctly reached sign-in but Playwright retried its intentionally removed sort control. The
  fixture now dispatches the change once and observes sign-in. The final focused welcome selection passed 22/22 on both
  engines in run `a74373a3-38ea-47b9-863b-f825708b83e0`, including the corrected recovery case, shared dismissal,
  initial sign-in, terminal focus, failure behavior, links, feedback handoff and final layout. Normal and short-window
  card images were inspected from both engines.
- Reused the passing feedback, menu, tooltip, sidebar and affected sort cases from that mixed run: the final changes
  only scope welcome layout and reuse quiet initial focus. The narrow welcome run covers that final delta. The broad run
  used the prior built bundle throughout; the final bundle was built only after it finished.
- Recorded asset-JavaScript regression scenarios: 244/244 passed, run `a0461de4-e9aa-4dc4-ac34-3a19b2b582da`.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, shipped-binary Clippy, desktop renderer
  compile, explicit release web build, website build, changed-document `dprint check`, changelog format and shell syntax
  passed. Warning-level shellcheck passed; full shellcheck still reports existing informational findings in unchanged
  parts of the desktop smoke script. The isolated source-delay checker found 299 delays and zero unannotated calls.
- A first unit attempt did not compile because the new test used a nonexistent fixture constructor; corrected before the
  passing run. A first browser attempt could not find its image's preinstalled engines under the private home; corrected
  browser discovery before runtime validation. The unused write variant caught by review also failed an early Clippy
  run; the corrected standalone storage PR passed. These failed observations remain retained privately.
- Full workspace runtime tests, the full browser suite, desktop runtime smoke and release brick tests were skipped:
  selected tests cover the new shared setting, startup/authentication/focus behavior and its integration with existing
  fixtures; the desktop change compiles through the native renderer and reuses the existing external opener. No live
  installation was touched. Capture images were not refreshed: all four capture/test configurations seed seen through
  their shared setup, and this plan requests no welcome screenshot publication. No standalone TypeScript typecheck is
  claimed; this harness has no TypeScript compiler dependency, and Playwright executed the selected TypeScript cases.

### Review gate

Each PR received the required fresh-context gpt-6.1-sol high correctness, design, idiomaticity and test-quality review.
The storage review caught unused client write plumbing that prevented its independent lint gate; that plumbing moved to
the card PR. The card review caught initial terminal focus, a repeated welcome decision on sign-in recovery, two
fabricated preference snapshots, second-client isolation and build stamps on refusal fixtures. All were fixed, and the
same reviewer passed the correction, layout/focus-helper and final test deltas. A weak immediate negative-tooltip
assertion was removed after review; it claimed nothing about a delayed appearance. The required scope reassessment found
no unnecessary complexity or material scope expansion. Source documentation received a separate pass. Commit and PR
wording received cold reads.

Implementation stayed with the executor; only the required reviews and prose cold reads were delegated. Private
accounting records retain reported external review counters; native/orchestrator usage and exact model/billing
attribution remain incomplete. They are available with the executor's private working log, not in public source.
