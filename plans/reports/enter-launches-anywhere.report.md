# Enter launches from launcher choices

## What this was about

Enter could launch while a text field had focus, but stopped doing so after focus moved to a choice. An incomplete setup
often did nothing visibly: Launch was greyed out, and existing refusal messages were below the visible part of the
launcher. The maintainer chose apply-then-launch for choices, submission of the values shown for radios, checkboxes and
dropdowns, and a visible refusal beside Launch. They chose the same behavior for Restart with, including confirming its
displayed stop-and-restart action.

## Things you should know

In New session, Clone and Replace with, Enter on a tab, harness, effort, permission or trust choice applies it and uses
the ordinary Launch action. Enter on the host or agent-type dropdown, resume checkbox or YOLO radio uses the values
shown without changing them. Incomplete drafts explain an attempted launch beside Launch; opening an unfinished draft
alone stays quiet. The greyed-out button's hover text also explains what is missing. Existing submit and helm refusals
appear there too.

Restart with uses the resulting choices immediately, including the model. During execution the maintainer clarified that
Enter in its model field must choose the model and restart. New session's model field still chooses without launching.
Enter in Restart with's command fields or on a YOLO radio invokes the primary action with the shown values; unchanged or
invalid settings do not restart. When the button says "stop and restart", Enter confirms stopping the working agent
first. The request retains the confirmation displayed to the user.

Search, action buttons, the YOLO confirmation and saving a template keep their dedicated Enter behavior. Holding Enter,
or pressing it while composing text, cannot launch or restart. Keyboard focus stays inside Restart with during a
request, including the first choice that changes an otherwise unchanged draft. The covered TODO entry was removed.

## Open questions and possible follow-ups

No maintainer decision remains and no product follow-up is proposed. One development WebKit test timed out after sending
all six expected New session choice requests; its cause is unknown. The exact test subsequently passed on both engines
with the same timeout and unchanged product source. A separate WebKit YOLO test once showed a blank page during setup;
its cause was not established, and it subsequently passed narrowly. The reproduced row-paint startup deadline mismatch
is described under Checks. Failed and interrupted runs remain retained alongside passing evidence.

## The PRs

