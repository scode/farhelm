# Reusing named launch templates

## What this was about

A quick-launch template that supplied a session name and a managed checkout worked once, then stopped at an occupied
directory. New Session opened with no usable preview, and Launch stayed disabled until the name was changed by hand.

The maintainer chose two kinds of template session name. A **base name** uses its literal name when free, then `-2`
through `-50` when its managed-checkout directory is taken. The session title and directory use the same number. A
**fixed name** stays exact and reports a clash. Existing template names become base names without an edit. Typed names
stay exact, and ordinary folder destinations never number names.

The stack implements that policy through the shared template application path, the Templates editor, Save as template,
agent template writes and agent-created checkouts. Saving a template-supplied base keeps its original name, not the
number resolved for one launch. The launcher's marker explains numbering; typing in Name takes control of the name. A
create that loses its approved directory reports the conflict and never submits another create automatically.

## Things you should know

The update path is unchanged, existing templates keep working as base names, and there is no database migration. Base
stays absent in saved definitions and normalized write replies. Fixed is the only stored new value.

Once any template has a fixed name, a build from before this work cannot list or use **any** template: no ⚡, number
keys, sequences, `tl:` or an agent's `--template`. Its Templates dialog cannot list the catalog either, so it cannot
repair this itself. Before going back, switch that template to base or delete it on the newer build. This also applies
to returning from a development build that saved a fixed name. The maintainer accepted this limit; it is omitted from
the changelog as requested.

The supervisor connection protocol changes from 60 to 61. Without that version change, an older supervisor on the same
protocol would reject the new fixed-name field while decoding a template reply, ending the whole helm connection and
refusing its waiting agent requests. The version change refuses that peer cleanly at connection time instead. The normal
SSH host update path remains available.

Agents can choose a kind with `template create` or `template edit --title-kind base|fixed`. Editing only the title keeps
its saved kind; editing only the kind uses the existing title. Template listings show the effective kind in text and
JSON. JSON discovery remains schema 6: the addition spells out the optional default without changing existing fields'
meanings. Agent-created base checkouts choose their name before approval, while repeating a create with the same
idempotency key returns its original checkout before another preview or card.

The docs page **Using Farhelm → Launch templates** describes the controls and saving behavior. The final PR removes the
repeated-quick-launch TODO entry. The unrelated host-dialog plan is being handled separately.

## Open questions and possible follow-ups

None required for this plan. Numbering remains bounded at `-50`; invalid or over-long names still refuse. No automatic
create retry, new reservation or supervisor-side naming mechanism was added.

## PRs

Four open draft PRs form a linear stack, ready for the separate landing monitor:

