## Since the last review

PR #2829 now includes the corrections requested when landing stopped: the keyboard-shortcuts dialog and docs explain
typed names and their editing keys, bare digits are distinguished from digits within a word, the hint uses a named theme
radius, and suggestions independently cover terminal link destinations. Its link regression tests both file and web
activation outside the hint's own rectangle. PR #2828 is rebased without changing its reviewed patch.

The earlier report's overview and validation conclusions are superseded. The shortcuts overview is on main and is now
integrated. The final radius change needed a fresh stylesheet check, and the revised shortcut help needed a new website
build and browser checks.

## What this was about

Quick-launch sequences previously combined only templates assigned to number keys. The same prefix now accepts template
names separated by spaces: Cmd+X, `myproject codex`, Enter in the Mac app, or Ctrl+Shift+X elsewhere. On the page this
starts the combined setup; in the session launcher it applies the choices without starting a session. Later templates
win where their choices overlap.

## Things you should know

Names match by exact capitalization first, then a unique exact name ignoring capitalization, then a unique prefix.
Unknown or ambiguous words refuse the whole sequence and name the problem. The amber hint shows queued templates and the
unfinished word, with up to five matching names beneath it. Tab queues a unique completion; Backspace edits only the
unfinished word. Enter reads current templates even while suggestions are loading.

New or renamed template names cannot contain whitespace, including nonbreaking spaces. Existing names remain usable,
save unchanged and retain delete Undo. Tab can queue a spaced name as one entry when its first word identifies it
uniquely; with both `my project` and `myproject`, use the spaced template's number key or the launcher. A name beginning
with a digit also needs its number key or the launcher because a leading bare digit chooses a binding.

The Launch templates and Keyboard shortcuts docs pages explain these choices. The combined changelog entry covers both
PRs, and the original TODO entry is removed. The rebase preserves the newly landed shortcuts overview, Settings gear,
release curation and local Podman image support. The only text conflict in the earlier rebase kept all completed TODO
removals; the final local-image rebase applied cleanly.

## Open questions and possible follow-ups

No required behavior was omitted. Chromium and WebKit exercise the web client's shortcut forms; this does not establish
native Mac keyboard dispatch. Browser-reserved chords retain their existing behavior. The existing docs screenshot
configuration supports Chromium only.

## The PRs

