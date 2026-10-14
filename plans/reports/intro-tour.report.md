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

### Landing

Landed alone in its round as #2014 then #2015. While it was landing, main gained #2016, which records review findings in
`review_feedback_queue/`. That change is documentation only and does not interact with this plan.

#### Release-relevant

The helm's database moves from schema 50 to 51 (the shared welcome-seen setting). As with every schema bump, a helm that
has run this build cannot be downgraded in place. The protocol is unchanged. Existing users see the welcome card once
after updating, as the report says.

#### Review

An independent fresh-context review found nothing blocking. It confirmed that:

- the migration matches a fresh database, and every rewound-schema test fixture also drops the new column;
- the browser suite, the README hero, the demo video and the docs screenshot setups all mark the welcome seen before any
  page opens, so no published capture can show the card;
- the shared reset helper marks it seen;
- Escape cannot close it twice;
- Get started opens the guide once on desktop;
- Send feedback opens the ordinary dialog and sends nothing.

#### Fixes made while landing

- The helm database's schema history had no entry for 51; it now does.
- The welcome browser tests touched only the welcome setting. The sign-in recovery case writes a list order through the
  real helm, which leaked "title" ordering into every later spec sharing that helm, and that case assumed the default
  order. The spec now resets every shared preference before and after each test.
- The brick-test procedure said the card appears only on a fresh app's first open. Upgraded helms start unseen too, so
  the upgrade checks will meet it. The procedure now says so.

#### Something the maintainer should know

A browser test on main fails independently of this plan: `terminal.spec.ts`, "compact rows retain distinct ended and
harness glyphs within two characters", on both engines. Its expected permission tooltips and descriptions predate the
guarded permission mode (#1992). The tooltip now also names the agent and command, and "default permission mode" is no
longer what a no-flag Codex launch shows. The guarded landing did not run that spec. It needs its expectations updated
to the current wording, a small follow-up outside this plan.

#### Other review notes left as they are

- Unverified on desktop: after the card closes, terminal focus is restored one animation frame later, and only if no
  dialog remains. If the native webview applies the close later than the script, the terminal may not get focus and
  needs a click. Worth a look on the Mac.
- If the browser has to sign in again while the first-open card shows, the card disappears without being marked seen and
  returns on the next load. That is within the spec's "may appear on a later load", but not stated.

#### Checks

- Run now:
  - `cargo fmt --all -- --check`, `dprint check`, the changelog format lint and the sleep checker;
  - `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
    `cargo check -p farhelm-ui --features desktop`;
  - the helm, UI and protocol packages in full through the recorder (pinned tmux, four slots, no retries): 1717 passed,
    none skipped, run `cf2df40d-1161-4540-8bd6-9c543ac06e50`;
  - the asset JavaScript harness (244 passed) and the website build;
  - on Chromium and WebKit, the welcome, feedback, sort, hover coverage, settings, session confirmations, quick launch
    and terminal specs, with the welcome spec already reset-corrected: 287 passed and 3 failed, run
    `26af5a00-7c86-4322-a804-32871158151c`, kept. Two failures were the stale terminal test described above, one per
    engine. The third was a single Chromium timeout in the session-confirmations walk.
  - the welcome and session-confirmations specs three times each on both engines: 72 passed, run
    `6e1d5915-a4bc-4420-a4e8-c87a40f5b04f`. The single timeout is treated as a load event on a shared machine, not
    logged as a flake.
- The schema history line is a comment only, added while the Rust run was underway; it changes no behavior.
- Not run: the full workspace battery, the full browser battery, the desktop runtime smoke and release gates. The plan's
  desktop smoke change only seeds the welcome as seen.
