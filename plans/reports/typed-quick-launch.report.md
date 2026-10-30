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
