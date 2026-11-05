## What this was about

A program could extend a printed web address onto a neighboring wrapped line while a resting hover kept the old
destination. Even after the terminal redrew, clicking the displayed `https://bank.com` could still open
`https://bank.co`. The maintainer chose best effort for a program racing a click, but no lasting mismatch.

The fix makes printed-link hovers remember the neighboring rows used to find the address. Changing one of those rows
invalidates the old link. The regression reproduced the old destination opening in Chromium and WebKit before the fix
and opens the updated address in both afterwards.

Two related decisions are recorded separately: opening newly written text before its next paint remains accepted under
the existing sub-second redraw allowance, and a hyperlink mismatch warning is judged when the hover starts. The warning
need not follow later redraws of other rows while the pointer rests; the displayed real destination stays authoritative.

## Things you should know

The code and regression are below the agreed 100-line size gate. No process that keeps a terminal, wire protocol,
program-supplied hyperlink warning logic, or paint/click ordering changed. All three feedback files and index entries
are removed, with execution recorded in their existing triage outcomes. The PRs remain drafts for the landing monitor.

The full JavaScript harness has one existing failure: the stylesheet token test rejects a literal `8px` corner radius.
The stylesheet and that test are unchanged from the plan's base, `59ef9722`. This plan leaves that unrelated failure
visible rather than claiming a clean full-suite result.

## Open questions and possible follow-ups

No maintainer decision is needed for this plan. The existing stylesheet token failure remains a separate follow-up.
Native Mac interaction was not exercised; WebKit provides browser-engine coverage, not a native desktop test.

## The PRs