- [#2325](https://github.com/scode/farhelm/pull/2325/changes) — shared base/fixed names, launcher numbering and the
  reproduce-first regression; head `9bf4dd879dfc`.
- [#2328](https://github.com/scode/farhelm/pull/2328/changes) — editor/save choice, preserved original bases and
  approval wording; head `9a698bd65583`.
- [#2330](https://github.com/scode/farhelm/pull/2330/changes) — agent template flag, edit semantics, listings and
  instructions; head `a0916cdf49bb`.
- [#2332](https://github.com/scode/farhelm/pull/2332/changes) — agent-created Base checkout numbering and keyed replay;
  head `521a403a3c16`.

## Checks run, reused and skipped

Runtime commands used the run recorder. Rust used pinned nextest 0.9.148 and the pinned tmux, four slots and zero
retries. Browser commands ran from `e2e` on Chromium and WebKit, one worker and zero retries. Selected-out Rust cases
are not coverage. Passing records below have complete reports and no runtime `SKIPPED` marker.

### Rust

- `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-ui -E 'test(launcher::) | test(github_checkout::) | test(templates::) | test(list::create_form::) | test(template)'`
  passed 138 in `d8661cf9-bd38-4ffa-8edf-a8e95816c82e`. This covered PR1's shared application, naming and store units
  before its final preview-revision integration correction; browser validation below covers that correction.
- `cargo nextest run -p farhelm-proto --lib -E 'test(protocol_version_is_pinned_at_61)'` passed one in
  `583283eb-422c-4fba-99ff-e94842db1ab3`. Reused: the protocol pin and decoder contract have not changed since PR1.
- `cargo nextest run -p farhelm-ui --lib -E 'test(list::templates::) | test(list::save_template::) | test(approvals::tests::a_template)'`
  passed 28 in `5957ed0e-aba3-4507-811d-66784337c4be`, covering PR2. Reused: its editor, save and approval code is
  unchanged by PR3/4.
- `cargo nextest run -p farhelm --bin farhelm --test agent_cli -E 'test(template) | test(agent_instructions::) | test(render::tests::)'`
  passed 24 in `df00d152-44cc-4706-b4f2-b0f1027cc798`, covering PR3's parser, real CLI payload and both listing formats.
  Reused: PR4 changes agent-create resolution and manual prose, leaving those template flags and listing paths intact.
- `cargo nextest run -p farhelm-helm --lib -E 'test(template)'` passed 32 in `4c8e8d9d-3eb5-4333-9b85-bbe4cee5d1c2`,
  including PR3's title-only and kind-only edits, orphan refusal and normalized Base reply/store. Reused: PR4 leaves
  template writes unchanged and separately checks launch resolution.
- `cargo nextest run -p farhelm-helm --lib -E 'test(=agent_requests::tests::template_base_checkout_names_are_previewed_before_approval_and_replayed)'`
  passed its one exact repaired case in `88ed9d23-c64e-426d-bfa3-812711582965`.
- `cargo nextest run -p farhelm-helm --lib -E 'test(agent_launch::) | test(agent_requests::tests::template_base_checkout) | test(agent_requests::tests::managed_checkout_recovery) | test(agent_requests::tests::guarded_wrapper_checkout)'`
  passed nine in `36fd5c03-d670-4ffe-b86d-797117347202`. This covers PR4's resolved policies, numbered approval and
  dispatch, cap/error boundaries, original keyed recovery and its host/wrapper guards.
- `cargo nextest run -p farhelm-proto -p farhelm-ui --lib -E 'test(launcher::) | test(github_checkout::) | test(list::create_form::)'`
  passed 77 in `0f6d5804-c576-4da3-8d82-20d5879bd4a8`. This checks the shared series, cap and reconciliation freeze,
  template application and the GUI's unchanged effective-name and destination behavior after the helper moved.
- `cargo nextest run -p farhelm --test e2e -E 'test(github_checkouts::)'` passed all four real checkout tests in
  `1d01160c-afe2-4c0b-ac14-b94d34e77cff`, on PR4's final executable source. This covers numbered Base creates and keyed
  replay beside the unchanged explicit-title refusal, actual Git clones, checkout-root creation, and recovery after a
  lost reply or helm restart. JUnit reports five passes because it includes the fixture-build setup; 383 selected-out
  cases are not coverage. The pinned tmux was 3.7c and its executable hash matched.

### Browser

- `npx playwright test tests/github-checkouts.spec.ts tests/github-checkout-composer.spec.ts tests/clone.spec.ts tests/quick-launch.spec.ts tests/tooltip-coverage.spec.ts`
  passed 176, 88 per engine, in `052cea02-4b35-4a26-ac44-b2968cc396c4`, on PR1's final source before publication. It
  covers repeated named quick launches, fixed/typed clashes, folder behavior, lost races and Clone.
- `npx playwright test tests/templates.spec.ts tests/github-checkouts.spec.ts -g 'session name kind survives|save as template captures|save as template keeps the'`
  passed 12, six per engine, in `0b96510a-f0de-48ed-8cad-d47ffe749397`, covering PR2's new editor/save interactions.
  Phone-width WebKit screenshots were inspected for wrapping and clipping.
- `npx playwright test tests/templates.spec.ts tests/agent-relay.spec.ts tests/tooltip-coverage.spec.ts` passed 108, 54
  per engine, in `a6ceb5fa-6720-428f-bb76-e44f2fefc8a2`, on PR2's final source before publication. This covers
  Templates, approval cards and hover help. The later documentation correction changed prose only.

These browser commands are reused through PR3/4: the remaining behavior changes affect the agent CLI and helm agent
create. PR4 moves the existing pure naming sequence without changing its candidates, cap, freeze or GUI source/target
state. Its Rust tests check the moved helper and GUI reads; another browser battery would repeat the same behavior.

### Builds and source checks

`cargo build` and the release web command `dx build --package farhelm-ui --platform web --release` passed before the
browser runs; the latter was rebuilt for PR2. PR3's shipping CLI compile and PR4's shipping, helm-test and real
e2e-target compile checks passed. The website command `bun install --frozen-lockfile && bun run build`, from `website`,
passed for the changed Launch templates page, including its final wording correction, with internal links valid.

`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
`cargo check -p farhelm-ui --features desktop` passed for PR4. The subsequent correction changes only a fake peer's
directory assertion and reply; product inputs to shipping and desktop checks are unchanged.

`cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`, the isolated-interpreter
`python -B scripts/check-test-sleeps.py` and `python3 scripts/check-agents-md.py` passed. The sleep check inspected 304
delays with zero missing rationales; instruction references had zero errors or warnings. Direct dprint checks of TS/MDX
selected no files and are not claimed as passes; the website build validates the docs page.

### Retained failures and scope

The reproduce-first browser regression failed on unchanged product code on both engines in
`c8cd0185-2edb-40e1-bb4f-46fb36c3bdc6`. Its Chromium post-cleanup snapshot weakened the refusal diagnosis, so the same
test was strengthened with a real occupied-preview premise and live refusal capture. It failed again on both engines in
`21b4673c-8c97-481d-b9ba-25c0cc2f0e40` before product edits; the same test passes in the final browser gate.

Broader browser attempt `894a081e-dc27-4fd5-adec-45c9f7671d76` was interrupted after a pre-existing Clone assertion
expected a tooltip attribute removed by earlier main work. It recorded 42 passes, one failure, one interruption and 132
unstarted cases. The test now checks displayed and saved directories while preserving its command assertion. The exact
repaired case passed both engines in `a39427ec-5691-48b9-a9df-9072d9368359`, before the complete 176-case gate.

PR4 development attempt `9beb38fe-445d-499f-8f39-2c74a01b3616` was interrupted during compilation for a new fixture's
wire-field correction; no tests ran and no report exists. Attempt `4c83e070-d3db-4520-8d6a-7685198444e3` was interrupted
after 80 passes while the new fake asserted the ordinary directory field instead of the managed-checkout binding.
Neither command counts as a pass. Their evidence is retained privately; these are same-session development corrections,
not latent-flake claims.

The full workspace and browser suites, installer/provisioning, desktop runtime, releases and hosted CI were skipped: the
selected checks cover naming, storage, editor/save, approval, CLI and checkout/retry behavior. No corresponding
installer, provisioning, release or desktop-runtime mechanism changed. No live installation was modified.

The final stack was carefully rebased from `9ba6b0e932af` onto `f6558b077600`. The compared full tree delta contains
only upstream shortcut wording in the spec/docs/changelog and TODO, triage and plan administration. The template,
numbering, approval, CLI and test inputs are unchanged. The successful commands above are reused across that rebase; new
hashes alone do not call for more runtime checks. Final source checks passed after the implementation was complete.

## Review gate outcome

Each PR had the required single fresh-context gpt-6.1-sol review at high effort, with the complete test-authoring
checklist. PR1's missing preview revision signal and two browser coverage gaps were corrected and reconciled. PR2's
documentation omission was corrected. PR3's explicit-Base write reply was normalized and its regression passed. PR4's
review found no remaining issue. No review swarm was used. Commit/PR wording went through separate cold reads.

### Landing

Landed as #2325, #2328, #2330 and #2332, merged bottom-up, with one small styling fix made during the landing (below).
No other plan landed in the same round.

#### Going back to an older version breaks more than the report says

The report says that once any template has a fixed name, an older build cannot list or use any template. Reading the
code, an older helm loses more than that, because one template it cannot read fails its whole template list, and it
reads that list in more places than the launcher:

- every `farhelm agent create` and every plain `farhelm spawn` fails, even one with no `--template`;
- agents cannot edit, delete or list templates, so an agent cannot remove the fixed-name template either.

Still working on the older helm: starting a session from New Session in the app by filling in the form yourself,
`farhelm spawn --inherit-agent`, and `farhelm agent clone`. The way out is the report's: switch the template back to a
base name, or delete it, on the newer build before going back. This was found by reading the code, not reproduced on an
older build.

No decision is needed for the changelog: the repository's changelog rule is that no entry mentions that a release cannot
be downgraded from, so this stays out of the draft like the narrower version the maintainer already accepted.

#### Release relevance

The supervisor connection protocol goes from 60 to 61. Protocol 60 shipped in 0.30.0-rc.1, so anyone on rc.1 gets a
supervisor update with the next release that includes this plan. The open 0.30.0-rc.2 release PR (#2333) is based on a
main from before this plan, so rc.2 as it stands does not include it; whether rc.2 should be rebased to pick it up is a
release decision.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained the plan that shows host setup and removal as numbered steps and keeps host refusals
in dialogs (#2298 through #2317), three fixes to dragging the Mac app's window (#2318, #2319, #2320), the switch that
stops agents from sending prompts to running sessions (#2336), and docs, instructions and plan bookkeeping. During the
round, before the stack merged, main also gained a trailing code review record (#2334), which adds findings to the
review queue and changes no code. A fresh-context reviewer and I read each against the stack:

- The host dialogs and the window-drag fixes touch the hosts panel, host setup and the Mac window frame; none of them
  touch templates, how the launcher names sessions, or agent creates. The new launcher styles do not collide with the
  host dialogs' styles.
- #2336 removed the prompt command from the instructions that Farhelm gives agents; this stack adds a paragraph on
  choosing base or fixed names to the same instructions. Both changes survive on the merged result.
- The template shortcuts that landed earlier in the day (number keys and ⚡ in the launcher, sequences, Cmd-J) apply
  templates through the same shared step this stack extended, so they number base names the same way, and applying a
  template still clears what the launcher kept from an earlier launch whose reply was lost, so a template shortcut
  starts a new launch instead of replaying that one, as those shortcuts need.
- No other open work changes the protocol.

The only rebase conflict was in TODO.md, between this stack's removal of its own entry and other edits on main; both
sides' changes are kept. The landed code is identical to what was tested after the landing fix below.

#### Fixes made during the landing

- The launcher's marker beside the Name label (the small badge saying a template supplied the name and that it will be
  numbered) used a hard-coded fully rounded corner. The UI's JavaScript tests include a check that the stylesheet uses
  only its named values, and it rejected this; the executor had not run those tests. The marker now uses the
  stylesheet's rounding for small badges, 8 pixels. On a marker about 17 pixels tall that is nearly fully round, so it
  should look the same; no screenshot was taken. Fixed in #2325's commit.

#### Possible follow-ups

- When a template's base name is taken, the launcher tries the next number in the background before showing the preview.
  The executor's browser tests ran this against a local host only. On a slow SSH host it may take visibly longer before
  Launch is enabled; this was not tried. Trying a repeated quick launch against a real remote host would settle it.
- Unrelated to this plan: `dprint check` fails on main because of a formatting issue in the UI crate's `Cargo.toml` that
  came with the Mac window-drag fix #2318. A CI run, which happens only on demand, would report it in its dprint job.

#### Checks

- Run now, on the rebased stack: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, and the isolated
  test-sleep check (304 delays, none without a reason). Through the test recorder with pinned nextest and tmux, four
  slots and no retries, the whole protocol, helm, supervisor and UI libraries, the `farhelm` binary's own tests, the
  agent command-line test file, and the managed-checkout, agent-listing and structured-launch end-to-end tests: 3327
  passed, run `dfd0f447-75b1-4cbb-9ef5-aac956e44d4e`. The executor had run selected tests only; these are whole
  libraries because the stack changes the protocol and template handling across all of them. The run's output has no
  runtime `SKIPPED` marker, so the end-to-end tests ran rather than returning early.
- `node --test` in the UI's JavaScript test directory through the recorder failed once on the stack as delivered (run
  `b4167102-447a-4810-a122-d144ee3e8516`, the marker's rounding above) and passed after the fix: 310 passed, run
  `ef7bee95-6d5c-4a01-8a2f-d5865f4be13d`. The Rust results above still apply after the fix, since it changed one
  stylesheet value that no Rust test reads.
- After the fix: the website build, the release web build, and
  `npx playwright test templates.spec.ts quick-launch.spec.ts launcher-template-shortcuts.spec.ts template-list-shortcut.spec.ts github-checkouts.spec.ts approval-layout.spec.ts save-template.spec.ts`
  through the recorder on both engines, one worker, no retries: 244 passed, run `0c748a23-78e0-4fbe-80ba-2e0acd773e96`.
  No `save-template.spec.ts` exists, so that name selected nothing; the Save as template cases live in the templates and
  checkout files, which ran. These cover the stack's launcher numbering, the Templates editor, Save as template, and the
  template shortcuts and approvals that share its code.
- On the merged main: `python3 releasing/check-changelog.py format` passed; `dprint check` passed for every file except
  the unrelated `Cargo.toml` above.
- Skipped: the full workspace and browser suites, installer, provisioning, desktop runtime and doctests, for the
  report's reasons, and `python3 scripts/check-agents-md.py`, since the stack changes none of the repository's own
  agent-instruction files (the instructions Farhelm gives agents are covered by the `farhelm` binary's tests above).
  Nothing that landed since the stack was based changes those areas in a way this stack depends on.

Nothing else in the executor's report became untrue at landing.