- [#2828 — require single-word template names](https://github.com/scode/farhelm/pull/2828/changes), based on main.
- [#2829 — type template names in launch sequences](https://github.com/scode/farhelm/pull/2829/changes), based on #2828.

## Checks run, reused and skipped

The corrected browser selections passed before the final local-image rebase, on source fingerprint
`8e9cb58652a908381a5a09086e3cdeecb62414800e8235848517ba35c970073c`:

- `npx playwright test 'keyboard-shortcuts\.spec\.ts' 'quick-launch\.spec\.ts' 'launcher-template-shortcuts\.spec\.ts' 'templates\.spec\.ts'`:
  254 passed, 127 per engine, in `192bdb84-ba92-45cd-8867-c396c7aa3a72`.
- `npx playwright test 'terminal-link-clicks\.spec\.ts' -g 'the quick-launch .* armed during a press'`: four passed, two
  per engine, in `89878bd9-95d5-46a3-9a57-960d3b85a1c9`. Both file and web destinations refuse under suggestions outside
  the original hint's rectangle, then work after cancellation.
- UI JavaScript (`node --test`): 374 passed in `1695a514-c223-4987-9ba4-5fd5577a15ec`, including independent palette
  coverage and the stylesheet's literal/value rules.

All three runs have complete source identity and retained output, normal cleanup, and zero skips, failures or unstarted
cases. All eight page/launcher candidate captures at 1280px and 390px, plus all four platform/engine overview captures,
were visually inspected.

`cargo build -p farhelm -p farhelm-fixtures` passed in `39c5aeee-d5d1-4e22-a6d3-a15fba31fad6`; the web release build
passed in `12b8e15c-bff8-45e5-b8c6-97a4624ccf4c`; `bun run build` passed 34 pages and internal links in
`7f19cb34-7065-4c42-b26d-f79538158ad2`. These complete runs use source fingerprint
`61ab7b70f8713e29f6e9a99b7c8fc7565f63e7386a139359e5447e707a228b45`. The only later change adds the new radius to the
JavaScript test's existing shared-geometry list; product, build and website inputs are unchanged. UI all-target Clippy
with warnings denied, Rust formatting, spec Markdown formatting, changelog format and whitespace checks passed. The
whole-checkout sleep checker inspected 327 delays with zero missing rationales.

The existing staged screenshot harness passed in `dec4372f-f24d-46cd-a199-1f4201ffb5cc`, on the corrected `8e9cb586`
source: `scripts/docs-screenshots.sh --only visual-review --no-build`, with the existing sentence-terminal scope. It
passed staging and all four dark/light, wide/650px-main-area cases, including both unbound sequence hints and their fit
assertions. The separate approval-source case was intentionally skipped by this scope. All 24 normal captures were
inspected, with detailed checks of the constrained hints. The harness's supported engine is Chromium; it was not
extended. Nothing was published. Complete source/output and normal cleanup were retained.

These corrected-source results are reused after comparing the final upstream delta. Local Podman image support changes
the container choice and protocol, so fresh backend/web builds and two container cases supply separate delta coverage.
It does not change typed matching, input ordering, help, CSS, link-cover behavior, JavaScript tests or the staged
screenshot cases. The earlier website build covers the edited keyboard guide; the local-image landing separately
validated its container guide. The old complete commands are observations of the fingerprints above, not claims that
those whole commands ran on the final tree.

After the final clean rebase onto main `7272a521c4315130e7c4c8b01374df0f042e46ed`,
`cargo build -p farhelm -p farhelm-fixtures` passed in `35809d60-4a57-4564-886c-b04c9213762c`, and the web release build
passed in `85fc2c5a-1625-43ad-a8f1-f9f97e339e92`. Both use complete source fingerprint
`3cac605ca3742631cf28eb8f9fb15f959171eaad90350dd29203e42bf3d0ed85`, with complete output and normal cleanup. This
refreshes the runtime inputs for protocol 67 and the new container image choice.

On that final source,
`npx playwright test 'templates\.spec\.ts' 'github-checkout-composer\.spec\.ts' -g 'container templates edit, apply, save|container choices require a managed checkout'`
passed four cases, two per engine, in `5ec3c153-3b2b-41e2-8ac0-fe7c26e1d1ae`. It covers saved container-template
edit/apply/save/clear and the required managed-checkout/image choices reaching a create request. Source, report and
output are complete, cleanup is normal, and there are no failures, skips or unstarted cases. A final fetch found only
another plan's delivery bookkeeping after that base; it changes no tested behavior.

The naming and resolver runtime checks are reused because the rebase preserves their behavior and these corrections
change only help, rendering and link-cover declaration:

- `cargo nextest run -p farhelm-proto -p farhelm-ui -p farhelm-helm -E 'test(/new_template_names_are_words|duplicate_names_are_free|agent_template_create_refuses_whitespace|a_new_or_renamed_template_needs_the_loaded_list/)'`:
  four passed in `1ff7bcc5-ab8a-4280-a804-d1e536df6985`, source
  `861a0e97f2b726fd87a7515dad941292ec2787d92a347404150ccaa38082af1e`.
- The historical-name save/Delete/Undo regression passed in `f1a9cf1e-e7bf-478a-b698-229790b7cc0f`, source
  `0159047fc2c43f72c2e528574f2540aaf6183f38a29dc333d584742e5118b6cc`. PR1's original code-review gate is reused after
  its rebase interdiff proved empty.
- `cargo nextest run -p farhelm-ui --lib -E 'test(launch_sequence::tests::)'`: 16 passed in
  `563d612a-274f-4d78-aeed-d78d0428b7a2`, source `e26f54d9d146ca5bd1fa8ce712b23fe64edcd328d41312071fc5da49b6c02db3`. The
  state, matching and relay logic is unchanged. Other cases were selected out; no runtime substrate skip supplied these
  passes.
- The original template-list-shortcut browser coverage is in `d918f0a8-7b9f-4465-9cf1-5c1c8bda8b86`, which passed all
  136 templates/template-list cases on both engines at source
  `a8a41e8bb51bc2389be53ec427405392645aa563ddb42600fa0c3d495832486b`. The template-list path is unchanged; the requested
  correction run covers the Templates editor again.

Failed and interrupted attempts remain retained. Earlier browser fixture failures and timing observations are described
in the working log; their partial passes are not reused as command passes. This correction's first JavaScript run,
`21fee985-95fa-47ae-bf31-7d17c39a75ff`, caught the new radius missing from the existing shared-geometry token list; the
list was corrected before the successful rerun. This is a same-session test correction, not a latent-flake claim.

Full workspace/browser batteries, installer, provisioning and native desktop runtime are skipped: the selected checks
cover the changed naming, sequence, help and link-cover behavior. No CI run was requested. The earlier landing's
intermittent WebKit setup-menu failure is not attributed to this plan; that case is outside this correction's selected
coverage, and the Decision's main comparison remains necessary if it recurs with the plan.

## The review gate's outcome

PR1's unchanged patch retains its original single gpt-6.1-sol/high gate. The corrected PR2 passed a fresh single
gpt-6.1-sol/high adversarial review with no actionable findings. The review checked that the suggestions regression
isolates the child rectangle, observes the cover refusal before transfer/open, and admits the same links after
cancellation. The independent resume check and scope assessment found no unexplained work or unnecessary mechanism.
Existing commit and PR wording remains unchanged from its accepted cold reads.

### Landing

Landed on its second attempt, with no fixes at landing. Inside a launch sequence you can now type template names as well
as press number keys: type a name (digits inside a word are part of it), Space to queue it, Tab to complete a uniquely
matched name and queue it, Backspace to edit the word, and Enter to launch (or, in an open session launcher, apply the
setup), which also queues an unfinished word. A number key with no word in progress still queues that key's template.
New template names, renames and Save as template now require a single word, and Duplicate suggests `<name>-copy` instead
of `<name> copy`; existing names with spaces keep loading, applying, launching and deleting unchanged, with no
migration.

- #2828: single-word template names in the editor, Save as template and the agent's template create, and the new
  Duplicate suggestion.
- #2829: typed names in launch sequences, the suggestion list under the sequence hint, the shortcuts overview's rows and
  docs table, the specification text, the Launch templates docs page, tests and a changelog entry.

The first landing attempt (2026-10-30) sent this plan back: the keyboard shortcuts overview, which landed after this
plan was first based, did not list the typed-name keys; a stylesheet test rejected a literal corner radius; the
docs-screenshot visual review still expected the old hint; and a terminal link under the new suggestion list could still
be opened through it. The redelivery fixes exactly those: the overview's dialog rows and docs table list the typed keys
and correct the number-key rows, the radius is a theme value, the visual review expects the new hint (the executor ran
it on Chromium, the only engine it supports, and the longer hint wraps inside the pill without clipping), and the
suggestion list now covers links beneath it, with a test that a link under the list but not under the hint is refused.
The executor's first report had wrongly said the overview had not reached main; its redelivered report supersedes that
statement.

#### What landed since the stack was based, and whether it interacted

The redelivery was based after podman-local-image (#2825); since then container-host-account (#2832, the supervisor's
container accounts), the finalized and illustrated v0.32.0 release notes (#2830, #2833; release notes, docs-screenshot
specs and the changelog) and new TODO entries and plans landed. Rebasing onto that main conflicted only in TODO.md,
where the plan's removal of its own entry was kept beside main's new entries; none of the new docs-screenshot specs
expects the old sequence hint, and the screenshot scenario's template names are single words. Neither container plan
interacts: a template with container settings reached by typing its name goes through the same quick-launch path, and
the same image and checkout checks, as one reached by its number key. The shortcuts overview (#2827), which landed after
the first delivery's base, did interact; that was the main reason for the send-back, and the redelivery covers it.

#### Things you should know

- Nobody ran the new link-cover test with the fix removed, so it has not been seen to fail; its own assertions (the link
  sits under the list and not under the hint) make the logic sound.
- Characters typed with AltGr or Option cancel a sequence instead of being typed, and neither SPEC.md nor the docs say
  so.
- Cmd+/ on a Mac (Ctrl+Shift+/ elsewhere) during a sequence cancels it and opens the shortcuts overview; no test covers
  that combination.
- The overview dialog, unlike its docs page, has no in-sequence row for the number shortcut's chord (that gap predates
  this plan), and its ⚡ row does not say that a word in progress is queued first (new with this plan's words; the docs
  page's row does say so).
- Cosmetic: the dialog's key chip for typing is a phrase ("Characters (digits continue a word)"); some new lines on the
  Launch templates page run past 120 characters (dprint does not format `.mdx`).
- Four provisioning browser cases fail now and then under heavy machine load, on main as well (below); they are not
  recorded as flakes because every observation so far was made under that load.

#### Checks

This round landed only this plan, checked on main `73da69e1` and merged onto a main that had since gained only release
notes, docs-screenshot specs, TODO entries and plans (the rebase conflicted only in TODO.md). The first landing attempt
ran the full battery on the first delivery, stacked on podman-local-image: there the whole browser suite, run
`f69ef2f8-60e8-4dd5-9413-5380dc2deb9b`, failed only the WebKit case "expanded setup failure closes the menu below it"
(see below); the whole Rust workspace, run `c801e422-448d-45a7-8bfe-cda03b3e8874`, only the known flake "shallow pause
resumes without reset or replay"; the UI's JavaScript tests, run `8c0811c6-00f9-4db5-ac73-f524165f8984`, only the
stylesheet test this redelivery fixes; and the desktop-build checks, the desktop-build tests (run
`d12ee8fc-fcf3-4724-b807-362a166bfabd`, 1015 passed) and the desktop-asset check passed. On the redelivered stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint, the check that
  every test delay states its reason (`python -B scripts/check-test-sleeps.py`), `dprint check`,
  `python3 releasing/check-changelog.py format`, the docs website build (`bun run build` in `website/`), the workspace
  debug build and the UI's release web build: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `c496591b-2cba-4482-b594-ca729e0a4bf1`: all 832 passed.
  The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `03cf81b3-7fd3-4c02-a7c0-f4f90f633d99`:
  all 374 passed, including the stylesheet test.
- `npx playwright test` in `e2e/` with the keyboard-shortcuts, quick-launch, launcher-template-shortcuts, templates,
  terminal-link-clicks, provisioning and hover-text specs, on Chromium and WebKit, three workers, no retries, run
  `1fab6281-683b-40d7-b3cf-e25d577ea5fc`: 471 passed, 9 failed, while other agents' work kept the machine's load average
  around 34 (the run took 46 minutes). Each failure was re-run three times on the engine it failed on. The three
  templates cases (WebKit, run `3b064323-2abc-4782-b781-ea521ce3d4fe`, 9 of 9) and the two quick-launch cases (WebKit,
  run `7bd9764d-1e13-475c-b1bd-367baea32131`, 6 of 6), which had failed on 60-second timeouts and 5-second waits, passed
  every time at comparable load (about 29 and 37); they are treated as load and not recorded as flakes. Of the four
  provisioning cases, three failed again now and then ("expanded setup failure closes the menu below it", "a wrong-host
  UPDATE acceptance warns on its source row and claims nothing", "two provisioning-busy rows reconcile independently";
  runs `d673d672-b658-4f1c-aa4b-96d5bf04d834` and `a1386bf4-79df-486c-b908-aa7ae0ecff86`) and "incarnation churn keeps a
  followed run while a target change detaches it" passed. The plan changes no host or setup code, and on main `73da69e1`
  without the plan the four cases, three times each on both engines at load about 30, failed 4 of 24 with the same
  errors (run `bdf6095f-57fd-4494-90c8-f4be6bf90b59`; there "two provisioning-busy rows" passed, while its failure here
  was the same host-panel measuring wait "expanded setup" hit). They are not this plan's.
- Reused: the first attempt's whole Rust workspace run for the plan's non-UI Rust (the helm refusing spaced names in the
  agent's template create, unchanged since), and the whole Rust workspace run of round 161,
  `4a0d6437-c2bf-43d4-8a19-18b6aada9171` (4581 passed; the known shallow-pause flake failed), for
  container-host-account, which changes only the supervisor; no single workspace run covered the plan and
  container-host-account together, whose combined tree got the build and lint above. The first attempt's desktop-build
  checks, desktop-build tests and desktop-asset check, since the redelivery changes no desktop-specific code. The
  executor's docs-screenshot visual review with the new hint, on Chromium. Not run: the rest of the browser suite (the
  first attempt ran it; the redelivery's changes are covered above), the desktop smoke tests, and the native Mac app
  (not runnable on Linux).

The report's checks were the executor's selected runs; the first landing attempt ran the full battery on the first
delivery, and this landing ran the affected tests on the redelivery. Nothing in the executor's redelivered report became
untrue at landing.
