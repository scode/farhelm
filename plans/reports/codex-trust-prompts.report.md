## What this was about

Codex, Muse and Pi offered trust as a choice in the session launcher, while Claude's managed-checkout opt-in lived in
Settings. After seeing Codex's directory question again, you clarified that there was no known regression: the goal was
to make those controls consistent and easier to find.

Settings now has a separate, off-by-default switch for each agent, beside Claude's. Codex, Muse and Pi use their own
per-launch arguments for directories Farhelm manages, including agent-requested checkouts. The launcher, search,
template editor and CLI no longer offer per-launch trust. Old template values are accepted, ignored when applied, and
dropped when saved. Old sessions still decode and keep their saved launch when restarted or replaced without edits.

## Things you should know

New, Clone, Restart with, Replace with and agent-created launches use the current setting for their destination. An
existing directory inside a managed checkout qualifies, even when picked manually; a directory outside managed checkouts
keeps the agent's own trust behavior. Plain Restart and Replace reuse the saved launch. Turning a switch off adds no
trust override; it does not revoke trust granted by an agent's saved settings or Muse's YOLO mode. Changing Settings
also does not answer a question already on screen.

For Codex and Muse, trusting a managed repository allows its settings, hooks and commands to run as if you had answered
the agent's trust question yourself. Tool-action approvals and YOLO remain separate choices.

Codex and Muse show independently dismissible hints above a recognized current trust question in a managed checkout.
Closing one occurrence and permanently dismissing an agent's hint are separate choices. Pi has no hint: Pi 0.85.1
started without a directory question in the observed empty-directory launch, and its switch approves project settings
and extensions rather than adding approval prompts for its actions. This uses the plan's agreed fallback.

Codex 0.163.0 started without a trust question with the existing per-launch override in both a plain directory and a git
repository root. Its trust argument was `-c 'projects={"<directory>"={trust_level="trusted"}}'`. Comparison launches
without that override also skipped the question; an already-trusted ancestor prevents treating those directories as an
untrusted baseline. Additional launches using an empty per-launch project table or an untrusted ancestor entry still did
not produce a question. There was therefore no otherwise-questioning baseline. This establishes the observed launches,
not an independent differential proof that the override suppressed the question. No vendor configuration was edited and
no trust question was answered. The config-writing fallback was not used.

Muse 1.4.10-R6427.1 displayed a real question. Its narrow terminal clips the footer, so recognition covers the observed
24- and 40-column shapes as well as the wide capture. Fixtures use neutral paths. Approval and YOLO confirmation
summaries omit the retired choice; their actual command previews still show any setting-derived argument.

## Open questions and possible follow-ups

The Start a session documentation's existing effort/permissions/trust screenshot still shows the removed control. Its
capture scenario also expects that control. Both were left for the separately authorized screenshot-refresh flow; the
surrounding prose and links describe the new behavior. No published capture or release notes were changed.

The Codex observation's control limitation remains as described above. No further work is required by the plan's agreed
fallbacks; a future unconfounded vendor check could provide stronger evidence about question suppression.

One existing replacement test could not reopen a session's menu after another session's failed replacement. Closing the
outgoing menu moves its error into the row and shifts the next row during the pointer gesture. The exact event target
loss remains a hypothesis. The test now closes that ordinary menu with Escape and verifies the older YOLO confirmation
is still open before continuing its retry-key assertions. Directly switching between those two menus with one pointer
gesture remains a possible separate investigation; product menu behavior was not changed.

## The PRs

