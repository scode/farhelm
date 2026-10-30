## What this was about

Quick-launch sequences could combine only templates assigned to number keys. The existing sequence prefix now accepts
template names separated by spaces: Cmd+X, `myproject codex`, Enter in the Mac app, or Ctrl+Shift+X elsewhere. On the
page this starts the combined setup; in the session launcher it applies the choices without starting a session. Later
templates win where their choices overlap. No new shortcut was added.

## Things you should know

Names match by exact capitalization first, then a unique exact name ignoring capitalization, then a unique prefix.
Unknown or ambiguous words refuse the whole sequence and name the problem. The amber pill shows queued entries and the
unfinished word, with up to five matching names beneath it. Tab queues a unique completion; Backspace edits only the
unfinished word. Enter reads the current templates even when the suggestions are still loading.

New or renamed template names cannot contain whitespace, including nonbreaking spaces. Existing names remain usable,
save unchanged and retain delete Undo. Tab can queue a spaced name as one entry when its first word identifies it
uniquely; with both `my project` and `myproject`, use the spaced template's number key or the launcher. A name beginning
with a digit also needs its number key or the launcher because a leading bare digit still chooses a binding.

The Launch templates guide explains these choices. The combined changelog entry covers both PRs, and the original TODO
entry is removed. The separate keyboard-shortcuts dialog had not reached main at the final check, so this stack does not
change it. Main's intervening release-note and plan bookkeeping changes do not interact with this work.

## Open questions and possible follow-ups

No required behavior was omitted. Browser checks exercise Chromium and WebKit, including the web client's shortcut
forms; they do not establish native Mac keyboard dispatch. Browser-reserved chords retain their existing behavior.

## The PRs

