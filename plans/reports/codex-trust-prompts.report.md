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