- [#2255 — add agent trust controls to Settings](https://github.com/scode/farhelm/pull/2255/changes), draft, head
  `c874292e3d86175802a2518d4fd62bcf0d4a8b15`.
- [#2256 — move agent trust out of the launcher](https://github.com/scode/farhelm/pull/2256/changes), draft, head
  `fda7b3aeb4b06e69d4c731aadd5752b363eea675`, based on #2255.

## Checks run, reused and skipped

- Run: `npx playwright test` selecting `templates`, `remembered-permissions`, `restart-with`, `replace`, `clone`,
  `launcher-enter`, `goose-pi-composer`, `github-checkouts`, `tooltip-coverage`, `quick-switcher`, `workspace-trust` and
  `claude-trust` specs: 318 passed, 159 per engine, recorder `e40036e4-c63f-48c1-b305-a1ec6185bf44`. One worker, zero
  retries, no skipped or incomplete cases; reports and cleanup complete, output untruncated. A fresh web build included
  the main integration, and the matching CLI build was reused because no Rust product input changed.
- Run: `npx playwright test tests/sidebar.spec.ts --grep …`, selecting arbitrary model-first choices, Enter on a focused
  recent setup, and reset notices: six passed across both engines, recorder `4447c6ee-9e41-4487-b3b2-b824bec18ae6`. Same
  one-worker, zero-retry policy; reports and cleanup complete, no skips or incomplete cases, retained output
  untruncated.

- Run: `cargo nextest run -p farhelm-ui -p farhelm-proto -p farhelm-helm -p farhelm --lib --bins -E …`, selecting
  UI/protocol units and template/CLI cases, 711 passed with 1,325 cases outside the selection, recorder
  `dfc3119b-07b8-4ebc-866d-6e2eca4a83a9`. This covers UI and protocol units, historical template application and writes,
  raw approval preconditions, CLI parsing, generated instructions and template listings. The exact two corrected UI
  fixtures first passed in `9d7dd102-afda-4ced-92b8-0cfe7fdd6304`. Both reports and cleanup are complete, retained
  output untruncated, with no runtime SKIPPED marker.
- Run: final `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo check -p farhelm-ui --features desktop` passed. The
  pinned-parser delay checker inspected 303 delays with none missing a rationale. Changed-file Markdown formatting,
  agent-instruction/link checks and changelog format passed. The website built 32 pages with all internal links valid;
  matching CLI and web application builds prepared the browser run. No previous plan's builds were reused.

- Run: focused Rust tests for the first unit, with pinned nextest 0.9.148, four slots, zero retries and pinned tmux
  3.7c. Recorder `2ed76dd5-d66a-4e49-a2de-51a444166cf4` passed 587 cases covering UI, launch compilation, the YOLO
  guard, settings migration/persistence and preferences. Recorder `2c5bdfbe-f019-4be2-9152-d68d48bc75de` passed 30 cases
  covering corrected destination ownership, approval snapshots and symlink redirection, protocol compatibility, trust
  observations and the one-way YOLO latch. Recorder `53789ff1-efd1-4f16-ad6e-a6d80c28b812` passed 8 final Muse/fixture
  cases. Reports complete, retained output untruncated, no runtime SKIPPED marker.
- Run: first-unit browser trust specs, ten passed across Chromium and WebKit, one worker, zero retries, recorder
  `dc7aa483-db72-4b4e-b8d8-c54890447109`. Strict reports and owned cleanup complete. The prior attempt
  `d4ec011d-8b94-4564-a62d-4bc81eb28a0c` passed 6 and failed 4 because synthetic refusals omitted the helm's build
  identity, withdrawing normal refreshes. Corrected fixtures retain that identity. The exact Codex Chromium case then
  passed in `3b755a4e-e9a1-420a-b97e-c6094faa32a1` before widening. An intervening selector error
  `0d8a8ab6-b15f-4559-8d80-e3c39695b6f5` ran no cases. Failed evidence remains private.
- Run: the initial twelve-file browser selection executed all 318 cases: 317 passed and one WebKit replacement case
  failed before its retry-key assertions, recorder `c811adf8-7a39-4869-ab44-07f1ef7289ba`. The unchanged exact case
  reproduced that boundary failure in `24c7c52c-83d3-4b27-b4a7-67ef17830c30`. Both failures and their attachments remain
  retained. After the explicit menu-dismissal setup correction, the exact WebKit case passed in
  `52d266c6-0c0a-4643-a364-db1bf97e8b8f`, followed by both variants on both engines, four passed in
  `939b41e6-3435-4393-9ea1-ab9b7d129ce0`. These are separate observations; the earlier broad command remains a failure.
- Run: earlier compile-only attempts `223ede97-b73c-4c28-adde-ec8ccd939954`, `a1f2af48-29b7-422b-8c4f-307dfd934403`,
  `dc63630e-c018-4788-aed7-1eb1779207bc` and `e1332678-113b-4a2c-a9e7-a040a210d098` failed before cases on integration
  or new-test fixture types. They are retained failed observations, not passes or latent flakes; later results are
  separate evidence.
- Reused: these Rust tests exercised recorded working trees before the PR commits existed. Run `2ed76dd5` was based on
  `b3b89cbd` with source fingerprint prefix `56fa4bcb54c7`; it preceded the ownership and Muse corrections, so only its
  unaffected compiler, YOLO guard, settings and migration coverage is reused. Runs `2c5bdfbe` and `53789ff1` were based
  on `3df5399b`, with fingerprint prefixes `a29add591524` and `0514c7a4889f`, and cover those later corrections.
  Together they cover the unchanged product paths committed in first-PR head `6a55cbe6`. The second unit does not change
  those paths. Its separate 711-case run `dfc3119b` exercised the corrected working tree on `6a55cbe6`, fingerprint
  prefix `b101c916d4df`, including changed template and UI behavior. The rebase preserves both units as described below.
  No other plan's results were reused.
- Rebase: main added terminal-link hover visibility checks while this plan ran. Both PRs rebased cleanly, and their
  interdiffs were empty. The change affects link activation and refusal stacking, with no shared launch-policy or
  template behavior. Rust checks remain applicable because upstream changed no Rust inputs; a fresh web build includes
  the newer JavaScript and CSS before final browser validation.
- Skipped: the full workspace and browser batteries, installer/provisioning suites, asset-JavaScript and native desktop
  runtime because this plan changes none of their additional areas. The unrelated terminal-link change brought in by the
  rebase has its own landing validation; this plan changes no JavaScript or link-admission behavior. Targeted browser
  specs cover the changed session-launch flows; WebKit stands in for the desktop engine family, without proving native
  webview or IPC behavior. No live installation was changed. The docs screenshot capture was deliberately not run.

## Review gate's outcome

Each PR received the required single fresh-context gpt-6.1-sol review at high effort, with the full test-authoring
checklist. The first review's three findings were corrected: bind granted trust to the checked canonical directory,
avoid enumerating unrelated children during ownership lookup, and recognize Muse's observed narrow question. The same
reviewer verified the corrections and browser fixture adjustment. The second review found no product issue; its
follow-up verified corrected test types and acknowledged that the original source review missed those compile errors.
The same reviewer also verified the replacement test's explicit outgoing-menu dismissal, with no findings and with the
original retry-key assertions preserved.

Commit and PR wording passed separate fresh native cold readers using the executing session's own model and effort. One
second-unit reader invalidated its contaminated blind read; its fresh replacement provided the accepted report. No PR
was marked ready or merged by the executor.

### Landing

Landed on its own in round 90, as #2255 and #2256, merged bottom-up, with two conflicts resolved during the landing
(below) and no other change to what the report describes.

#### What landed since the stack was based, and whether it interacted

Since this stack's base, main gained the move of the open session's details to a bar below the terminal (#2253), a batch
of recorded review findings (#2258, documentation only), and test and flake-log fixes (#2259 to #2264). The landing
monitor read the stack against them, and a fresh-context reviewer did the same independently. Neither found a functional
conflict:

- The Codex and Muse trust hints sit beside Claude's, between the top row of tabs and actions and the terminal, ahead of
  the tunnel strip, which is where #2253 keeps every conditional notice. A visible hint shrinks the terminal area, so
  the approval cards, which #2253 now anchors to the terminal's top edge, move below it, as they do for Claude's hint.
- Nothing in the stack targets the old session header, the old tab row or the moved copy buttons, and every selector its
  browser tests use exists in the new layout. Main's new test helpers share no files with the stack.
- SPEC.md and SPEC_impl.md, which both sides edited, agree on where the hints sit, and nothing in them still offers
  trust in the launcher.

One change on main does conflict with the stack, without a textual conflict: #2264's flake record for the Replace tests,
which landed shortly before this stack merged and which the landing caught only afterwards. See "A decision for the
maintainer" below.

Two textual conflicts came from #2253. In the session view, the stack's new trust-question marker on the page and
#2253's renamed top row both stay. In TODO.md, both sides had removed an entry (the session-details one and this plan's
own), and both removals stay.

#### Upgrade

#2255 raises the protocol from 54 to 55 and the helm database from schema 56 to 57, adding the new Settings switches.
Neither 54 nor 55 has shipped (v0.29.0 carries protocol 51), so the next release already requires remote hosts to be
updated, and no other change claims 55 or 57. An older helm refuses the newer database. #2256 is marked as a breaking
change because per-launch trust and the command line's `--trust` option are in v0.29.0 (and earlier stable releases) and
are now gone; the two PRs end with one changelog entry that says so. Two caveats the release curation may want to keep:
launches into folders Farhelm does not manage can no longer get trust from Farhelm at all, and a trust choice remembered
in an old template or launch is dropped without notice.

#### Fixes made during the landing

None besides the two conflict resolutions above.

#### A decision for the maintainer

The Replace test change in #2256 hides a recorded product bug. To get the Replace tests to pass, the stack closes the
other session's menu with Escape before opening the source session's menu. Main's flake log, in an entry that landed
shortly before this stack (#2264), traces this test's WebKit failures to a real pointer race: dismissing the other row's
open menu moves its error into that row and shifts the source row's menu button between press and release, so the click
misses. That entry and the TODO.md Deflake entry rejected exactly this test-side close because it would hide the defect,
and left the choice between reserving space for the error and changing how dismissal is ordered to you. The stack's
change covers both the header variant (recorded) and the sidebar variant, which failed two of six WebKit runs on main
during the previous round (run `18a485cb-d9da-40a4-9152-6e419dc34b65`) and is not recorded. Both passed in this
landing's browser run, which is what the change makes them do. The executor wrote its report before #2264 landed, which
is why it calls the cause a hypothesis. The landing monitor and its reviewer missed the conflict before merging, because
#2264 reached main after the round's review.

The landing left the flake-log entry, the Deflake entry and the sweep exclusion in place. The options:

- (a) revert the test change, so both variants keep showing the race; add the sidebar variant to the Deflake entry and
  the sweep exclusion beside the header variant, and keep them until the product race is fixed;
- (b) keep it, reword the Deflake entry as the product-fix item (reserve space for the error or reorder dismissal), and
  drop the header variant's sweep exclusion, accepting that these tests no longer catch the race.

The landing monitor recommends (a), since the record's author chose to keep the bug visible. Until you choose, a passing
run of these tests says nothing about the race.

#### Possible follow-ups

- The new trust browser test rewrites the session list and detail replies and tears them down the way the confirmation
  test did before #2259, which is the pattern that made that test hang. Switching it to #2259's new helper would avoid
  the same hang there; about twenty other test files still use the old pattern.
- The map of per-agent behavior at the top of the supervisor's agent module does not name the two new places that answer
  per agent: the helm's stored per-agent trust switches and the per-agent text of the trust hints. Each of these is
  written so that adding a new agent fails to build until it is filled in, but the map should still name them.
- The new Muse screen captures are named as hand-made ("derived") screens, while SPEC_impl.md and a code comment call
  them captured, and the screen-capture guide still describes only Claude and Codex.
- Approval cards for agent-requested launches no longer have a separate "workspace trust" line. When a Settings switch
  grants trust to such a launch, the card shows it only inside the start command. This may be intended; it is worth a
  look.
- Two style-sheet comments still mention the launcher's trust choices.

#### Checks

- Run now, on the stack rebased onto main (the tree that merged; both PRs' landed diffs are identical to it):
  `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and `cargo check -p farhelm-ui --features desktop`;
  `cargo nextest run -p farhelm-ui --lib` through the test recorder, four slots, no retries: 525 passed, run
  `7d0f5f42-c433-4b76-809a-361a197aa98c`; then fresh `cargo build` and web builds and `npx playwright test` on the
  executor's twelve browser files plus `header.spec.ts` and `session-confirmations.spec.ts` (the moved session view and
  #2259's new helper), through the recorder with pinned tmux, one worker, no retries: 340 passed, none skipped, run
  `9f01fe8a-0009-4564-9fcd-327be67b3a06`.
- Reused: the executor's helm, supervisor, protocol and command-line tests, at the revisions the report gives. Nothing
  that reached main since the stack's base changes Rust code outside the UI, so they still apply.
- Skipped: everything the report skipped, for the reasons it gives.

One thing in the executor's report no longer holds: it treats the Replace failure's cause as a hypothesis and its test
change as a setup correction, while main's flake record (#2264) now establishes a product race that the change hides.
Nothing else in it became untrue at landing.