- [#1729](https://github.com/scode/farhelm/pull/1729/changes): show refused-launch reasons beside Launch.
- [#1735](https://github.com/scode/farhelm/pull/1735/changes): apply a New session choice and launch with Enter.
- [#1737](https://github.com/scode/farhelm/pull/1737/changes): perform Restart with's displayed action from its choices
  and fields with Enter.

The PRs form a linear draft stack. None was marked ready or merged by this executor.

## Checks run, reused and skipped

The final Restart with and shared-model selection has completed passing command evidence for all 32 intended cases
across Chromium and WebKit. Run `0523b5f6-0014-4ef4-81e7-666ba10b2e5e` passed 22 cases (stop consent,
repeat/composition, model refusal, command fields, held-request focus, YOLO cancellation and New session model policy).
`da257637-b72f-4100-8f52-e59e2044ea25` passed the exact choice case on both engines;
`1ae38dad-2ade-4c4a-82eb-9644242a5601` passed the exact fixed-context case on both engines;
`a2be85e2-a320-4319-b297-5e980a02f352` passed the remaining six YOLO-confirmation, stop-asking and model-choice cases.
These successful commands had complete retained reports, zero failures and zero skips. They cover the final product and
assertions in PR 3 revision `378356747ecb`; earlier matching runs preceded the commit operation without a source change.
No failed or interrupted command supplies finishing pass evidence.

The final combined source passed Rust formatting, both required Clippy configurations and desktop UI compilation. The
native and web builds were refreshed after the careful rebase. SPEC, TODO and fragment formatting, changelog format lint
and the isolated source delay checker passed; the final delay check inspected 269 calls with zero missing reasons. These
checks cover the final product source. Later corrections changed browser fixtures and assertions only, so builds and
Rust checks were reused. The source checks relevant to those corrections passed again.

PR 1's recorded launcher-refusal browser run `0101e512-4bd0-4e37-9a05-273b91762294` passed eight cases across Chromium
and WebKit. The focused launcher-form nextest run `1f5d9e02-1be3-45ba-bf19-d80ba1ef0d88` passed 32 cases. These are
reused from PR 1 revision `ebc839d9`; its changed lines are identical after rebase, and later work preserves its
prerequisite/refusal logic.

PR 2's initial command `4ee02b9c-7397-405b-8ed9-6153649d9cc1` failed with 19 completed passing-case observations and
three failures. Two were test readiness assertions expecting lowercase "high" where the recent setup correctly showed
"High", before repeat input. The third was the WebKit timeout described above. The casing assertion was corrected. Exact
repeat and choice-button reproduction `cd9e537d-0992-4e9e-8877-2c39b60ec12d` passed all four cases on both engines. Run
`3a483add-f09e-45fb-8316-9d9083c8be2a` passed the other ten new cases (value controls, incomplete drafts, action buttons
and YOLO Enter rules). Together they give completed passing-command evidence for all 14 new cases; the failed command is
not reused as a pass. Preserved tab, search, recent, model and save-template rules passed all 20 selected cases in
`b354de8e-3fb2-4ac7-a5a6-62793766e41d`. The earlier successful commands cover PR 2 revision `2e17f25b`, whose changed
lines are identical after rebase. The ten-case command ran against the final combined stack; PR 3's optional
model-primary policy does not change New session's policy, which was also checked on that stack. Failed evidence was
preserved; passing reruns do not erase it.

PR 3 browser run `de56ada8-e2f8-4a50-85ac-9e9f1525bb99` was interrupted: 14 passed, five failed, one was interrupted and
12 did not run. It is retained as incomplete validation. Three new cases failed before Enter because their injected
model did not offer Low in the real catalog. Their catalog is now controlled and choice visibility is asserted before
input. Another case incorrectly expected the harness-default model to be omitted; the wire format correctly sends null.
The existing WebKit YOLO-confirmation case failed before opening the dialog with a blank page; its cause remains
unknown, and it passed narrowly in the next completed run. Completed cases in that interrupted command are observations
only; they do not supply finishing pass evidence.

Corrected narrow run `e6eadea9-1569-43a9-b79d-0a4cab13a1b4` completed with seven passes and three WebKit failures, all
before opening the dialog. The exact choice case reproduced that row-readiness failure in
`d5660931-b0b0-472c-8b2f-583aa20f7695`: Chromium passed and WebKit failed. Traces show navigation completing before WASM
startup and the first session-list paint; the injected row appears after the default five-second setup deadline. The
dialog helper now uses the existing session setup budget of 20 seconds for its exact row oracle and adds bounded source
and DOM diagnostics on failure. Input, request, focus and whole-case deadlines are unchanged. The same exact case passed
on both engines in `da257637-b72f-4100-8f52-e59e2044ea25`. One reproduction selected no tests
(`0100def6-c9e9-4f9d-97e3-d2fdf79dff13`), and one prematurely launched reproduction was deliberately interrupted
(`caf2c195-06fa-4ca5-b94d-c8547cb4caf6`); neither is a behavior pass. All failed and interrupted evidence is retained.

The final eight-case selection `a30e16e4-9728-4f8e-9052-c338bd73a484` completed with seven passes and a WebKit
fixed-context setup failure at that older test's own five-second row wait. It had not reached the dialog. The exact case
passed on both engines, with that original wait unchanged, in `1ae38dad-2ade-4c4a-82eb-9644242a5601`. Its failed command
remains retained and does not supply pass evidence; no product or fixture amendment followed that narrow pass.

The stack was carefully rebased onto inspected main. Landed repository-cache work changed checkout preparation and
startup cleanup, conversation-notice work changed diagnostics and hook persistence, and desktop UTF-8 work changed
native text encoding. Their full diffs did not change this work's launcher choices, whether Launch or Restart is
allowed, model handling or stop consent. Later upstream changes, including final inspected main `a3c4afac`, contained
queue bookkeeping only. There were no textual conflicts or functional conflicts requiring a decision; native builds were
refreshed for the landed executable changes.

No full Rust or browser battery was run. Focused browser cases cover the changed Enter behavior and its shared primary,
YOLO, model and focus interactions. Launcher-form units cover prerequisite logic; no supervisor lifecycle or installer
behavior was changed by this stack. Existing relevant evidence was reused per command. No CI workflow was dispatched.

## Review gate outcome

Each code PR passed its independent gpt-6.1-sol high source review. PR 1 added empty-command refusal coverage. PR 2
fixed the existing recent-row handler's repeat guard and asserted cancellation of native activation. PR 3 fixed the
first same-event choice's focus handoff to a still-disabled Restart button by using the existing dialog container as
fallback; its held-request tests also wait for request arrival before inspecting bodies. The controlled catalog,
null-model assertion and bounded row-readiness corrections passed follow-up source review. No review finding was left
unresolved. Scope reassessments found no unnecessary substantial mechanism. Fresh wording readers accepted the commit
and PR prose. Source review and recorded runtime validation both passed; each supplies its own evidence.

Implementation stayed with the executing session under the plan's no-workhorse rule. Only the required reviews were
delegated: source reviews to native gpt-6.1-sol at high effort, scope and wording reviews to native gpt-6.1-sol at
medium effort, and the required resume/report cold reads to the executing harness's inherited model. Review conclusions
were checked against source; the author collected build and runtime evidence. Native usage and exact model telemetry
were unavailable. Private orchestration evidence is retained under session `2c666c48-ffa9-434a-826f-3b4baa7214f0`; the
plan's working log beside the checkouts identifies it.

### Landing

Landed on 2026-10-09 (UTC) as three squash commits on main, in order: #1729 (launcher refusals by the Launch button),
#1735 (Enter launches from the launcher's choices) and #1737 (Enter restarts from Restart with's choices). The plan
waited 8 to 19 hours after delivery because the monitor stalled between landing rounds; that was the monitor's fault,
not the plan's.

#### What else was on main

Between the commit the stack was built on and the landing, main gained the version-hover-text change (#1733, landed just
before this one), documentation, TODO entries, test cleanups, a Rust 1.99 Clippy cleanup and the planning queue's
bookkeeping. The only conflict was in TODO.md, where #1733's entry sat next to this plan's; both are removed.

#### Review before merging

A separate reviewer that had not worked on any of the five plans landing in this round (version-hover-text,
enter-launches-anywhere, drag-copy-notice, preview-lock-identity, transcript-reads-on-need) read them against each other
and main before anything merged. They share no code that conflicts; the only textual conflicts were TODO.md, where each
plan removed only its own entries, and FLAKES.md, where entries were appended. No protocol, supervisor or helm database
version changes. It found no conflict between the new Enter behavior and the quick switcher, Cmd+N, or the
save-as-template panel (Enter in its name field still saves). It did find that #1729 changes how the Launch button
refuses (focusable with a refusal next to it, rather than natively disabled) and that several existing browser specs
outside the stack look up the launcher's error line, which the plan's own browser runs had not covered: GitHub-checkout
composer, GitHub checkouts, destination authority, create idempotency, clone and replace. The landing ran all of them on
both engines; they pass (below).

One small point the landing did not change: the comment in the sidebar's launcher Tab-order test still says a disabled
Launch button is skipped by Tab; after #1729 a refused Launch stays focusable. The test itself still passes.

#### Checks

- Run now, on all five plans stacked together in landing order: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` (again after main gained
  a Rust 1.99 lint cleanup, still clean), `cargo check -p farhelm-ui --features desktop`, the web build check,
  `dprint
  check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format` and the UI
  JavaScript tests (203 passed), all clean; the supervisor and protocol unit tests in full with the Codex, Grok and hook
  identity end-to-end tests (run `30c797ad`: 1205 of 1206; the one failure is described in transcript-reads-on-need's
  notes and passed after its fix, 22 of 22 identity tests); and, through the recorder on Chromium and WebKit, the
  GitHub-checkout composer, GitHub checkouts, destination authority, create idempotency, clone, replace, mouse modes,
  launcher Enter and Restart with specs (run `cd71dca0`, 196 of 196 passed).

Nothing in the report above was made untrue by the landing.