- [#2828 — require single-word template names](https://github.com/scode/farhelm/pull/2828/changes), draft, based on
  main.
- [#2829 — type template names in launch sequences](https://github.com/scode/farhelm/pull/2829/changes), draft, based on
  #2828.

## Checks run, reused and skipped

- Name-policy Rust selection:
  `cargo nextest run -p farhelm-proto -p farhelm-ui -p farhelm-helm -E 'test(/new_template_names_are_words|duplicate_names_are_free|agent_template_create_refuses_whitespace|a_new_or_renamed_template_needs_the_loaded_list/)'`.
  Four passed in recorder `1ff7bcc5-ab8a-4280-a804-d1e536df6985`, source fingerprint
  `861a0e97f2b726fd87a7515dad941292ec2787d92a347404150ccaa38082af1e`. The added historical-name save/Delete/Undo
  regression passed in `f1a9cf1e-e7bf-478a-b698-229790b7cc0f`. The other cases were selected out, with no runtime
  substrate skips. That added regression, PR1's Clippy, and its 16-case browser selection used source fingerprint
  `0159047fc2c43f72c2e528574f2540aaf6183f38a29dc333d584742e5118b6cc`. These results are reused for PR1's unchanged
  name-validation, duplicate-suggestion and historical-name save/Undo code. Later work adds sequence handling, rendering
  and browser coverage without changing those naming boundaries.
- Sequence Rust selection (`cargo nextest run -p farhelm-ui --lib -E 'test(launch_sequence::tests::)'`): 16 passed in
  `563d612a-274f-4d78-aeed-d78d0428b7a2`, with 814 selected out and no runtime skips. The complete source fingerprint
  was `e26f54d9d146ca5bd1fa8ce712b23fe64edcd328d41312071fc5da49b6c02db3`. This is reused after the indicator's
  empty-word display correction because resolver/state behavior did not change; the final Clippy and web build compile
  the corrected rendering.
- UI JavaScript (`node --test` in its unit-test directory): 374 passed in `289a43e5-1e90-46f7-b3e5-30e2a28177fc` on that
  same fingerprint. The later changes are Rust rendering, CSS placement, prose and browser fixtures; the JavaScript
  implementation and its tests are unchanged. An isolated author's earlier JS record had incomplete source identity and
  supplies no acceptance evidence.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed in `260007d1-5c46-4d7d-b12f-44260e55ad88`, source
  fingerprint `8b8a2f0dd710177dc6b6bcbeae8eabbbf214d515a29eaf096be9cc82b8f7efd4`; PR1's proto/UI/helm Clippy passed in
  `84476c5d-d133-44a2-8ae0-23d3e57ea06a`. The final web release bundle passed in `b917de89-e521-40f5-bc43-15b314f3df34`
  after a visual review found and corrected wrapped hints crossing the pill's curved border. The final source
  fingerprint is `a8a41e8bb51bc2389be53ec427405392645aa563ddb42600fa0c3d495832486b`. The radius correction changes no
  layout dimensions, input behavior or native code, so earlier native/JS/Clippy results remain applicable. The
  backend/fixture build from `e00c7b83-c98b-4e0c-a859-3245936a9cde` (`cargo build -p farhelm -p farhelm-fixtures`),
  source fingerprint `533861f8d558de0ab5718e308bf324193334966632e1c41b4da79feb2d10085f`, is reused: later edits leave
  its backend and fixture sources unchanged.
- `npx playwright test 'quick-launch\.spec\.ts' 'launcher-template-shortcuts\.spec\.ts'` passed 112 cases, 56 per
  engine, in `2e328728-9a49-4497-a49e-358b6daa3b91`, on that final fingerprint. All eight page/launcher captures at
  1280px and 390px were inspected in Chromium and WebKit.
  `npx playwright test 'templates\.spec\.ts' 'template-list-shortcut\.spec\.ts'` passed 136 cases, 68 per engine, in
  `d918f0a8-7b9f-4465-9cf1-5c1c8bda8b86`, on the same fingerprint. Together these commands cover all 248 related cases,
  124 per engine, with zero retries, skips, failures or unstarted cases. The first PR's focused name-entry and
  legacy-compatibility browser selection passed 16 cases, eight per engine, in `e5de3351-ff45-472f-82be-7a0fafac9abd`.
- Failed and interrupted browser attempts remain retained. The first exposed three authored fixture mistakes: a wrong
  template field name, normalized text used to inspect an invisible character, and a narrow-window scroll target that
  overshot the main area. Correcting those premises preserved the assertions; the six affected engine instances passed
  in `108406df-9721-4909-a4f0-7f5f30b788c6`. Broad run `af300644-ef04-4ad5-a842-3da9fab95e54` was interrupted after
  fixture failures; narrow run `d9cd7533-df75-4a43-8b66-68126505b9ec` failed. Neither is pass evidence. Later broad run
  `0e993b78-26cd-4fee-81cf-9a6e5e5da64b` was interrupted after four timing failures; none reproduced in two
  unchanged-source attempts each (`097f09f7-a99e-4113-a3a9-022e0e6b1a40` for Chromium and
  `a8c307ef-ef9e-4011-9087-79055be5bdf3` for WebKit). Those single-engine reproductions use generic recording, not
  strict browser-policy counts. Both interrupted broad attempts report forced cleanup. Their partial passes are not
  reused as command passes, and the evidence does not establish a product regression or latent flake.
- Website build and internal links (`bun run build`) passed in `ab7c566d-b939-4932-98a8-ecc92d00a1da`, source
  fingerprint `a2506642982490eccfeb8e326eac5bb3043796efba5119e9aafe8abe2a55cd4b`. Reused because website sources have
  not changed since that build. The first attempted Bun invocation printed usage rather than building and is not
  evidence. Formatting, changed-Markdown lint, changelog format and the maintained whole-checkout sleep checker passed;
  it inspected 327 delays with zero missing rationales. Two isolated source-check attempts exceeded the checker's
  budget; the integrated check passed without changing its deadline.
- The full workspace/browser batteries, installer, provisioning and native desktop runtime were skipped because the
  focused suites cover the changed naming and sequence boundaries. No CI run was requested. All accepted runtime records
  have complete source identity and retained output, with normal cleanup.

## The review gate's outcome

Each PR passed its required single gpt-6.1-sol review at high effort and the commit/PR wording cold read. PR1's review
added Rust coverage for historical-name Undo. PR2's four findings were fixed and rechecked: the empty word no longer
shows `(empty)`, the specification distinguishes exact references from freshly matched words, and two browser fixtures
now establish their narrow-layout and held-catalog premises. No finding was rejected. Both isolated authors completed
their assumptions and decisions checkpoints; their work was inspected and validated after integration.
