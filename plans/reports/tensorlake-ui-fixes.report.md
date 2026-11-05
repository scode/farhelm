## Since the last review

The landing monitor returned the stack because closing Settings from its header sometimes showed no unsaved-recipe
question on WebKit. Nothing had merged. This round updates #3050 in place; no PR was added or dropped. The other three
PRs retain their reviewed behavior and the monitor's fixes: the Suspend and Refresh hover text, class selectors for the
Add host help, and the changelog's coverage of #3049 and #3050.

A controlled browser run established a focus race. The editor's opening animation frame could take focus from Name after
the field was selected. Subsequent text then never reached the field, leaving the draft unchanged; header Close
therefore closed without a question. The correction makes that initial focus and scroll yield to a field or control
already chosen. The ordinary heading focus and editor-top opening remain. Close authorization and the recipe's draft,
save and confirmation owners are unchanged.

The original test was left unchanged. A separate regression holds the actual opening frame, chooses Name, then releases
the frame and checks that focus, scroll position, typing and the header-close question survive. It failed before the
product correction and passes afterward in Chromium and WebKit. The natural failure did not recur in ten unchanged
baseline WebKit runs here, so the controlled result establishes the mechanism without claiming to recover the original
monitor runs' exact event order.

The unchanged close test passed 20 consecutive WebKit repetitions after the correction, with one worker and no retries.
The full recipe spec passed all 16 cases, and four Add host focus cases passed, covering both palettes and both engines.
All landing fixes remain in the four draft PRs.

## What this was about

Tensorlake's interface made several operations hard to follow: Suspend claimed the sandbox was suspended before the
service confirmed it; launching on a suspended host briefly said it was unavailable and showed no resume progress;
recipe Save required hunting below the fields; Refresh sandboxes appeared inert; and Add host's creation question was an
unstructured block of text. This plan addresses those six TODO entries in four draft PRs.

Suspend and Resume now show their pending state, and Suspended appears only after confirmation. The launcher shows its
own resume and reconnection progress, including quick launch. Existing-directory launches proceed when ready; a fresh
checkout still requires reviewing the refreshed preview and launching again. A failed resume says the session was not
launched.

Recipe actions and their questions and errors stay visible in Add host and Settings. Settings opens the editor at its
top, keeps its normal Close control, and gives sandbox discovery a heading, progress and a checked time even when none
are found. Add host now has field help and a recipe summary, followed by the standard numbered setup confirmation with
size, billing and idle-limit information, and Back. The Suspend question and sandbox details also use the app's normal
presentation.

## Things you should know

- Lifecycle API calls, timeouts and the rule against resuming a sandbox automatically are unchanged. A repeated Resume
  on a connected, running sandbox preserves its viewers. Routine pending transitions do not create a host-attention
  problem; independent setup warnings still do.
- Only the launcher's own pending resume suppresses its unavailable message. A host another surface is changing remains
  unavailable to it. Fresh-checkout resume does not authorize a session create.
- Back discards the displayed creation offer. Continue obtains a new plan, and Cancel and Escape send no creation
  request. Only the acknowledged Tensorlake creation preference skips this question; SSH's preference cannot do so.
- The combined billing callout also appears on Tensorlake setup reruns. Ordinary SSH setup keeps its existing answers.
  Copy plan remains in the body and preserves the helm's exact text.
- Managed snapshot builds already install both Claude Code and Codex unconditionally, so the setup step naming both was
  accurate and was left alone. The experimental-warning typo and stale implementation-spec claim that the browser fake
  refused suspend/resume were corrected. Existing unreleased changelog fragments were retained; the lifecycle fragment
  was amended, and presentation polishing needed no additional entry.
- The Tensorlake sandboxes and Manage hosts guides match the new behavior. These were incidental documentation edits; no
  preview server was started.

## Open questions and possible follow-ups

No required decision remains. The new screens were inspected in Chromium and WebKit, in both palettes. Native Mac
rendering and real Tensorlake service behavior were not exercised; the browser cases use the existing fake or
intercepted plans.

