## What this was about

An open session cycled through red and amber warnings when its supervisor went away, including during an update, then
lost its terminal display. The maintainer approved one calm card naming the cause, smaller diagnostics, a dimmed last
screen, and neutral session-list marks. This applies to supervisor unavailability generally; it does not track updates.

The session now keeps its last screen and tabs visibly inactive behind the card. When the host returns, it retries
promptly without taking control from another window. A session first opened while its host is unavailable still has no
terminal to display. A host reboot still leads to the interrupted-session view.

## Things you should know

The approved retained-screen design was implemented; the empty-screen fallback was not taken. The card distinguishes
temporary connection trouble from causes such as a host needing an update, a host newer than the helm, an identity
problem, or a removed host. The helm's remedy is preserved verbatim. Browser connection trouble has the same card with
its own diagnostics and a neutral reconnect button; that button is absent when retrying from the window cannot help.
Last-known status and age remain in one band. Session-list marks use the same cause, with a grey, still status dot.

Review found and corrected two recovery orderings: a delayed replay callback could cancel a newer retry after its socket
died, and replay finishing while the old screen was inactive could leave keyboard focus unrestored. Focus recovery now
preserves other deliberate keyboard owners, including fields and dialogs. A separate competing-window test confirms that
automatic recovery yields to the window that took control during the outage.

An isolated real SSH update took about 35 seconds. Sampling the helm's connection state during that fixture observed two
gaps, roughly 70 milliseconds and 1.35 seconds. These are observations from one test, not an outage-duration guarantee.
Temporary measurement code was removed before review and is absent from the PR. No live installation was changed.

The incidental documentation update is on **Work in a session**, under **When the connection drops**. No preview server
was started during this unattended run.

## Open questions and possible follow-ups

No product decision remains open. Browser evidence uses Chromium and WebKit on Linux; it does not exercise the native
Mac desktop shell. Existing host-update progress and failed-update presentation were left unchanged.

## The PRs