- [#3018: refresh printed links after neighboring edits](https://github.com/scode/farhelm/pull/3018/changes), draft.
- [#3019: retire the before-redraw terminal URL finding](https://github.com/scode/farhelm/pull/3019/changes), draft.
- [#3021: clarify terminal link warning timing](https://github.com/scode/farhelm/pull/3021/changes), draft.

## Checks run, reused and skipped

- Ran the new browser regression before the fix with generic test recording, two workers and zero retries: both engines
  failed by opening the old prefix after the changed neighboring row was painted. Run
  `2584e6da-4cd0-420a-8289-6a6c1cbd9670` is retained as failure evidence. After the fix, the same selection passed
  twice, once per engine, in `5cc882d6-60b9-4860-af15-7f2c11538349`.
- Ran `npx playwright test --config grid-client/playwright.config.ts grid-attach.spec.ts --workers 2 --retries 0`
  through the recorder's generic mode: 74 passed, 37 per engine, with no skips or retries. Run
  `3f447182-8847-46ae-80b2-6e8dbcd21d30` exercises the shipped assets against a fake terminal socket, including plain
  links, program-supplied hyperlink warnings, file links, tunnels, repaint and activation. It makes no real-stack or
  controlled-runner claim.
- Ran `node --test` in the UI JavaScript harness through the recorder: 515 passed, one existing stylesheet-token test
  failed, zero skipped. Run `405da099-af1a-4662-aba1-31b8de56dad2` is retained. The focused
  `node --test grid-buffer.test.js` passed all 35 tests in `2a40c17c-8dc5-4893-834b-b63773d24d25`.
- Ran the isolated-interpreter test-sleep checker: 412 delays inspected, zero missing rationales. Changed Markdown
  formatting, changelog format and whitespace checks passed. The agent-instructions checker passed with zero errors and
  three existing citation warnings outside this work.
- The post-fix browser and JavaScript runs above tested base `59ef9722` plus the recorded working-tree snapshot
  `df8fc91767f81d6272ae62455934e11829697da088d6801609f6dba13045360f`. Their passing browser and buffer results still
  cover the final stack: later edits only explain the hover dependency contract or change specification and review
  bookkeeping. The source snapshots, output EOF and complete untruncated retained output were verified for every
  recorded run. Upstream changes through `2cbfdc77` affect unrelated Rust test fixtures, flake tracking and plan state;
  their full diff was inspected and does not change this coverage.
- Skipped Rust builds/tests, live-stack browser tests, the rest of the static grid suite, CI and native desktop tests:
  no Rust, build wiring, transport, rendering or fit behavior changed. The buffer unit tests and attachment spec cover
  the affected URL search and hover/click path. The two documentation-only PRs required no runtime tests.

## Review gate's outcome

Each PR passed the required single fresh-context gpt-6.1-sol review at high reasoning effort. The first review found one
stale explanation that limited hover dependency rows to program-supplied hyperlinks; it was corrected and the reviewer
confirmed it resolved. No functional findings remain. Independent commit-message cold reads completed, and the delivery
report was also cold-read before delivery.

### Landing

Landed with no fixes at landing. Clicking a printed web address now opens what the terminal shows even after a program
extends the address onto a neighboring wrapped line under a resting pointer, and the specification now says when the
hyperlink mismatch warning is judged.

- #3018: printed-link hovers remember the neighboring rows used to find the address, with a browser regression.
- #3019: retires the review finding about clicking newly written text before its next paint, which stays accepted.
- #3021: the specification's wording that the mismatch warning is judged when the hover starts.

#### What landed since the stack was based, and whether it interacted

Five changes landed on main after the plan's base and before this round started:

- #3012 and #3013 corrected the tests behind three failures that earlier landings reported on main;
- #3014 recorded a browser flake;
- #3016, an approved planning PR, queued five plans and added their pointers to TODO.md;
- #3017 moved eight containerized-session review findings from the review queue into TODO.md.

None touches the terminal grid or its specification, and after #3017's removals the review queue's index still lists
exactly the files present. A fresh-context review read the plan against these and against the earlier grid plans on main
(the repeated-link warning and the drag-copy notice) and found no interaction: the program-hyperlink path is unchanged.

While the round ran, #3027 also landed on main: it replaces a literal corner radius in the "Tensorlake support is
experimental" warning's stylesheet rule with the shared design token. It changes only that rule, which the plan does not
touch, and it fixes the one JavaScript test failure described under Checks; the JavaScript tests were run again on main
after the merge. Between this plan's merges, #3038 also landed, adding illustrations to the v0.35.0 release notes (docs
screenshots and their data); it touches nothing the plan changed. Main now differs from the tested stack only by those
two changes.

#### Things you should know

- A click on a printed address now does nothing if a row its hover depends on (the rows the address was found on, and
  the row just beyond each end) is redrawn between press and release, because the click opens only a hover that is still
  current at release. A program that keeps redrawing a neighboring line (a spinner or status line) can therefore swallow
  such clicks with no explanation, and the tunnel hint flickers on each redraw. Program-supplied hyperlinks already
  behave this way, so this matches the existing design; re-checking the link at release instead would be a separate
  change.
- Printed file paths record no neighboring rows either, so they may have the same gap as the one fixed here. The review
  did not verify this, and its impact is smaller because a file hover shows the full resolved path.
- The specification's new sentence says the warning "need not" follow redraws of "other rows". The code still re-judges
  it when the link's own rows, or the row beyond either end, change, which that wording allows; tighter wording would
  help a later reader.

#### Checks

This round landed only this plan, stacked on main `cf43a0ed`. On the stack:

- `cargo fmt --all -- --check`, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`, 412 delays, none without a reason), `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three warnings about
  older text), the workspace debug build and the UI's release web build: all passed. Clippy was not run: no Rust source
  changed.
- The grid terminal component's browser suite, which holds the plan's regression and is not part of the main browser
  suite (`npx playwright test --config grid-client/playwright.config.ts --retries=0` in `e2e/`, Chromium and WebKit),
  run `e32a3935-090c-498a-a862-d330ed2ab595`: all 158 passed.
- The UI's Rust tests, which build the changed scripts into the UI, `cargo nextest run -p farhelm-ui`, run
  `7fe5fd22-3911-472c-83c8-ee2137adc7c6`: all 857 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `cecd2ff3-25f6-441a-a62a-c189450edab9`: 515 passed, 1 failed. The failure is the
  check that the shipped stylesheet uses only design tokens: the "Tensorlake support is experimental" warning (#3010,
  landed on main before the plan's base, outside the plan queue) added a literal `8px` corner radius. The executor's
  report saw the same failure. It is not this plan's, and #3027 fixed it on main during the landing; the JavaScript
  tests run again on main after the merge, run `62299e15-29e4-489e-8e84-8a97b1a21f4e`, all 516 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack), runs `f6817104-0ea4-4e1a-b48b-6c4ebabf2fd9`, `bccbb884-a162-4023-8682-6367fc564910`,
  `2d338903-2d70-4c8c-99c4-3d4044d78670`, `9a648ba2-ce5a-49a1-9329-028beae0d99e`,
  `4a0618c3-d05d-437b-92c0-1918b3fbfbc7`, `1313815c-a8c6-4e16-8d02-7710ab6831b5`,
  `09beff9d-547f-4412-9f2d-00353268d3ea`, `c9de159f-30f6-4161-abbb-7f19887499dc`,
  `8fe592d8-fd5f-4c19-a387-9a186f7498af`, `035273be-325b-4175-a29a-dba4d25254bd`: 2996 passed, 18 skipped by the specs'
  own design, 6 failed. Then the same suite with every stack's terminals in terminal hosts, since host terminals render
  through the same grid code (`FARHELM_E2E_SUITE_TERMINALS=host`), runs `ea697f50-0177-48e5-882b-37e12fe98e09`,
  `e8d4e772-208c-4908-a84f-ca9edb32fd99`, `c1a33098-ffa0-4c7d-962a-f5e8b7560914`,
  `67bdd00e-2973-4f31-afb3-eb2d4aa7293d`, `6d92d6a1-6361-466c-9e84-038d218d4cf5`,
  `03071c4c-3b35-4862-b2be-509eb3ddee06`, `0aafd136-95e9-4b9b-a2a8-1b9552809115`,
  `fb435a0d-f6c9-4da4-b317-ca85d0a1f061`, `e0456921-ff32-4fc5-b6c6-706f0898f158`,
  `58a9cac7-f35c-4623-8fd1-e7a60d6324c1`: 2081 passed, 75 skipped by the specs' own design, 4 failed. This run covers
  only the specs listed for host terminals (`e2e/host-terminal-specs.ts`), which is why it holds fewer tests. Of the ten
  failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `f6817104`,
    `9a648ba2`, `67bdd00e`, `fb435a0d`), because the Add host form's "SSH" and "Tensorlake sandbox" choices have no
    hover text. Those choices came with the Tensorlake sandbox hosts (#2973), on main before the plan's base; the plan
    does not touch the form. It is a break on main and a follow-up there.
  - Recorded flakes: the launcher's "Runs in YOLO mode" step on WebKit, in "create-intent-key-rebinds-on-host-change: a
    new target is a new intent" (once in each suite: runs `4a0618c3` and `67bdd00e`), and "post-clone commands with
    carriage returns refuse edits but can be cleared" (WebKit, once, in the host-terminal suite: run `e0456921`;
    recorded by open draft PR #2549).
  - Three single failures in the ordinary suite, none in code the plan touches, each of which then passed three re-runs
    on the same build, also on sandboxes (the run ids below are the re-runs): "a sequence refused at its prefix leaves
    later sequences their own bindings" (Chromium; the page's focus did not return to the page body; failed in
    `2d338903`, re-run `75b17c09-0b63-4b41-b68d-504f176cf3c0`), "suspending an agent with unknown status asks before
    sending the request" (WebKit; its session was missing from an activity read; failed in `4a0618c3`, re-run
    `f7329da0-6ec8-45ba-8985-5f040fd9401b`), and "an unconfirmed delete sends the precondition and shows the
    supervisor's refusal" (WebKit; a stopped session reported an error instead of exited, as in the flake #3014 recorded
    for another test; failed in `c9de159f`, re-run `8a06acae-23a6-4ad7-b5b2-0ab320192785`). Each is a single failure,
    not recorded as a flake.
- Reused: the executor's run of the plan's regression before the fix, which failed in both engines by opening the old
  address. Not run: the workspace's Rust tests and the desktop-build tests (no Rust source changed), the doctests, the
  desktop smoke tests, the installer tests, the development screenshot review (no shot changed) and the native Mac app
  (not runnable on Linux).

The executor's report calls the PRs drafts and names the older main its runs tested; what landed is its commits rebased
onto current main, with their content unchanged. The stylesheet test failure it calls a separate follow-up was fixed on
main by #3027 during the landing. Nothing else in it became untrue at landing.