## The PRs

- [#3048](https://github.com/scode/farhelm/pull/3048/changes): confirmed lifecycle states and honest launch-resume
  progress.
- [#3049](https://github.com/scode/farhelm/pull/3049/changes): standard Suspend confirmation and sandbox details.
- [#3050](https://github.com/scode/farhelm/pull/3050/changes): visible recipe actions and sandbox discovery feedback.
- [#3051](https://github.com/scode/farhelm/pull/3051/changes): structured Add host form and creation confirmation.

All four remain drafts in one linear stack. The earlier integration rebase covered manual pin ordering, its schema,
session-list ownership and menu focus, shared API types, styles and specification text. This round rebased onto
`551a733d03c379736abfc5f008900ef5ed0d3410`; main had gained only plan bookkeeping. Every PR patch and the working
correction were unchanged by that rebase. The focus correction is folded into #3050, preserving its reviewed descendant.
Final published tip `076f420ede3272819536d2e0dbfd0834db544c49` has the same product and test files as the validated
correction; only the plan index and Decision differ from that tested tree. No design conflict or manual resolution was
needed.

## Checks run, reused and skipped

Runtime commands below ran through `python3 scripts/record-test-run.py`, with pinned tmux 3.7c, zero retries, four Rust
slots or up to three browser workers as applicable. Counts name observations rather than treating a later pass as
erasing a failure.

This round's new evidence covers the three-file focus correction before committing, with Git head
`f7dad8657b044b5da5339e92c7dbecdf1d5d07ab` and working-tree fingerprint
`1706c0c5286f7693505875e988ba949e159a49cec7797cb2590a6ea2709e6b41`. It changes the recipe opening handoff, its browser
regression and the implementation specification. Backend lifecycle behavior and the other dialogs remain as reviewed.

- The unchanged exact close test passed ten baseline WebKit repetitions in `5543e95d-6cdf-4948-90af-7e1961f8aa68`; this
  was a non-reproduction before the fix. The separate held-frame case then failed its post-frame focus assertion in
  `a16d3703-f84f-4ef6-9186-9ecd10faa439`. Diagnostic run `fd93b603-62d7-4e46-894c-0c48ba0df2f6` reproduced that failure
  and captured the heading active, an empty Name after typing, and Settings gone after the real header close. Both
  failed records and their private screenshots and traces were retained.
- The corrected held-frame case passed once per engine in `61406cf3-40f7-4145-ab4e-826e84efccc5`, using the real
  corrected release web bundle. Source, reports, output and cleanup are complete, with no skipped or interrupted cases.
- The unchanged original close test passed 20 consecutive WebKit repetitions in `8df432db-3301-4005-8913-a0ab4ed9e60c`.
  Its retained JSON report confirms 20 executed passes, only the WebKit recipe project, one worker, no retries, skips,
  flaky outcomes or global errors. The source fingerprint matches the corrected held-frame run; output reaches EOF
  without omission and all owned stacks cleaned up. This was generic recording to permit the deliberate single-engine
  repetition policy.
- The complete recipe file passed all 16 cases, eight per engine, in `43034a1e-621f-48a0-b014-36143dcaf139`. This
  includes both palettes' opening and sticky-answer geometry, CRUD, token privacy, validation, all close paths, refresh
  recovery and snapshot builds. Four Add host inline-recipe cases passed in `e81ac0f3-1e70-4ab9-a267-001e8541c928`,
  covering initial Name focus and persistent answers in both palettes and engines. Both records have the same corrected
  source fingerprint, complete reports and output, no skipped or interrupted cases, and complete cleanup.
- The actual release web build and native UI all-target desktop-feature Clippy check passed after the correction. The
  test-delay source check inspected 412 delays with none missing a reason; formatting and whitespace checks passed.
- The whole UI JavaScript harness passed all 527 tests, with no skips, in `9395b061-4465-42ff-b002-167242d2edb6` at
  `f7dad8657b044b5da5339e92c7dbecdf1d5d07ab`. This includes the monitor's stylesheet selector correction. Its asset and
  JavaScript-test inputs are unchanged by the subsequent Rust focus, browser-test and specification patch, so that
  command's coverage is reused.

The additional child commands were:

```sh
npx playwright test --project=webkit-tensorlake-recipes -g 'closing Settings with an unsaved recipe asks first and finishes the close' --repeat-each=10 --workers=1 --max-failures=1
npx playwright test --project=webkit-tensorlake-recipes -g 'a recipe opening preserves a newer field focus before header close' --workers=1
npx playwright test 'tensorlake-recipes\.spec\.ts' -g 'a recipe opening preserves a newer field focus before header close'
npx playwright test --project=webkit-tensorlake-recipes -g 'closing Settings with an unsaved recipe asks first and finishes the close' --repeat-each=20 --workers=1 --max-failures=1 --reporter=list,json
npx playwright test 'tensorlake-recipes\.spec\.ts'
npx playwright test 'tensorlake-hosts\.spec\.ts' -g 'Add host pins inline recipe answers and focuses Name'
node --test
```

The first two used generic recording for intentional single-engine reproduction; the corrected two-engine run used the
recorder's controlled Playwright mode. Node ran from the UI JavaScript test directory with its default discovery.

The reused checks cover completed PR1 source at `3e6f1894a121c8f397e31cb6b80e1a10c96392f9`, PR2 at
`c182602ebb6c3578ebb80354794a3d1501b84019`, and PR3 at `cc2443d864fd15d393549c9e8f496cdf450334cc`. Runs were recorded
before committing, so their manifests also retain each working tree's source fingerprint. The final rebase preserved
every PR patch; the final integration checks below cover its shared-code seams.

These were the Rust child commands for the 184-case selection and its corrected two-case selection:

```sh
cargo nextest run -p farhelm-helm -p farhelm-ui --lib -E '(package(farhelm-helm) & (test(tensorlake::hosts::tests::lifecycle_) | test(phase_matches_the_serialized_tag) | test(tensorlake_lifecycle_intent_projects_without_claiming_suspension) | test(a_changing_sandbox_is_never_probed_for_update_or_setup) | test(suspended))) | (package(farhelm-ui) & (test(hosts::) | test(list::shared::) | test(list::create_form::tests::) | test(tensorlake_lifecycle_phase)))'
cargo nextest run -p farhelm-helm -p farhelm-ui --lib -E 'test(=tensorlake::hosts::tests::lifecycle_suspend_holds_routing_and_excludes_removal) | test(=hosts::attention_list::tests::tensorlake_transitions_preserve_independent_attention)'
```

Browser child commands, from `e2e/`, in the order of the coverage groups below: PR1 and PR2 full lifecycle runs; PR3
full affected files and final opening correction; PR4 new structure and existing regressions; final integration.

```sh
npx playwright test 'tensorlake-lifecycle\.spec\.ts'
npx playwright test 'tensorlake-(recipes|hosts|settings|tunnels)\.spec\.ts'
npx playwright test 'tensorlake-recipes\.spec\.ts' -g 'Settings keeps recipe answers sticky and isolated'
npx playwright test 'tensorlake-hosts\.spec\.ts' -g 'structures its form and creation plan'
npx playwright test 'tensorlake-hosts\.spec\.ts' 'tensorlake-settings\.spec\.ts' 'provisioning\.spec\.ts' -g 'adding a Tensorlake host|the Tensorlake form|inline reuse recipe|recipe deleted during confirmation|cancel retires a Tensorlake|rerun before first create|Tensorlake settings open with the experimental warning|host-setup-layout'
npx playwright test 'tensorlake-lifecycle\.spec\.ts' -g 'pending .* names its transition|launching in an existing folder|successful resume stays honest|another surface|quick launch shows preparation|failed resume refuses|delayed create after resume|fresh checkout resumes first'
```

- Reused PR1 Rust coverage: the focused Tensorlake lifecycle, provisioning refusal and UI-state selections covered 184
  unique cases. Run `59d32350-db3b-48ee-8179-c954f266321d` had 182 passes and two fixture failures; both corrected cases
  passed in `93aa84d3-87f3-4e07-afad-5ff9fba97a27`. The original run remains failed. Its product behavior was unchanged
  by the later presentation PRs and integration rebase.
- Reused PR1 browser coverage: 40 passed, 20 per engine, in `6c6f9d38-b402-4a26-a19e-728627f107e1`. An earlier run had
  one WebKit route-teardown failure in `20a74578-c65a-46b8-8d2c-f338745cf57c`; route callbacks now drain before cleanup,
  and the failed evidence was retained.
- Reused PR2 browser coverage: four focused visual cases passed in `6cf442ba-13f4-47ab-bdbb-54cafe1e4b2a`, then 44
  affected cases passed in `48c6a740-b1c6-4600-9d3a-bc6e83d6b152`, 22 per engine. All eight focused screenshots were
  inspected. Later changes preserve that dialog and lifecycle behavior.
- Reused PR3 coverage: 48 affected browser cases passed in `dfaafaa4-17e0-4b31-80ba-18ed4ec8fdaf`, 24 per engine. After
  the final editor-opening scroll correction, its four affected palette/engine cases passed again in
  `473e87e1-7718-4d7d-9ee2-4529b9e2e2bb`; the other 44 retain their coverage. Earlier Chromium geometry failures were
  retained in `c9b86fc5-1354-44d0-b901-ec79f7541373`, `ff799516-ef67-4552-a8fb-534a7f5f5770` and
  `d0d1815a-9778-484c-91c8-7f449bf79181`, then corrected without weakening bounds. Sixteen representative screenshots
  from the full run and all four final opening screenshots were inspected.
- PR4: four new dark/light form-and-plan cases passed in `a13e8ffe-3f20-44a2-8f66-10783e50a4ee`, and all eight actual
  screenshots were inspected. Twenty existing creation, rerun, skip-policy, cancellation, warning and SSH-layout cases
  passed in `441573ef-216e-4e9a-890a-72ea67d38fc5`, ten per engine. These also establish Back's fresh plan, no-create
  cancellation and exact Copy plan bytes. Both records are complete, with no skipped or interrupted cases or cleanup
  gaps.
- Each presentation unit passed the actual release web build and the UI's all-target desktop-feature Clippy check. The
  required test-delay source check inspected 412 delays with none missing a reason. Rust formatting, changed Markdown
  formatting and whitespace checks passed. The changed lifecycle fragment passed changelog formatting. Initial PR4
  compilation found an incorrect module path, which was corrected before the passing builds. Commands:
  `dx build --package farhelm-ui --platform web --release` in the UI crate,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check` on the changed Markdown, `git diff --check`, and
  `python3 releasing/check-changelog.py format`.
- Final integration on the rebased stack at `6619cdcd`: the CLI build, actual release web build and all-target
  desktop-feature UI Clippy check passed. Twenty transition and launcher-resume browser cases passed in
  `60f123ac-827d-465a-9259-3e5f889789e7`, ten per engine, against the integrated binaries and bundle. The report, source
  identity, stream EOF and cleanup are complete, with no skips, failures or interruptions. These target the new list
  ownership and shared session-data seams; unchanged dialog and recipe coverage above was reused. The CLI command was
  `cargo build -p farhelm`; web build and Clippy commands were as above.
- Skipped: another full runtime battery, because unchanged per-unit coverage and the targeted integration selection
  cover the identified seams; native desktop runtime, installer and provisioning-distribution gates, because those
  implementations did not change; website build, because its prose edits changed no links, headings, frontmatter or
  executable examples; hosted CI, which is manual-only. No live installation was changed.

## The review gate's outcome

Every PR passed its required independent gpt-6.1-sol review at high effort. Findings about recipe-editor errors and
fractional clipping were fixed and reviewed again. A separate review at the executing model checked the editor's scope
and supported the smaller existing-owner design. No required finding remains unresolved or was rejected. Commit and PR
wording passed fresh cold reads, and changed files received a separate documentation pass.

The follow-up's fresh gpt-6.1-sol review at high effort found no issues in the focus admission or strict clock
regression. Its separate scope checkpoint supported the local existing-owner correction without new state or a broader
focus framework. Updated PR wording passed a fresh cold read, and all three changed files received the documentation
pass.

### Landing

Landed with fixes at landing. Tensorlake hosts now show Suspend and Resume as pending until the service confirms them,
the launcher shows its own resume progress instead of a brief "unavailable", recipe actions and errors stay in view,
sandbox discovery shows its progress, and the Add host question for a sandbox is laid out as the usual setup
confirmation.

- #3048: suspend and resume progress, and the launcher's resume progress.
- #3049: the Suspend confirmation in the app's normal dialog, with hover text added at landing.
- #3050: recipe controls and sandbox discovery, with hover text added at landing and the focus fix from the send-back
  below.
- #3051: the Add host dialog for sandboxes, with the stylesheet and changelog fixes below.

#### Sent back once, and why

The first landing attempt found that closing Settings from its header with an unsaved new recipe sometimes showed no
"Save these changes before leaving?" question on WebKit: "closing Settings with an unsaved recipe asks first and
finishes the close" failed 4 of 14 WebKit runs on the stack and 0 of 10 on main without the plan. The monitor blocked
the landing, nothing merged, and it answered the block by sending the plan back to fix the cause. The executor found it:
the recipe editor, opening in Settings, now moves focus to its heading one animation frame after it appears; when that
frame ran after the Name field had already been chosen, it took focus away before the typing arrived, so the recipe
stayed unchanged and there was nothing to ask about. The editor now leaves focus alone when a usable field or control
already holds it, and a new browser test holds that frame to show the typing and the question survive. The landing added
a check to that test that the frame really has not run yet when the field is chosen, so it cannot pass vacuously if the
clock stops holding it. Those changes are all in #3050.

#### The fixes made at landing

- The stylesheet test that requires design tokens rather than bare values read two new id selectors in #3051,
  `#add-tensorlake-name-help` and `#add-tensorlake-recipe-help`, as the hex color `#add`, so the UI's JavaScript tests
  failed on this stack. The landing selects the same elements by the classes they already carry; the page renders the
  same, and the ids stay for the fields' accessible descriptions.
- Only #3048 touched a changelog fragment, so the release-time sweep would have reported #3049 and #3050, both `fix:`
  PRs, as uncovered. Tensorlake hosts have not shipped yet, so the landing added a `pr: 3049, 3050` claim to the
  existing Tensorlake hosts fragment, in #3051 (a PR that edits a fragment counts as covered). The fragment's text is
  unchanged; the executor judged that this presentation polish needs no entry of its own, and the claim keeps that
  judgment while satisfying the sweep.
- Three new controls had no hover text, which every control should have: the Suspend confirmation's Cancel and Suspend
  buttons (#3049) and sandbox discovery's Refresh (#3050). The landing added it ("Cancel: keep the sandbox running",
  "Suspend: pause this sandbox until it is resumed", "Refresh: check Tensorlake again for sandboxes not on this helm").
  The tooltip coverage test never opens these, so it did not catch them.

All three were made in the first attempt, pushed to the plan's branches with the block, and kept by the executor.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping landed on main since the plan's base, and nothing else landed in this round. A
fresh-context review read it against the plans that landed just before its base, and a second one read the send-back's
change: the host attention list (a sandbox that is suspending or resuming gets no attention row of its own, and setup
warnings still produce one, as the amended specification says) and the pin order (the two plans share one helm file,
where this plan only adds a refusal message); it found no interaction with either. The second review found the fix sound
and asked only for the test's premise check. No helm schema or supervisor protocol changed; an older browser tab shows a
host in the new transitional state as "Host state unknown", the designed tolerance (the item below about the
suspend/resume direction is about a later, unknown direction value, which has no such fallback).

#### Things you should know

- The editor's opening focus now gives way to any focused, visible control anywhere on the page, not only to its own
  fields. Every current way of opening the editor removes the control that opened it, so the initial focus still
  happens; a future way of opening it that leaves its trigger on screen would quietly lose that focus.
- The browser test of hover text still fails on main: the plain SSH Add host form's "SSH" and "Tensorlake sandbox"
  choices have no hover text (from #2973). This plan does not touch that form, and the Tensorlake form keeps the same
  two buttons without hover text. Nothing in TODO.md or the review queue records it yet.
- The specification and docs call the reconnecting state "Connecting…", while the UI shows "Connecting" without the
  ellipsis.
- After a confirmed Suspend, the host briefly shows Connecting before settling on Suspended. That happened before the
  plan too, but it slightly undercuts "Suspended only after confirmation".
- The suspend/resume direction carried to the browser has no fallback for an unknown value, so a future third direction
  would make an older browser tab fail the whole Hosts panel rather than just that host.
- Sandbox discovery now shows "Checking…" on every automatic reread, not only after Refresh, which may look busy.
- The new host state has no row in the shared host-list fixture that the helm and UI tests both decode.

#### Checks

The checks ran in two rounds. The first, on the plan as first delivered with the landing's fixes (stack top `f7dad865`,
on main `c501a760`), covered everything; the second, on the redelivered plan (stack top `10078dbe`, on main `30981f76`,
which differs from `c501a760` only in plan bookkeeping), covered what changed since: one browser-side script inside the
recipe editor's opening, the new browser test and one SPEC_impl.md sentence. That script runs only when the recipe
editor opens, in Settings or in Add host, so the second round re-ran the specs that open it; the first round's results
for everything else still describe what landed.

- On the final stack: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 412 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed. The same gates passed in the first round.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, on the final stack, run
  `35152fa7-1718-4177-94c7-fd49eeed8e6e`: all 879 passed. In the first round: the UI's JavaScript tests (`node --test`
  in `crates/farhelm-ui/js-tests`), run `7c1acb97-f311-4bb1-ae49-3b21278c0630`: all 527 passed; the desktop-build tests,
  `cargo nextest run -p farhelm-ui --features desktop`, run `7a05d383-7ba1-4246-aa01-42994081f4da`: all 1068 passed; the
  whole Rust workspace with the pinned tmux, `cargo nextest run --workspace --exclude farhelm-desktop`, run
  `73aa2036-84f8-4cc3-a6fe-9e8f2433bc9a`: all 5637 passed, 70 skipped (ignored by design).
- Browser tests on Tensorlake sandboxes at the maintainer's request (cloud virtual machines; each booted the usual three
  test stacks and ran its share through the recorder's generic mode,
  `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  the stack). On the final stack: the whole Tensorlake recipes spec, the new test included, three times on each engine,
  WebKit run `ad95c4c4-4c8f-455f-b17e-d657670d7858` and Chromium run `bc1b731a-0418-4a29-b9f6-d05c0b52f7bb`: all 24
  passed on each; then "closing Settings with an unsaved recipe asks first and finishes the close" 20 more times on
  WebKit, runs `54cb0c34-c1ca-4db2-8ebf-f951c8280ddd` and `99b8f20d-c215-4e2c-973c-89fc39620a2f`: all 20 passed. With
  the three in the spec runs, that test passed 23 of 23 on WebKit, against 4 failures in 14 before the fix. And the
  Tensorlake hosts spec, where Add host opens the recipe editor, once on each engine: all 11 passed on each, WebKit run
  `de43a0e9-152e-4ae3-9b00-ab2b6a83dc20` and Chromium run `7292840f-e1c1-42a0-a619-a42b54fd7ce8`.
- In the first round, the whole browser suite on Chromium and WebKit on ten sandboxes, runs
  `31c2d692-0ec9-48c7-adf4-af4353c493bb`, `ec2dbf12-452d-4c2d-8520-c26c49c779a6`,
  `21068fb4-aef7-4b2b-8e28-e6ad7031512b`, `e6073bcc-504c-46aa-9f99-31a9cb9643ab`,
  `8411fc9f-2464-45c0-ad0a-6f7f8841b8a9`, `74e243c9-914e-41b7-9bc1-5631ba143577`,
  `b9f92afa-efbe-4593-859b-52ddd19ad3a6`, `c79eff47-d0e5-4250-a622-e0533ac56c92`,
  `23b86d4f-9b92-460e-bd50-f71a009dcf88`, `c57979d2-fb33-4824-a370-dfc0ebe24286`: 3042 passed, 18 skipped by the specs'
  own design, 6 failed; and the same suite with terminals in terminal hosts (only the specs listed in
  `e2e/host-terminal-specs.ts`), runs `e73aba9f-520c-400d-844f-2f1de83b71e8`, `faa86185-9d47-418b-8633-27b9612af67c`,
  `c572aa6c-e2f2-4ea4-83f7-7125c8682018`, `c08116ac-b22b-4023-a990-1f1a76c6dd0d`,
  `ffe0693e-0a67-4039-a8e4-4621365c4561`, `4ae30c9a-4e63-4ace-ad31-5c5da7d54ac0`,
  `42c9376d-fb3e-4620-85e7-b2c0fe9f16b2`, `538090f9-d51b-4683-bcd6-5990d0e63298`,
  `99125a81-b208-4643-b900-b50c321f1b39`, `db26ceb4-057d-4c4a-9a9f-727cdb7d6aaf`: 2082 passed, 75 skipped by the specs'
  own design, 3 failed. Of the nine failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `ec2dbf12`,
    `b9f92afa`, `c08116ac`, `538090f9`), on the Add host form's "SSH" and "Tensorlake sandbox" choices (see Things you
    should know).
  - The feedback dialog's disabled Send color on WebKit, in "the dialog refuses blank and over-long messages without
    sending" (run `c79eff47`), a recorded flake.
  - "Closing Settings with an unsaved recipe asks first and finishes the close" (WebKit, run `23b86d4f`), the failure
    that sent the plan back, fixed as described above.
  - Three single failures, none in the code the plan changed, each of which then passed three re-runs on the same build,
    also on sandboxes (the run ids below are the re-runs): "incarnation churn keeps a followed run while a target change
    detaches it" (WebKit; failed in `e6073bcc`, re-run `094b6771-a369-44dd-9d4b-45c33620a55f`); "suspending an agent
    with stale idle status asks before sending the request" (WebKit; its session was missing from an activity read, as
    other tests in the same spec did in earlier rounds; failed in `c57979d2`, re-run
    `2974246f-1a72-4475-a170-439dd5a5e91b`); and "typed page refusals are atomic and empty Enter leaves later bindings
    paired" (Chromium; focus did not return to the page body, as another quick-launch test did in an earlier round;
    failed in `ffe0693e`, re-run `479e3dde-6a6a-488f-ac2d-f9cb6a1d29ec`). Each is a single failure, not recorded as a
    flake.
- Not run: the doctests (no doc examples changed), the grid component's browser suite (no grid code changed), the
  desktop smoke tests and the installer tests (no desktop shell, packaging or installer code changed), the development
  screenshot review (no shot changed), the docs website preview (the build above passed; no page was viewed), the native
  Mac app (not runnable on Linux) and the real Tensorlake service (the browser specs use the existing fake).

The executor's report calls the PRs drafts; what landed is its commits rebased onto main `30981f76` (the report names an
earlier plan-bookkeeping commit as the base) with the landing's fixes above, each in the commit it belongs to. Its
statement that the final branches hold exactly the tested correction stopped being true when the landing added the
premise check to the new test. Its 20 clean WebKit runs of the close test came after 10 clean runs before the fix on its
own machine, so they did not by themselves show the fix; the landing's runs in Checks above did, on the sandboxes where
the failure had shown. Nothing else in it became untrue at landing.