- [#2198: calm session outage cards](https://github.com/scode/farhelm/pull/2198/changes). Draft head
  `95913f06f9395addd830c571b3c0ab57dbc7d713`, bookmark `plan/calm-session-unavailable/01-outage-card`.

## Checks run, reused and skipped

- Replay/rename and unavailable-host/refusal/reboot integration passed all 58 cases, 29 per engine, with zero skips in
  run `1af9034c-87fc-43d3-a2ca-d071215b9c00`. Selection:
  `npx playwright test 'terminal-replay-rename.spec.ts|terminal-multihost.spec.ts' --grep 'reattach-lands-at-tail|replay-|reveal-does-not-steal-focus|unreachable-host-goes-stale|stale-session-metadata-view|op-refused-on-unreachable|a refused Stop|reboot-'`.
  The replay expression matches that spec's filename, so this covers the complete replay/rename spec, including its
  focus-preservation cases, plus seven multihost cases per engine. All browser recorder reports were complete; retained
  output showed no runtime substrate skips.
- The new delayed-replay-callback regression passed in both engines in run `7d340361-9e3c-49fd-9473-816a9856dd90`. That
  run also had four failures in the outage tests' focus premises, two per engine: one assertion incorrectly expected
  focus to leave the compact checkbox, and the dedicated focus test had not established initial terminal focus. After
  correcting only those test premises, all four outage cases passed with zero skips in run
  `607ef26b-1604-448d-9a0f-6ae8ceb58a68`, including replay revealing before fresh detail and click-free focus recovery.
  Selections:
  `npx playwright test 'terminal-reconnect.spec.ts|terminal-multihost.spec.ts' --grep 'late-replay-flush|supervisor-outage:'`,
  then `npx playwright test terminal-multihost.spec.ts --grep 'supervisor-outage:'`.
- The complete reconnect spec passed 50 cases with zero skips in run `3a1c28bb-8a78-4ba3-895f-e726008090f9` before the
  two review fixes. It covers the retry ladder, background probes, heartbeat, build mismatch, manual recovery and
  competing ownership; the later targeted tests above cover the corrected orderings. Selection:
  `npx playwright test terminal-reconnect.spec.ts`. The final competing-owner test also passed both engines separately
  in run `1c160a2d-8ec0-4330-b6d5-aec4ec562c43` before the final portable browser-side state comparison, which the
  complete reconnect run includes.
- The UI unit tests passed 509 cases with no skips in run `74451d6c-d9f4-4b13-9b8f-5cb3788fc777`. Reused for the final
  patch because the later functional corrections are in JavaScript; Rust changes afterward only adjusted comment and
  macro-token indentation. The final JavaScript unit harness passed 261 cases with no skips in run
  `1684e05a-bdc9-46c7-a0ca-475fefbf2552`. Commands: `cargo nextest run -p farhelm-ui --lib`, and from the UI JS-test
  directory, `node --test`.
- The isolated real SSH update test passed in run `9949f3d3-0ae9-4b25-a4f0-c4a93281818a`, with no runtime substrate
  skip. Its earlier attempt `65d8beee-6beb-4e6d-a206-1b81bc991cd3` timed out during compilation before executing a
  fixture. Selection:
  `cargo nextest run -p farhelm-helm --lib -E 'test(=provisioning::tests::provisioning_and_update_over_ssh_preserve_an_operable_session)'`.
- The pre-change supervisor-restart reproduction passed both engines in run `fb4b97d1-68ff-4608-a742-44836d1d8c73`. It
  observed the red banner, amber retry surface, terminal removal, host notices and eventual remount. A preceding
  diagnostic run `529201be-c1aa-4967-8d9d-51a332e57dc6` failed an ambiguous terminal-count selector before stopping the
  supervisor. Later same-session browser failures were retained: `88b283ae-becb-4a25-8910-36669516910b` exposed an
  incorrectly present inert attribute and was interrupted after diagnosis; `0f566457-308b-4578-8f3d-7f810e328d24` passed
  both real-outage cases but failed two takeover selectors; `dc66aea4-377c-40b0-b23d-06e94ea15862` exposed a takeover
  fixture that had left its shell channel connected. The attribute and test premises were corrected before the later
  passes. These retained failures remain separate observations, not latent-flake claims.
- `cargo check -p farhelm-ui --features desktop`, `cargo clippy --all-targets -- -D warnings`, the CLI/fixture build,
  the final Dioxus web build, and the website build passed. The website build rendered 32 pages. The Rust compile/lint
  checks precede the final JavaScript-only corrections. Main's intervening host-plan/trash keyboard-access changes
  affect separate components and spec/CSS sections. A final rebase also incorporated the input-method guard from PR
  #2194. That listener explicitly leaves xterm input alone; its composition history changes neither replay nor focus
  handoff, and these tests generate no composition events. The reviewed patch is unchanged across the rebase, so the
  targeted runtime evidence is reused without a repeat. The only textual conflict was adjacent TODO removals; both
  completed entries remain removed. The new guard's own tests are outside the 261-test count above.
- `cargo fmt --all -- --check`, `dprint check`, the isolated test-sleep checker (302 delays, no missing rationale),
  `python3 scripts/check-agents-md.py`, and `python3 releasing/check-changelog.py format` passed. The formatter has no
  JavaScript/TypeScript plugin, so it supplies no formatting claim for those files. The final documentation pass covered
  every changed file.
- Full workspace runtime, desktop runtime, installer, distribution and full-browser checks were skipped because the
  targeted tests cover the changed rendering, replay, focus, ownership and host-recovery boundaries. No CI run was
  requested or triggered.

## The review gate's outcome

One fresh-context gpt-6.1-sol high source reviewer examined correctness, design, idioms and the full test-authoring
checklist. Its two initial findings were fixed locally. Corrective review and a final review of the test-premise delta
found no remaining actionable issue. A fresh review on the executing session's inherited model assessed necessity and
scope and found no unnecessary complexity. The executor checked the cited mechanisms and owns the runtime evidence; the
source and scope reviewers ran no tests. A fresh gpt-6.1-sol medium reader passed the commit and PR wording.
Implementation work was not delegated.

### Landing

Landed in round 77, as #2198, first of the round's two plans and unchanged from what the report describes.

#### What landed since the PR was based, and whether it interacted

Since this PR's base, main gained the cancelled-download fix (#2197), which changes the helm's download bookkeeping, and
three specification-only changes (#2193, #2195, #2196) about Claude settings edits, stale views across GUIs and the
trash's scale limit. The round's other plan, the explicit bare-repository policy fix for checkouts (#2199), lands right
after this one. The landing monitor read them against this PR, and a fresh-context reviewer did the same independently.
Neither found an interaction:

- The new stale-views rule (#2195) covers a view that misses a change saved from another GUI. The outage card reacts to
  the host becoming unavailable, which is a different event. This PR keeps SPEC.md's sentence that changes from any
  client appear in other clients automatically, one of the live-update rules #2195 preserves, and its automatic retry
  still never takes a session from another window.
- The Claude settings (#2193) and trash (#2196) changes touch neither the session view nor host outages.
- The download fix only changes what the helm cleans up when a file download is cancelled before the host accepts it.
  During an outage the panes are inert, so no new download starts from the frozen screen, and one already in flight
  fails when the connection dies, as before.
- The checkout fix touches only how the supervisor creates checkouts. A failed checkout still shows as the existing
  failed-checkout state, never as the outage card.

The landing rebase was clean, and the PR's diff is identical to the one the executor delivered. Its SPEC.md and
SPEC_impl.md edits merged without conflict alongside the three specification changes above.

#### Fixes made during the landing

None.

#### Documentation

The PR updates the docs page **Work in a session**, under **When the connection drops**. No preview server was started.
The reviewer noted three small points on that page, none blocking:

- It drops "The agent keeps running on its host the whole time" in favor of a link to the page on what survives what,
  which is still a stub, so readers currently lose that reassurance. Dropping it is defensible because a host reboot
  does end the agent.
- "Reattaches by itself" leaves out SPEC.md's two exceptions: it will not take the session back from another window, and
  a session whose host rebooted shows its interrupted card instead.
- The page writes the button as "Reconnect now" (capitalized because it now starts the sentence), while the interface
  shows "reconnect now".

One leftover in SPEC_impl.md: its paragraph on viewing session details during an outage still says they are viewable
"behind the host-unreachable notice", a phrase SPEC.md no longer uses.

None of these was fixed at landing, because they are wording choices rather than errors that break anything. A small
documentation follow-up could restore the reassurance that a supervisor outage leaves the agent running (until the page
on what survives what is written), mention the two reattach exceptions, and change the SPEC_impl.md phrase to "behind
the outage card".

#### Checks

- Run now, at the rebased tip: `dprint check` on SPEC.md, SPEC_impl.md and TODO.md, and
  `python3 scripts/check-agents-md.py`, both clean, to cover the three specification changes merged alongside it.
- Reused, because the landing rebase left the PR's diff unchanged, and what joined its base is either specification-only
  or, for #2197, confined to cancelling a file download in the helm, which none of these tests exercise: the passing
  runs the report lists, including the outage and replay browser run `1af9034c-87fc-43d3-a2ca-d071215b9c00` (58 passed),
  the corrected outage cases `607ef26b-1604-448d-9a0f-6ae8ceb58a68`, the full reconnect spec
  `3a1c28bb-8a78-4ba3-895f-e726008090f9`, the UI unit run `74451d6c-d9f4-4b13-9b8f-5cb3788fc777`, the JavaScript harness
  `1684e05a-bdc9-46c7-a0ca-475fefbf2552` and the real SSH update test `9949f3d3-0ae9-4b25-a4f0-c4a93281818a`, plus its
  builds and static checks. The report explains which of those ran before its last JavaScript corrections and why they
  still apply.
- Skipped: everything the report skipped, for the reasons it gives. The Mac desktop app was not exercised.

Nothing in the executor's report became untrue at landing.
